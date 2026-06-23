use parking_lot::Mutex;
/// Background scheduler for autopilot records with a non-manual schedule.
///
/// Spawns a single Tokio task on app startup. Every minute it checks which
/// autopilots are due to run, fires them off in separate tasks, and updates
/// `lastRunAt` in the store. Respects the `status` field — paused/archived
/// autopilots are never triggered.
///
/// Schedules are **clock-anchored** in DEVICE-LOCAL time: a recurring schedule
/// fires at a chosen wall-clock time, not on a rolling interval.
///   manual      — never triggered automatically
///   hourly      — every hour at `:scheduleMinute` (minute past the hour;
///                 `scheduleHour` ignored; defaults to minute 0)
///   daily       — once a day at `scheduleHour:scheduleMinute` (defaults 09:00)
///   twice_daily — at `scheduleHour:scheduleMinute` AND 12 h later
///
/// Due-ness is decided against the **most recent scheduled occurrence at-or-
/// before now** (see [`last_occurrence_ms`]): an autopilot is due iff its
/// `lastRunAt` predates that occurrence. This gives catch-up for free — a
/// missed occurrence (app was closed) runs once shortly after the next launch
/// — while never double-running, because once `lastRunAt` is stamped at/after
/// the occurrence it is no longer due until the next one rolls around.
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use tauri::{AppHandle, Manager};

use crate::autopilot::{Autopilot, AutopilotStatus, AutopilotStore};
use crate::db::ts_to_db;

const TICK_INTERVAL_SECS: u64 = 60;

/// Grace period after launch before the first catch-up sweep, so the app finishes
/// startup (window, stores, plugins) before any autopilot scrape kicks off.
const STARTUP_CATCHUP_DELAY_SECS: u64 = 5;

/// Default local clock time for daily/twice_daily when no time is set, so
/// records created before the run-time picker keep firing in the morning.
const DEFAULT_HOUR: u32 = 9;
const DEFAULT_MINUTE: u32 = 0;

/// Build a local `DateTime` for `date` (taken from `anchor`) at `h:m:00`.
/// Returns `None` only for the rare non-existent local wall-clock times (DST
/// spring-forward gaps), in which case the caller treats the occurrence as
/// absent for that day rather than guessing.
fn local_at(anchor: &DateTime<Local>, h: u32, m: u32) -> Option<DateTime<Local>> {
    Local
        .with_ymd_and_hms(anchor.year(), anchor.month(), anchor.day(), h, m, 0)
        .single()
}

/// Most recent scheduled occurrence at-or-before `now` (local), as epoch ms.
///
/// `None` for manual/unknown schedules. For recurring schedules this is always
/// `Some` under normal clocks (it walks back to the previous hour/day when the
/// time has not yet been reached today).
///
/// Hour/minute are defensively clamped to valid ranges before use: a legacy or
/// imported record carrying an out-of-range value (e.g. `hour = 25`) falls back
/// to the safe default instead of producing a permanently-`None` occurrence
/// (silently-dead autopilot). Belt-and-suspenders with the storage-side range
/// guard in [`crate::autopilot`].
fn last_occurrence_ms(
    schedule: &str,
    hour: Option<u32>,
    minute: Option<u32>,
    now: DateTime<Local>,
) -> Option<i64> {
    // Clamp out-of-range times to the safe default rather than trusting the
    // stored value — `local_at` would otherwise return `None` forever.
    let hour = hour.filter(|&h| h <= 23);
    let minute = minute.filter(|&m| m <= 59);
    match schedule {
        "hourly" => {
            // Every hour at `:m`. This hour's `:m` if already past it, else the
            // previous hour's `:m`.
            let m = minute.unwrap_or(DEFAULT_MINUTE);
            let this_hour = local_at(&now, now.hour(), m)?;
            let occ = if this_hour <= now {
                this_hour
            } else {
                this_hour - chrono::Duration::hours(1)
            };
            Some(occ.timestamp_millis())
        }
        "daily" => {
            // Today at `h:m` if already past it, else yesterday's `h:m`.
            let h = hour.unwrap_or(DEFAULT_HOUR);
            let m = minute.unwrap_or(DEFAULT_MINUTE);
            let today = local_at(&now, h, m)?;
            let occ = if today <= now {
                today
            } else {
                today - chrono::Duration::days(1)
            };
            Some(occ.timestamp_millis())
        }
        "twice_daily" => {
            // Two daily occurrences {`h:m`, `h:m`+12h}. The latest of the four
            // candidates (today + yesterday, both offsets) that is `<= now`.
            let h = hour.unwrap_or(DEFAULT_HOUR);
            let m = minute.unwrap_or(DEFAULT_MINUTE);
            let base = local_at(&now, h, m)?;
            let twelve = chrono::Duration::hours(12);
            let day = chrono::Duration::days(1);
            [base, base + twelve, base - day, base + twelve - day]
                .into_iter()
                .filter(|occ| *occ <= now)
                .max()
                .map(|occ| occ.timestamp_millis())
        }
        _ => None, // manual or unknown — never auto-run
    }
}

fn is_due(ap: &Autopilot) -> bool {
    if ap.status != AutopilotStatus::Active {
        return false;
    }
    let Some(occurrence_ms) = last_occurrence_ms(
        &ap.schedule,
        ap.schedule_hour,
        ap.schedule_minute,
        Local::now(),
    ) else {
        return false; // manual/unknown — never auto-run
    };
    match ap.last_run_at {
        // Never ran → run once soon after creation (preserves today's
        // first-run-immediately behaviour).
        None => true,
        // Due iff the last run predates the most recent occurrence: a missed or
        // just-reached occurrence is due; a run at/after it is not (no
        // double-run until the next occurrence).
        Some(last) => ts_to_db(last) < occurrence_ms,
    }
}

pub fn start(app: AppHandle) {
    let store: Arc<Mutex<AutopilotStore>> =
        app.state::<Arc<Mutex<AutopilotStore>>>().inner().clone();

    // Reconcile any run left mid-flight by a crash/close before the first sweep
    // could start a new one, so the UI shows an honest "interrupted" badge
    // rather than a stuck "running" state.
    store.lock().mark_interrupted_runs();

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
    use chrono::Offset;

    use super::*;
    use crate::autopilot::{AutopilotFilter, AutopilotTarget};

    fn ap(
        schedule: &str,
        status: AutopilotStatus,
        last_run_at: Option<u64>,
        schedule_hour: Option<u32>,
        schedule_minute: Option<u32>,
    ) -> Autopilot {
        Autopilot {
            id: "id".into(),
            name: "name".into(),
            status,
            target: AutopilotTarget {
                boards: vec!["linkedin".into()],
                query: "rust".into(),
                location: None,
                country_code: None,
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
            schedule_hour,
            schedule_minute,
            resume_text: None,
            cover_letter: None,
            total_found: 0,
            total_applied: 0,
            found_jobs: Vec::new(),
            run_status: None,
            last_run_at,
            created_at: 0,
            updated_at: 0,
        }
    }

    /// A fixed *local* wall-clock instant on 2026-06-04 at `h:m:00`, built
    /// timezone-portably.
    ///
    /// We can't construct it via `Local.with_ymd_and_hms(...)` — on a CI runner
    /// whose local tz makes that wall time non-existent or ambiguous (a DST
    /// gap/overlap), `.single()` is `None` and `.unwrap()` panics. Instead we
    /// derive the UTC offset for that calendar day from a stable epoch instant
    /// and subtract it, so we land on a real instant whose *local* H:M is what
    /// we asked for, on every runner regardless of timezone.
    fn local_on_2026_06_04(h: u32, m: u32) -> DateTime<Local> {
        // Stable anchor: 2026-06-04 00:00:00 UTC, expressed in local time.
        // 1_780_531_200_000 ms = 2026-06-04T00:00:00Z.
        let local_midnight_utc = DateTime::from_timestamp_millis(1_780_531_200_000)
            .unwrap()
            .with_timezone(&Local);
        // Offset that Local applies on that day; subtracting it makes the
        // resulting instant read as `h:m` on the local wall clock.
        let offset_secs = local_midnight_utc.offset().fix().local_minus_utc() as i64;
        let target_utc_ms =
            1_780_531_200_000 + (h as i64 * 3_600 + m as i64 * 60 - offset_secs) * 1_000;
        DateTime::from_timestamp_millis(target_utc_ms)
            .unwrap()
            .with_timezone(&Local)
    }

    /// Fixed reference instant on 2026-06-04 at `h:m` local. All occurrence math
    /// is deterministic against it (independent of the wall clock) and the
    /// construction never panics on any runner's timezone.
    fn now_at(h: u32, m: u32) -> DateTime<Local> {
        local_on_2026_06_04(h, m)
    }

    /// Same calendar instant one day earlier (2026-06-03) at `h:m` local —
    /// used by the "yesterday's occurrence" assertions.
    fn yesterday_at(h: u32, m: u32) -> DateTime<Local> {
        now_at(h, m) - chrono::Duration::days(1)
    }

    // ── last_occurrence_ms ─────────────────────────────────────────────────

    #[test]
    fn manual_and_unknown_have_no_occurrence() {
        let now = now_at(14, 30);
        assert_eq!(last_occurrence_ms("manual", None, None, now), None);
        assert_eq!(last_occurrence_ms("weekly", None, None, now), None);
    }

    #[test]
    fn hourly_uses_this_hour_when_past_the_minute_else_previous_hour() {
        // minute 15, now 14:30 → this hour's 14:15.
        let now = now_at(14, 30);
        assert_eq!(
            last_occurrence_ms("hourly", None, Some(15), now),
            Some(now_at(14, 15).timestamp_millis())
        );
        // minute 45, now 14:30 (not yet reached) → previous hour's 13:45.
        assert_eq!(
            last_occurrence_ms("hourly", None, Some(45), now),
            Some(now_at(13, 45).timestamp_millis())
        );
        // scheduleHour is ignored for hourly; default minute is 0.
        assert_eq!(
            last_occurrence_ms("hourly", Some(7), None, now),
            Some(now_at(14, 0).timestamp_millis())
        );
    }

    #[test]
    fn daily_uses_today_when_past_else_yesterday() {
        // 09:00 default, now 14:30 → today 09:00.
        let now = now_at(14, 30);
        assert_eq!(
            last_occurrence_ms("daily", None, None, now),
            Some(now_at(9, 0).timestamp_millis())
        );
        // Scheduled 18:30, now 14:30 (not yet) → yesterday 18:30.
        assert_eq!(
            last_occurrence_ms("daily", Some(18), Some(30), now),
            Some(yesterday_at(18, 30).timestamp_millis())
        );
    }

    #[test]
    fn twice_daily_picks_the_later_reached_occurrence() {
        // Base 09:00 (+12h = 21:00). now 14:30 → the 09:00 slot (21:00 not reached).
        let now = now_at(14, 30);
        assert_eq!(
            last_occurrence_ms("twice_daily", Some(9), Some(0), now),
            Some(now_at(9, 0).timestamp_millis())
        );
        // now 22:00 → the 21:00 slot is now the latest reached today.
        let now_late = now_at(22, 0);
        assert_eq!(
            last_occurrence_ms("twice_daily", Some(9), Some(0), now_late),
            Some(now_at(21, 0).timestamp_millis())
        );
        // Before both of today's slots (now 06:00) → yesterday's later slot 21:00.
        let now_early = now_at(6, 0);
        assert_eq!(
            last_occurrence_ms("twice_daily", Some(9), Some(0), now_early),
            Some(yesterday_at(21, 0).timestamp_millis())
        );
    }

    // ── is_due (live clock; clock-stable invariants) ───────────────────────

    #[test]
    fn manual_is_never_due_even_if_never_run() {
        assert!(!is_due(&ap(
            "manual",
            AutopilotStatus::Active,
            None,
            None,
            None
        )));
    }

    #[test]
    fn paused_or_archived_is_never_due() {
        assert!(!is_due(&ap(
            "hourly",
            AutopilotStatus::Paused,
            None,
            None,
            None
        )));
        assert!(!is_due(&ap(
            "hourly",
            AutopilotStatus::Archived,
            None,
            None,
            None
        )));
        // Even with a long-overdue last run, a non-active record never fires.
        let long_ago = 0;
        assert!(!is_due(&ap(
            "daily",
            AutopilotStatus::Paused,
            Some(long_ago),
            None,
            None
        )));
    }

    #[test]
    fn active_scheduled_is_due_when_never_run() {
        // Never ran → runs once soon after creation (first-run-immediately).
        assert!(is_due(&ap(
            "hourly",
            AutopilotStatus::Active,
            None,
            None,
            None
        )));
        assert!(is_due(&ap(
            "daily",
            AutopilotStatus::Active,
            None,
            None,
            None
        )));
    }

    #[test]
    fn ran_at_or_after_the_latest_occurrence_is_not_due() {
        // The no-double-run invariant, expressed deterministically: a run stamped
        // at-or-after the most recent occurrence means not-due (the `is_due`
        // predicate is `last < occurrence`). We pin `now` to a fixed instant and
        // compute the occurrence against it — no live clock, so no minute-boundary
        // race. `is_due` itself reads `Local::now()`, which is exactly the source
        // of the flake we're avoiding here.
        let now = now_at(14, 30);
        // Mirrors `is_due`'s decision for a record that already ran: due iff
        // `last_run_at < occurrence`. Asserting the negative for runs at/after
        // the occurrence is the no-double-run guarantee.
        let due_after_run = |occ: i64, last_run: i64| last_run < occ;
        for (schedule, hour, minute) in [
            ("hourly", None, None),
            ("daily", None, None),
            ("twice_daily", Some(9), Some(0)),
        ] {
            let occ = last_occurrence_ms(schedule, hour, minute, now)
                .expect("recurring schedule has an occurrence");
            // last_run exactly at the occurrence → not due (boundary is `<`).
            assert!(
                !due_after_run(occ, occ),
                "{schedule}: run at the occurrence is not due"
            );
            // last_run after the occurrence → not due, until the next one rolls.
            assert!(
                !due_after_run(occ, occ + 1),
                "{schedule}: run after the occurrence is not due"
            );
        }
    }

    #[test]
    fn missed_occurrence_while_closed_is_caught_up_once() {
        // Last ran far in the past (epoch 0) — well before the most recent
        // occurrence — so the missed run is caught up exactly once on next tick.
        assert!(is_due(&ap(
            "daily",
            AutopilotStatus::Active,
            Some(0),
            None,
            None
        )));
        assert!(is_due(&ap(
            "twice_daily",
            AutopilotStatus::Active,
            Some(0),
            Some(9),
            Some(0)
        )));
    }
}
