use parking_lot::Mutex;
/// Background scheduler for autopilot records with a non-manual schedule.
///
/// Spawns a single Tokio task on app startup. Every minute it checks which
/// autopilots are due to run, fires them off in separate tasks, and updates
/// `lastRunAt` in the store. Respects the `status` field — paused/archived
/// autopilots are never triggered.
///
/// Schedule intervals:
///   manual      — never triggered automatically
///   hourly      — run if last_run_at is > 60 min ago (or never ran)
///   twice_daily — run if last_run_at is > 12 h ago (or never ran)
///   daily       — run if last_run_at is > 24 h ago (or never ran)
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::autopilot::{Autopilot, AutopilotStatus, AutopilotStore};

const TICK_INTERVAL_SECS: u64 = 60;

/// Grace period after launch before the first catch-up sweep, so the app finishes
/// startup (window, stores, plugins) before any autopilot scrape kicks off.
const STARTUP_CATCHUP_DELAY_SECS: u64 = 5;

/// Seconds between auto-runs for each schedule variant.
fn interval_secs(schedule: &str) -> Option<u64> {
    match schedule {
        "hourly" => Some(60 * 60),
        "twice_daily" => Some(12 * 60 * 60),
        "daily" => Some(24 * 60 * 60),
        _ => None, // manual or unknown — never auto-run
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn is_due(ap: &Autopilot) -> bool {
    let Some(interval) = interval_secs(&ap.schedule) else {
        return false;
    };
    if ap.status != AutopilotStatus::Active {
        return false;
    }
    let elapsed_ms = match ap.last_run_at {
        Some(last) => now_ms().saturating_sub(last),
        None => u64::MAX, // never ran → always due
    };
    elapsed_ms >= interval * 1_000
}

pub fn start(app: AppHandle) {
    let store: Arc<Mutex<AutopilotStore>> =
        app.state::<Arc<Mutex<AutopilotStore>>>().inner().clone();

    tauri::async_runtime::spawn(async move {
        // Catch up on autopilots that fell overdue while the app was closed:
        // run one sweep shortly after launch rather than waiting a full tick
        // interval. The brief delay lets startup settle first.
        tokio::time::sleep(Duration::from_secs(STARTUP_CATCHUP_DELAY_SECS)).await;
        tick(&app, &store).await;

        let mut interval = tokio::time::interval(Duration::from_secs(TICK_INTERVAL_SECS));
        interval.tick().await; // consume the immediate tick (catch-up already ran)

        loop {
            interval.tick().await;
            tick(&app, &store).await;
        }
    });
}

fn collect_due(store: &Arc<Mutex<AutopilotStore>>) -> Vec<Autopilot> {
    store.lock().list().into_iter().filter(is_due).collect()
}

async fn tick(app: &AppHandle, store: &Arc<Mutex<AutopilotStore>>) {
    let due = collect_due(store);

    for ap in due {
        // Stamp lastRunAt immediately so a slow run doesn't trigger twice.
        store.lock().stamp_last_run(&ap.id);

        let app_clone = app.clone();
        let ap_id = ap.id.clone();
        tauri::async_runtime::spawn(async move {
            crate::commands::autopilot::autopilot_run(app_clone, ap_id).await;
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::autopilot::{AutopilotFilter, AutopilotTarget};

    fn ap(schedule: &str, status: AutopilotStatus, last_run_at: Option<u64>) -> Autopilot {
        Autopilot {
            id: "id".into(),
            name: "name".into(),
            status,
            target: AutopilotTarget {
                board: "linkedin".into(),
                query: "rust".into(),
                location: None,
                work_type: None,
                pages: 1,
                date_filter: None,
                top_n: 3,
            },
            filter: AutopilotFilter {
                min_match_score: 0.0,
                keywords: None,
                exclude_keywords: None,
            },
            schedule: schedule.into(),
            resume_text: None,
            cover_letter: None,
            total_found: 0,
            total_applied: 0,
            found_jobs: Vec::new(),
            last_run_at,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn interval_secs_maps_known_schedules() {
        assert_eq!(interval_secs("hourly"), Some(60 * 60));
        assert_eq!(interval_secs("twice_daily"), Some(12 * 60 * 60));
        assert_eq!(interval_secs("daily"), Some(24 * 60 * 60));
        assert_eq!(interval_secs("manual"), None);
        assert_eq!(interval_secs("weekly"), None); // unknown → never auto-run
    }

    #[test]
    fn manual_is_never_due_even_if_never_run() {
        assert!(!is_due(&ap("manual", AutopilotStatus::Active, None)));
    }

    #[test]
    fn paused_or_archived_is_never_due() {
        assert!(!is_due(&ap("hourly", AutopilotStatus::Paused, None)));
        assert!(!is_due(&ap("hourly", AutopilotStatus::Archived, None)));
    }

    #[test]
    fn active_scheduled_is_due_when_never_run() {
        assert!(is_due(&ap("hourly", AutopilotStatus::Active, None)));
    }

    #[test]
    fn due_only_after_the_interval_elapses() {
        let thirty_min_ago = now_ms() - 30 * 60 * 1000;
        assert!(!is_due(&ap(
            "hourly",
            AutopilotStatus::Active,
            Some(thirty_min_ago)
        )));

        let two_hours_ago = now_ms() - 2 * 60 * 60 * 1000;
        assert!(is_due(&ap(
            "hourly",
            AutopilotStatus::Active,
            Some(two_hours_ago)
        )));
    }
}
