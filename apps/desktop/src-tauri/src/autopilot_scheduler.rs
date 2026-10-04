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
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::autopilot::{Autopilot, AutopilotStatus, AutopilotStore};
use crate::db::ts_to_db;

const TICK_INTERVAL_SECS: u64 = 60;

/// Grace period after launch before the first catch-up sweep, so the app finishes
/// startup (window, stores, plugins) before any autopilot scrape kicks off.
const STARTUP_CATCHUP_DELAY_SECS: u64 = 5;

/// Backoff before a bounded retry: [`run_with_single_retry`]'s single retry of
/// a `Failed` run, and [`schedule_interrupted_retries`]'s crash-recovery retry.
/// Long enough to let a transient cause clear (a 429 window, a flaky network)
/// without hammering.
///
/// This duration alone does NOT guarantee a retry never races a fresh scheduled
/// run — for an hourly schedule, 12 minutes is NOT always "well inside" the next
/// occurrence. Both arms re-validate the record immediately before firing, but
/// on DIFFERENT signals, so the closed gap differs:
///   - [`schedule_interrupted_retries`] re-checks the FULL [`still_needs_recovery`]
///     predicate (schedulable + not due + `last_run_at` unchanged) and defers to
///     the tick if the slot rolled or was already served during the sleep — the
///     race against a normal scheduled run is fully closed for this arm.
///   - [`run_with_single_retry`]'s Failed-run retry re-checks only
///     [`should_retry_after_backoff`] — PAUSE WINS: a user Pausing/Archiving/
///     switching to manual during the backoff must not still get the retry
///     scrape. It does NOT re-check due-ness/`last_run_at`, so a schedule whose
///     interval is on the same order as this backoff (hourly) could in theory
///     still have its retry race a fresh tick run that already served the next
///     occurrence. Accepted as a narrower, documented trade-off (not closed
///     here): the retry is still bounded to ONE, and the concurrent-run guard
///     on `autopilot_run` prevents the two from literally overlapping, even
///     though it can't prevent them running sequentially.
const RETRY_BACKOFF: Duration = Duration::from_secs(12 * 60);

/// Default local clock time for daily/twice_daily when no time is set, so
/// records created before the run-time picker keep firing in the morning.
const DEFAULT_HOUR: u32 = 9;
const DEFAULT_MINUTE: u32 = 0;

/// Width of the per-autopilot schedule jitter window.
///
/// Every install created before the run-time picker defaults to 09:00
/// ([`DEFAULT_HOUR`]/[`DEFAULT_MINUTE`]), and the tick is 60 s wide — so without
/// this, every default-schedule install in a time zone hits the same third-party
/// APIs inside the same minute. That is a thundering herd against hosts we do
/// not own, and this repo's own logs already show `adzuna: HTTP 503` on
/// scheduled runs.
///
/// Ten minutes, deliberately: wide enough to spread a herd across ten tick
/// windows, far enough under the shortest interval (hourly) that a shifted
/// occurrence can never overtake the next one, and small enough that a user who
/// chose 09:00 is not surprised by what they see.
const SCHEDULE_JITTER_WINDOW_SECS: i64 = 10 * 60;

/// Hourly is the tightest schedule this scheduler supports. If the window ever
/// grew past an hour, a shifted occurrence could jump the NEXT one and silently
/// skip a run — a failure no runtime test would catch, because they all use
/// small offsets. Asserted at COMPILE time rather than as a test: a test
/// comparing two constants is a tautology clippy rightly rejects, and this makes
/// the bad value unbuildable instead of merely reported.
const _: () = assert!(SCHEDULE_JITTER_WINDOW_SECS < 3600);

/// A stable per-autopilot offset inside [0, [`SCHEDULE_JITTER_WINDOW_SECS`]).
///
/// **Deterministic, not random**, and that is the whole design. The occurrence
/// stays a single well-defined instant, so everything built on it still holds:
/// catch-up after a missed occurrence, the no-double-run property once
/// `lastRunAt` is stamped at/after it, and reproducible tests. A random delay
/// would have meant sleeping inside the tick, which reopens both.
///
/// FNV-1a rather than `DefaultHasher`: the standard hasher is explicitly NOT
/// guaranteed stable across Rust releases, and an offset that moved on a
/// toolchain bump would silently shift every user's schedule. Ids are UUIDs, so
/// the low bits are already well distributed.
fn jitter_for(id: &str) -> chrono::Duration {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    chrono::Duration::seconds((hash % SCHEDULE_JITTER_WINDOW_SECS as u64) as i64)
}

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
    jitter: chrono::Duration,
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
            let this_hour = local_at(&now, now.hour(), m)? + jitter;
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
            let today = local_at(&now, h, m)? + jitter;
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
            let base = local_at(&now, h, m)? + jitter;
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

/// Whether the scheduler auto-runs this record at all — `Active` with a
/// recurring (non-manual) schedule — independent of whether it is *currently*
/// due. Used to decide if a crash-interrupted run is worth a recovery retry: a
/// paused, archived, or manual record never auto-runs, so it is never retried.
fn is_schedulable(ap: &Autopilot) -> bool {
    ap.status == AutopilotStatus::Active
        && last_occurrence_ms(
            &ap.schedule,
            ap.schedule_hour,
            ap.schedule_minute,
            Local::now(),
            jitter_for(&ap.id),
        )
        .is_some()
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
        jitter_for(&ap.id),
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
    // rather than a stuck "running" state. The reconciled ids drive a bounded
    // recovery retry below.
    let interrupted = store.lock().mark_interrupted_runs();

    // One-shot, idempotent loosen of autopilots saved with the old auto-prefilled
    // restrictive filters (the zero-jobs bug). Gated by a sidecar marker file, so
    // it is a no-op after the first run. Synchronous file IO — fine here on the
    // setup path, before the sweep loop spawns below.
    store.lock().relax_legacy_filters_once();

    // Recover runs cut off mid-flight by the last crash/close (see below). Done
    // before the catch-up sweep spawns so both observe the same reconciled state.
    schedule_interrupted_retries(&app, &store, interrupted);

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
        // Stamp lastRunAt immediately so a slow run doesn't trigger twice. This
        // stamp-before-run double-fire guard is UNCHANGED — the retry below is a
        // separate, bounded recovery that never re-stamps or re-enters `is_due`.
        store.lock().stamp_last_run(&ap.id);

        let app_clone = app.clone();
        let store_clone = store.clone();
        let ap_id = ap.id.clone();
        tauri::async_runtime::spawn(run_with_single_retry(app_clone, store_clone, ap_id));
    }
}

/// Run an autopilot once and, if it ended `Failed`, retry it exactly ONCE after
/// [`RETRY_BACKOFF`]. This closes the scheduler's only gap: the stamp-before-run
/// double-fire guard consumes the occurrence up-front, so before this a failed
/// occurrence was lost with no retry until the next one rolled.
///
/// Immediately before firing the retry, the record is re-read and the retry is
/// skipped unless [`should_retry_after_backoff`] still holds — PAUSE WINS: a
/// user Pausing, Archiving, or switching the schedule to manual during the
/// backoff must not still get a full scrape (updated `lastRunAt`/`foundJobs`,
/// a possible notification) for a record they just told the app to stop
/// running.
///
/// Bounded to a single retry — the retry's own outcome is deliberately NOT
/// re-inspected — so a persistently-failing board can never spin a retry storm.
/// In-process (not persisted): a retry still pending when the app closes is
/// dropped rather than queued. Justified over a durable retry queue by YAGNI —
/// the audit asked for ONE bounded retry, not a job queue; the next scheduled
/// occurrence still runs, and the concurrent-run guard on `autopilot_run` keeps
/// a retry that overlaps a fresh run from double-running.
async fn run_with_single_retry(app: AppHandle, store: Arc<Mutex<AutopilotStore>>, ap_id: String) {
    let outcome = crate::commands::autopilot::autopilot_run(app.clone(), ap_id.clone()).await;
    if !outcome_failed(&outcome) {
        return;
    }
    log::info!(
        "[autopilot] run {ap_id} failed; one retry in {}s if still active",
        RETRY_BACKOFF.as_secs()
    );
    tokio::time::sleep(RETRY_BACKOFF).await;
    // Re-check right before firing (see the doc comment above and
    // `should_retry_after_backoff`): pause/archive/switch-to-manual (or a
    // deletion) during the backoff wins over the pending retry.
    let record = store.lock().get(&ap_id);
    if !should_retry_after_backoff(record.as_ref()) {
        log::info!("[autopilot] retry for {ap_id} skipped — no longer schedulable");
        return;
    }
    // Single retry — outcome intentionally ignored (bounded; no second retry).
    crate::commands::autopilot::autopilot_run(app, ap_id).await;
}

/// Whether the Failed-run retry should still fire, given a fresh read of the
/// record taken immediately before it (after the backoff sleep). Mirrors
/// [`still_needs_recovery`]'s pause-wins principle for this OTHER retry arm: a
/// user Pausing, Archiving, or switching the schedule to manual during the
/// backoff window must win over the pending retry — a paused record must never
/// get a scrape, even from a bounded recovery retry. `None` (the record was
/// deleted during the backoff) also skips.
fn should_retry_after_backoff(ap: Option<&Autopilot>) -> bool {
    ap.is_some_and(is_schedulable)
}

/// Whether an `autopilot_run` resolved payload represents a run that ended
/// `Failed` — the ONLY outcome eligible for a retry. Two failure shapes both
/// qualify: the outright-scrape-error `{ error, .. }` (never reached the record;
/// persisted `Failed` via `fail_run_without_summaries`) and the derived
/// `status: "failed"` (reached the record but zero boards succeeded). A
/// `Completed`/`CompletedWithErrors` run (real, possibly-partial results), a
/// user-`cancelled` run, and a `skipped` double-invoke are NEVER retried.
fn outcome_failed(payload: &Value) -> bool {
    // A user stop or a de-duplicated double-invoke is not a failure.
    if payload
        .get("cancelled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || payload.get("skipped").is_some()
    {
        return false;
    }
    if payload.get("error").is_some() {
        return true;
    }
    payload.get("status").and_then(Value::as_str) == Some("failed")
}

/// Whether an interrupted-run recovery is still warranted for `ap`, given the
/// `last_run_at` snapshot captured when the recovery was first scheduled.
/// Evaluated both at scheduling time and again immediately before the retry
/// actually runs (after the backoff sleep) — a NORMAL scheduled `tick` can
/// independently serve this record's occurrence during that sleep, and its own
/// `stamp_last_run` moves `last_run_at` forward. Re-running on top of that would
/// be a redundant, purely SEQUENTIAL double-scrape the concurrent-run guard
/// (`RunGuard`) cannot catch, since the two runs never overlap.
///
/// All three must hold:
///   - `is_schedulable` — still `Active` with a recurring schedule (not paused/
///     archived/switched to manual since the crash);
///   - `!is_due` — the slot is still consumed by SOME stamp, not a freshly-due,
///     not-yet-served occurrence the tick is about to own;
///   - `last_run_at == baseline` — nothing has re-stamped it since the snapshot.
///     This is the clause that actually distinguishes "still the original
///     crashed stamp" from "a fresh tick run already served a later
///     occurrence" — `!is_due` alone reads identically for both (either way the
///     slot reads as "not due"), so it can't tell them apart on its own.
fn still_needs_recovery(ap: &Autopilot, baseline_last_run_at: Option<u64>) -> bool {
    is_schedulable(ap) && !is_due(ap) && ap.last_run_at == baseline_last_run_at
}

/// Schedule ONE delayed, best-effort recovery retry for each run interrupted by
/// the last crash/close whose scheduled occurrence has NOT since rolled. A
/// rolled occurrence is already re-run by the startup catch-up sweep, so only
/// the same-occurrence gap ([`still_needs_recovery`]: a schedulable record whose
/// slot is already consumed, and unchanged since) is recovered here — a paused/
/// manual record is skipped, and skipped again on the pre-run recheck below if
/// the tick beat the recovery to it. Bounded to a single `autopilot_run` (not
/// [`run_with_single_retry`], which would add a further retry) so a run that
/// keeps being interrupted can only re-run once per app launch — never a storm.
/// In-process/best-effort: a pending recovery is dropped if the app closes
/// again.
fn schedule_interrupted_retries(
    app: &AppHandle,
    store: &Arc<Mutex<AutopilotStore>>,
    interrupted: Vec<String>,
) {
    for id in interrupted {
        let Some(ap) = store.lock().get(&id) else {
            continue;
        };
        let baseline_last_run_at = ap.last_run_at;
        if !still_needs_recovery(&ap, baseline_last_run_at) {
            continue;
        }
        let app = app.clone();
        let store = store.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(RETRY_BACKOFF).await;
            // Re-check right before running (see `still_needs_recovery` doc): if
            // the slot rolled to a fresh due occurrence, or a tick already
            // served it during the sleep, the normal scheduler owns it — skip.
            let still_needed = store
                .lock()
                .get(&id)
                .is_some_and(|ap| still_needs_recovery(&ap, baseline_last_run_at));
            if !still_needed {
                return;
            }
            crate::commands::autopilot::autopilot_run(app, id).await;
        });
    }
}

#[cfg(test)]
mod tests;
