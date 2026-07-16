//! Background scheduler for email-confirmation watching (Task #23, auto-track
//! Layer C). Mirrors [`crate::autopilot_scheduler`]'s split from its L1
//! store: `email_watch` (the store + connector + pure parse/match/poll logic)
//! stays Tauri-free, and THIS module (L2) is the one place in the whole
//! family that reaches up into `commands::notifications::push_and_notify`
//! (L3) — the same upward shell-reach `autopilot_scheduler` already has for
//! `commands::autopilot::autopilot_run`, via its own `R7_ALLOW` entry rather
//! than growing one on the L1 store.
//!
//! Spawns a single Tokio task on app startup (`start`), after a short
//! [`STARTUP_GRACE`] so the rest of boot settles first. Every
//! [`TICK_INTERVAL`] it checks whether a REAL IMAP check is due — gated by
//! [`is_due`], which measures elapsed time against `last_check_ms` (the
//! timestamp of the last ATTEMPT, success or failure) and a
//! consecutive-failure backoff ([`backoff_interval`]) — bounding the
//! Gmail-auth-spam abuse case a security review flagged (min interval +
//! failure backoff, PR B pinned requirement #2).
//!
//! [`run_check`] is the shared fetch+parse+match+notify pass: the scheduler's
//! own due-gated tick calls it, and so does the manual
//! `email_watch_check_now` command (after its own separate 60 s
//! min-interval guard — see `commands::email_watch`), so "real" behavior is
//! defined exactly once. [`run_check`] ALSO carries its own concurrent-run
//! guard ([`RunGuard`]) — the 60 s min-gap check alone is TOCTOU (it reads
//! `last_check_ms` before the multi-second IMAP pass runs, so N concurrent
//! callers can all pass the same stale read); the guard makes "only one
//! `run_check` body executes at a time" true regardless of caller.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::Local;
use parking_lot::Mutex;
use tauri::{AppHandle, Manager};

use crate::applications::{Application, ApplicationStatus, ApplicationStore};
use crate::credentials::CredentialStore;
use crate::db::now_ms;
use crate::email_watch::imap_client::{self, DEFAULT_IMAP_HOST, DEFAULT_IMAP_PORT};
use crate::email_watch::{poller, EmailWatchStatus, EmailWatchStore, CREDENTIAL_SLOT};
use crate::error::{AppError, AppResult};

/// Internal check cadence — how often the loop wakes up to ask "is a real
/// IMAP check due yet". NOT the interval between real checks (that is
/// [`BASE_CHECK_INTERVAL`] × backoff) — mirrors `autopilot_scheduler`'s own
/// 60 s sweep tick against a longer effective schedule.
const TICK_INTERVAL: Duration = Duration::from_secs(60);

/// Grace period after launch before the first check, so the app finishes
/// startup (window, stores, plugins) before an IMAP connection is opened.
const STARTUP_GRACE: Duration = Duration::from_secs(10);

/// Minimum time between real IMAP checks with no failures — the poller's
/// base tick interval (PR B pinned requirement #2).
const BASE_CHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Backoff ceiling — a persistently-failing mailbox never waits longer than
/// this between retries.
const MAX_BACKOFF: Duration = Duration::from_secs(2 * 60 * 60);

/// How far back `SINCE` looks on every real check — see
/// [`imap_client::LOOKBACK_DAYS`] for the rationale (a uniform bound rather
/// than a UID-range query).
const LOOKBACK_DAYS: i64 = imap_client::LOOKBACK_DAYS;

/// Effective wait between real IMAP checks: [`BASE_CHECK_INTERVAL`], doubled
/// once per consecutive failure and capped at [`MAX_BACKOFF`]. In-memory only
/// (resets to 0 on restart) — machine-local ephemera, not worth persisting;
/// a fresh boot starting at the base interval is a safe, conservative
/// default rather than a correctness requirement.
fn backoff_interval(consecutive_failures: u32) -> Duration {
    let doublings = consecutive_failures.min(4); // 15m << 4 = 4h, already > MAX_BACKOFF
    let secs = BASE_CHECK_INTERVAL
        .as_secs()
        .saturating_mul(1u64 << doublings);
    Duration::from_secs(secs).min(MAX_BACKOFF)
}

/// Whether enough time has passed since `last_check_ms` (the last ATTEMPT,
/// not the last success — see the module doc) for another real IMAP check,
/// given the current backoff state. `None` (never checked) is always due.
fn is_due(last_check_ms: Option<u64>, consecutive_failures: u32, now_ms: u64) -> bool {
    match last_check_ms {
        None => true,
        Some(last) => {
            now_ms.saturating_sub(last) >= backoff_interval(consecutive_failures).as_millis() as u64
        }
    }
}

/// Spawn the background check loop. Mirrors `autopilot_scheduler::start`'s
/// shape: `tauri::async_runtime::spawn` (never bare `tokio::spawn` — there is
/// no reactor before Tauri's own setup completes), a startup-grace sleep,
/// then an immediate first check followed by an interval loop.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_GRACE).await;

        let mut consecutive_failures: u32 = 0;
        tick(&app, &mut consecutive_failures).await;

        let mut interval = tokio::time::interval(TICK_INTERVAL);
        interval.tick().await; // consume the immediate tick (first check already ran)
        loop {
            interval.tick().await;
            tick(&app, &mut consecutive_failures).await;
        }
    });
}

async fn tick(app: &AppHandle, consecutive_failures: &mut u32) {
    let Some(store) = app.try_state::<EmailWatchStore>() else {
        return;
    };
    let account = store.account();
    if !account.enabled || account.address.is_none() {
        return;
    }
    if !is_due(account.last_check_ms, *consecutive_failures, now_ms()) {
        return;
    }
    match classify_tick_outcome(&run_check(app).await) {
        TickOutcome::Success => *consecutive_failures = 0,
        // A concurrent-run refusal (`RunGuard` losing to a manual
        // `check_now` already in flight) never even attempted the IMAP round
        // trip — it is neither a success nor a real failure, so it must not
        // inflate the backoff. Leave `consecutive_failures` untouched; the
        // next tick re-evaluates `is_due` against the SAME backoff state.
        TickOutcome::RateLimited => {}
        // Failures are swallowed here (best-effort, like Layer A) — the
        // scheduler retries next tick, bounded by the backoff above.
        TickOutcome::Failure => *consecutive_failures = consecutive_failures.saturating_add(1),
    }
}

/// How a completed [`run_check`] call should affect `consecutive_failures` —
/// factored out of [`tick`] as a pure classification so it's directly
/// unit-testable without a live `AppHandle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TickOutcome {
    Success,
    /// A concurrent-run refusal (`RunGuard`/the 60 s min-gap guard) — no
    /// real attempt was made, so this must be treated as neither success nor
    /// failure.
    RateLimited,
    Failure,
}

fn classify_tick_outcome(result: &AppResult<EmailWatchStatus>) -> TickOutcome {
    match result {
        Ok(_) => TickOutcome::Success,
        Err(AppError::RateLimited(_)) => TickOutcome::RateLimited,
        Err(_) => TickOutcome::Failure,
    }
}

/// Process-global in-flight flag backing [`RunGuard`] — mirrors
/// `commands::autopilot::RUNS_IN_FLIGHT`, simplified to a single flag (not a
/// per-id `HashSet`) since there is only ever ONE configured mailbox. Process-
/// local/transient (holds no user data), so a module static rather than
/// managed Tauri state.
static RUN_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// The exact rejection text surfaced to the renderer whenever a check
/// (scheduled or manual) is refused — either because one is already in
/// flight ([`RunGuard`]) or because one ran too recently
/// (`commands::email_watch::email_watch_check_now`'s own 60 s min-gap
/// guard). Both return this SAME string over IPC — `AppError` serializes as
/// plain text, so the renderer discriminates by an exact string match (see
/// `EmailWatchSection`'s `CHECK_NOW_RATE_LIMIT_MESSAGE`); do not change this
/// text without updating that renderer constant too.
pub const RATE_LIMITED_MESSAGE: &str = "a check already ran recently — try again in a moment";

/// RAII claim on [`RUN_IN_FLIGHT`], mirroring `commands::autopilot::
/// RunGuard`. [`RunGuard::try_acquire`] returns `None` when a check is
/// already in flight (the caller must refuse, never queue/wait); dropping
/// the returned guard clears the flag, so the claim is released on EVERY
/// exit path — a normal return, an early `?`, or a panic unwind.
struct RunGuard;

impl RunGuard {
    fn try_acquire() -> Option<RunGuard> {
        RUN_IN_FLIGHT
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| RunGuard)
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        RUN_IN_FLIGHT.store(false, Ordering::Release);
    }
}

/// Run one full fetch+parse+match+notify pass against the currently
/// configured mailbox, and stamp `last_check_ms` regardless of outcome (so
/// [`is_due`]'s elapsed-time gate is measured from the last ATTEMPT, not the
/// last success — otherwise a failing mail host would make every internal
/// [`TICK_INTERVAL`] wake-up re-attempt immediately instead of respecting the
/// backoff). Shared by the scheduler's own tick and the manual
/// `email_watch_check_now` command. Refuses immediately (never queues) when
/// a check is already in flight — see [`RunGuard`].
pub async fn run_check(app: &AppHandle) -> AppResult<EmailWatchStatus> {
    let store = app
        .try_state::<EmailWatchStore>()
        .ok_or_else(|| AppError::Storage("email watch is unavailable".to_string()))?;

    let Some(_guard) = RunGuard::try_acquire() else {
        return Err(AppError::RateLimited(RATE_LIMITED_MESSAGE.to_string()));
    };

    let outcome = run_check_inner(app, &store).await;
    if let Err(e) = store.record_check(now_ms()) {
        log::warn!("[email_watch] failed to record the check timestamp: {e}");
    }
    outcome?;
    Ok(store.status())
}

async fn run_check_inner(app: &AppHandle, store: &EmailWatchStore) -> AppResult<()> {
    let account = store.account();
    let address = account
        .address
        .ok_or_else(|| AppError::Config("no email account is connected".to_string()))?;
    let host = account
        .host
        .unwrap_or_else(|| DEFAULT_IMAP_HOST.to_string());
    let port = account.port.unwrap_or(DEFAULT_IMAP_PORT);
    let app_password = app
        .try_state::<Mutex<CredentialStore>>()
        .ok_or_else(|| AppError::Storage("credential store is unavailable".to_string()))?
        .lock()
        .get_decrypted(CREDENTIAL_SLOT)
        .map(|(_, password)| password)
        .ok_or_else(|| {
            AppError::Config("no app password is stored for this account".to_string())
        })?;

    let saved: Vec<Application> = app
        .try_state::<ApplicationStore>()
        .map(|s| s.list())
        .unwrap_or_default()
        .into_iter()
        .filter(|a| a.status == ApplicationStatus::Saved)
        .collect();
    let saved_for_notify = saved.clone();

    let since = Local::now().date_naive() - chrono::Duration::days(LOOKBACK_DAYS);
    let stored_uidvalidity = account.uidvalidity;
    let stored_last_uid = account.last_uid;

    let tick = match tokio::task::spawn_blocking(move || {
        poller::run_tick(
            &host,
            port,
            &address,
            &app_password,
            since,
            stored_uidvalidity,
            stored_last_uid,
            &saved,
        )
    })
    .await
    {
        Ok(result) => result?,
        Err(_) => {
            // Fixed-string log — a `JoinError`'s `Display` can echo the panic
            // payload, which (unlike `imap`'s own errors) has no guarantee of
            // being content-free.
            log::warn!("[email_watch] tick task panicked");
            return Err(AppError::Message(
                "email check failed unexpectedly".to_string(),
            ));
        }
    };

    // Re-check the account is STILL connected right before writing/notifying
    // anything — a Disconnect or a factory reset landing during the
    // multi-second `spawn_blocking` IMAP round trip above must suppress BOTH
    // the writes (already guarded at the DB layer above — this store method
    // resolved just now, so a race after this point is vanishingly narrow)
    // AND the notification, which has NO DB awareness of its own and would
    // otherwise fire a "Possible application confirmation" card sourced from
    // a mailbox the user just disconnected. Bail silently — nothing to
    // report for an account that no longer exists.
    if store.account().address.is_none() {
        return Ok(());
    }

    if tick.uidvalidity_changed {
        store.reset_on_uidvalidity_change(tick.uidvalidity)?;
    }

    // Seed from the SAME effective-watermark decision `poller::run_tick`
    // itself used (not the raw pre-tick `stored_last_uid` snapshot): after a
    // UIDVALIDITY change, the old value is meaningless against the new
    // numbering — using it here would risk `advance_last_uid` writing a
    // stale/too-high bound that silently suppresses every real message under
    // the new numbering. Reuses `poller::effective_last_uid` rather than a
    // second copy of the same if/else, so there is exactly one decision.
    let mut max_uid = poller::effective_last_uid(tick.uidvalidity_changed, stored_last_uid);
    for outcome in &tick.outcomes {
        max_uid = Some(max_uid.map_or(outcome.uid, |m| m.max(outcome.uid)));
        let uid_key = outcome.uid.to_string();
        if store.has_seen(&uid_key) {
            continue; // already considered on a previous tick — never re-notify
        }
        // Stamp `seen` FIRST, then notify — so a crash/failure between the
        // two never leaves a match un-deduped on the next tick.
        store.mark_seen(
            &uid_key,
            outcome.matched_application_id.as_deref(),
            now_ms(),
        )?;
        if let Some(app_id) = &outcome.matched_application_id {
            if let Some(matched) = saved_for_notify.iter().find(|a| &a.id == app_id) {
                notify_match(app, matched);
            }
        }
    }
    if let Some(uid) = max_uid {
        store.advance_last_uid(uid)?;
    }

    Ok(())
}

fn notify_match(app: &AppHandle, matched: &Application) {
    let mut search = serde_json::Map::new();
    search.insert(
        "highlight".to_string(),
        serde_json::Value::String(matched.id.clone()),
    );
    let body = if matched.title.trim().is_empty() {
        matched.company.clone()
    } else {
        format!("{} · {}", matched.title, matched.company)
    };
    crate::commands::notifications::push_and_notify(
        app,
        crate::notifications::NewNotification {
            kind: "email.match".to_string(),
            title: "Possible application confirmation".to_string(),
            body,
            route: Some(crate::notifications::NotificationRoute {
                to: "/applications".to_string(),
                search: Some(search),
            }),
        },
        crate::commands::notifications::OsBanner::WhenUnfocused,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── RATE_LIMITED_MESSAGE ↔ renderer sentinel parity ─────────────────────
    //
    // `RATE_LIMITED_MESSAGE` and the renderer's `CHECK_NOW_RATE_LIMIT_MESSAGE`
    // (`EmailWatchSection/index.tsx`) are independent literals linked only by
    // a comment on each side — an `AppError` serializes as plain text over
    // IPC, so the renderer discriminates the friendly-copy case by an EXACT
    // string match. Editing either alone would pass every test while
    // silently breaking that match. Mirrors `extension_bridge::test::
    // message_type_constants_match_ts`'s TS-source-as-text parity approach.

    /// Path from this crate's manifest dir (`apps/desktop/src-tauri`) to the
    /// renderer file hard-coding the same sentinel text.
    const RATE_LIMIT_RENDERER_SOURCE: &str =
        "../src/renderer/features/settings/components/accounts/EmailWatchSection/index.tsx";

    #[test]
    fn rate_limited_message_matches_the_renderer_sentinel() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(RATE_LIMIT_RENDERER_SOURCE);
        let ts = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
        let needle = format!("CHECK_NOW_RATE_LIMIT_MESSAGE = '{RATE_LIMITED_MESSAGE}';");
        assert!(
            ts.contains(&needle),
            "Rust RATE_LIMITED_MESSAGE ({RATE_LIMITED_MESSAGE:?}) not found as `{needle}` \
             in EmailWatchSection/index.tsx's CHECK_NOW_RATE_LIMIT_MESSAGE — the two \
             sentinels are independent literals and must be edited together"
        );
    }

    #[test]
    fn backoff_doubles_per_failure_and_caps_at_max_backoff() {
        assert_eq!(backoff_interval(0), BASE_CHECK_INTERVAL);
        assert_eq!(backoff_interval(1), Duration::from_secs(30 * 60));
        assert_eq!(backoff_interval(2), Duration::from_secs(60 * 60));
        assert_eq!(backoff_interval(3), Duration::from_secs(2 * 60 * 60));
        // 4 doublings would be 4h > the 2h cap.
        assert_eq!(backoff_interval(4), MAX_BACKOFF);
        // Never grows past the cap no matter how many consecutive failures.
        assert_eq!(backoff_interval(50), MAX_BACKOFF);
    }

    #[test]
    fn never_checked_is_always_due() {
        assert!(is_due(None, 0, now_ms()));
        assert!(is_due(None, 5, now_ms()));
    }

    #[test]
    fn not_due_before_the_base_interval_elapses() {
        let now = 1_000_000_000_000u64;
        let last = now - Duration::from_secs(5 * 60).as_millis() as u64; // 5 min ago
        assert!(!is_due(Some(last), 0, now));
    }

    #[test]
    fn due_once_the_base_interval_has_elapsed_with_no_failures() {
        let now = 1_000_000_000_000u64;
        let last = now - BASE_CHECK_INTERVAL.as_millis() as u64;
        assert!(is_due(Some(last), 0, now));
    }

    #[test]
    fn backoff_delays_due_ness_past_the_base_interval_after_failures() {
        let now = 1_000_000_000_000u64;
        // 20 minutes since the last attempt: past the 15 min base interval,
        // but well under the 30 min (1-failure) backoff interval.
        let last = now - Duration::from_secs(20 * 60).as_millis() as u64;
        assert!(
            is_due(Some(last), 0, now),
            "no failures — base interval alone gates it"
        );
        assert!(
            !is_due(Some(last), 1, now),
            "one failure — 30 min backoff not yet elapsed"
        );
    }

    // ── classify_tick_outcome (/review LOW) ─────────────────────────────────

    #[test]
    fn classify_tick_outcome_success_resets_and_rate_limited_is_distinct_from_failure() {
        let ok: AppResult<EmailWatchStatus> = Ok(EmailWatchStatus::default());
        assert_eq!(classify_tick_outcome(&ok), TickOutcome::Success);

        let rate_limited: AppResult<EmailWatchStatus> = Err(AppError::RateLimited(
            "a check already ran recently".to_string(),
        ));
        assert_eq!(
            classify_tick_outcome(&rate_limited),
            TickOutcome::RateLimited,
            "a concurrent-run refusal must not classify as a real failure"
        );

        let real_failure: AppResult<EmailWatchStatus> = Err(AppError::Network(
            "could not connect to the mail server".to_string(),
        ));
        assert_eq!(classify_tick_outcome(&real_failure), TickOutcome::Failure);
    }

    // ── concurrent-run guard (rust-backend-architect HIGH) ─────────────────
    // `RUN_IN_FLIGHT` is a single process-global flag (not per-id, unlike
    // autopilot's), so these two tests share state and MUST run serially
    // relative to each other or they'd flake against the parallel test runner.

    #[test]
    #[serial_test::serial]
    fn run_guard_blocks_a_second_concurrent_acquire() {
        let first = RunGuard::try_acquire().expect("first acquire succeeds");
        assert!(
            RunGuard::try_acquire().is_none(),
            "a second acquire while one is in flight is blocked (no concurrent runs)"
        );
        drop(first);
        assert!(
            RunGuard::try_acquire().is_some(),
            "after the first guard drops, a new acquire succeeds"
        );
    }

    #[test]
    #[serial_test::serial]
    fn run_guard_releases_on_drop_even_after_repeated_acquire_attempts() {
        let guard = RunGuard::try_acquire().expect("acquire succeeds");
        // Several refused attempts while held must not corrupt the flag —
        // each is a no-op read, not a competing claim.
        for _ in 0..3 {
            assert!(RunGuard::try_acquire().is_none());
        }
        drop(guard);
        assert!(RunGuard::try_acquire().is_some());
    }
}
