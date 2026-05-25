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
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::autopilot::{Autopilot, AutopilotStatus, AutopilotStore};

const TICK_INTERVAL_SECS: u64 = 60;

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
    let store: Arc<Mutex<AutopilotStore>> = app
        .state::<Arc<Mutex<AutopilotStore>>>()
        .inner()
        .clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(TICK_INTERVAL_SECS));
        interval.tick().await; // consume the immediate first tick

        loop {
            interval.tick().await;
            tick(&app, &store).await;
        }
    });
}

fn collect_due(store: &Arc<Mutex<AutopilotStore>>) -> Vec<Autopilot> {
    match store.lock() {
        Ok(g) => g.list().into_iter().filter(is_due).collect(),
        Err(_) => vec![],
    }
}

async fn tick(app: &AppHandle, store: &Arc<Mutex<AutopilotStore>>) {
    let due = collect_due(store);

    for ap in due {
        // Stamp lastRunAt immediately so a slow run doesn't trigger twice.
        if let Ok(g) = store.lock() {
            g.stamp_last_run(&ap.id);
        }

        let app_clone = app.clone();
        let ap_id = ap.id.clone();
        tokio::spawn(async move {
            crate::commands::autopilot::autopilot_run(app_clone, ap_id).await;
        });
    }
}
