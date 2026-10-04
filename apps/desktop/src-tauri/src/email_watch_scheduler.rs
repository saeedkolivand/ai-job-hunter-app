//! Background scheduler for email-confirmation watching (Task #23, auto-track
//! Layer C). Mirrors [`crate::autopilot_scheduler`]'s split from its L1
//! store: `email_watch` (the store + connector + pure parse/match/poll logic)
//! stays Tauri-free, and THIS module (L2) is the one place in the whole
//! family that reaches up into `commands::notifications::push_and_notify`
//! (L3) — the same upward shell-reach `autopilot_scheduler` already has for
//! `commands::autopilot::autopilot_run`, via its own `R7_ALLOW` entry rather
//! than growing one on the L1 store.
//!
//! **v2 slice 3**: [`run_check_inner`]'s per-outcome loop is also where
//! [`crate::email_watch::auto_write::apply_matched_intent`] gets its ONE
//! runtime caller (ADR-0013's own text names this module, not `poller`, as
//! the wiring site — `poller::run_tick` stays pure matching/classification;
//! this file is the only place with both the `ApplicationStore` handle and
//! the license to write). Same ordering the store's own doc requires: after
//! `mark_seen` (dedupe always lands first) and right alongside
//! `notify_match` (both read the same matched application).
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

use crate::applications::{Application, ApplicationStore};
use crate::credentials::CredentialStore;
use crate::db::now_ms;
use crate::email_watch::imap_client::{self, DEFAULT_IMAP_HOST, DEFAULT_IMAP_PORT};
use crate::email_watch::intent::EmailIntent;
use crate::email_watch::{poller, EmailWatchStatus, EmailWatchStore, CREDENTIAL_SLOT};
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, APPLICATIONS_CHANGED};
use crate::observability::sanitize_reason;

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
        log::warn!(
            "[email_watch] failed to record the check timestamp: {}",
            sanitize_reason(&e.to_string())
        );
    }
    outcome?;
    Ok(store.status())
}

/// Whether `run_check_inner` may proceed to commit this tick's outcomes
/// (reset UIDVALIDITY, stamp `seen`, advance the watermark) AND fire any
/// notifications, given whether the account is STILL connected right after
/// the multi-second `spawn_blocking` IMAP round trip returned.
///
/// Every post-tick write is ALSO separately guarded at the DB layer
/// (`address IS NOT NULL` on `advance_last_uid`/`mark_seen`/
/// `reset_on_uidvalidity_change`) — but `notify_match` has NO database
/// awareness of its own, so THIS decision is the only thing standing between
/// a vanished account (a Disconnect or factory reset landing mid-tick) and a
/// stale notification card sourced from a mailbox the user just
/// disconnected. Pure so the decision itself is directly unit-tested, not
/// just exercised indirectly via the DB-layer no-op guards — mirrors this
/// file's own `is_due`/`backoff_interval`/`classify_tick_outcome` pattern.
fn should_commit_outcomes(account_still_connected: bool) -> bool {
    account_still_connected
}

async fn run_check_inner(app: &AppHandle, store: &EmailWatchStore) -> AppResult<()> {
    let account = store.account();
    let address = account
        .address
        .ok_or_else(|| AppError::Config("no email account is connected".to_string()))?;
    let host = account
        .host
        .unwrap_or_else(|| DEFAULT_IMAP_HOST.to_string());
    // Computed BEFORE `host` moves into the `spawn_blocking` closure below
    // -- consulted again at the auto-write gate near the bottom of this
    // fn, well past that move. See `parser::host_is_known_to_stamp`'s doc:
    // this is the write-gate's ONLY signal that isn't message content an
    // attacker can influence -- it closes the "lone forged
    // Authentication-Results header, host never stamped a genuine one"
    // residual for a known-host population.
    let write_gate_host_ok = crate::email_watch::parser::host_is_known_to_stamp(&host);
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

    // Kept (not just consumed into `candidates`/`candidates_for_notify`) —
    // the post-tick loop below needs the live store for the auto-write call
    // AND for the eligibility set below.
    //
    // Candidacy is no longer "status == Saved" — see
    // `email_watch::matcher`'s module doc. The candidate list is every
    // application (live statuses are unconditionally eligible), plus
    // `unconfirmed_email_write_ids` telling `poller::run_tick` (which
    // threads it straight through to `matcher::best_match`) which of the
    // TERMINAL ones are themselves still-unconfirmed email-derived writes
    // — the one case a terminal status stays a candidate. This is the only
    // place with the `ApplicationStore` handle needed to compute that set,
    // which is why it's computed here rather than inside the pure L1
    // matcher/poller.
    let applications = app.try_state::<ApplicationStore>();
    let candidates: Vec<Application> = applications
        .as_deref()
        .map(|s| s.list())
        .unwrap_or_default();
    let unconfirmed_email_write_ids: std::collections::HashSet<String> = applications
        .as_deref()
        .map(|s| {
            candidates
                .iter()
                .filter(|a| s.current_status_is_unconfirmed_email_write(&a.id))
                .map(|a| a.id.clone())
                .collect()
        })
        .unwrap_or_default();
    let candidates_for_notify = candidates.clone();

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
            &candidates,
            &unconfirmed_email_write_ids,
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
    // otherwise fire a card (title now chosen by `outcome.intent` — see
    // `notify_match`) sourced from a mailbox the user just disconnected.
    // Bail silently — nothing to
    // report for an account that no longer exists. The decision itself is a
    // pure fn (see `should_commit_outcomes`) so it's directly unit-tested,
    // not just exercised indirectly via the DB-layer no-op guards.
    if !should_commit_outcomes(store.account().address.is_some()) {
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
            if let Some(matched) = candidates_for_notify.iter().find(|a| &a.id == app_id) {
                notify_match(app, matched, outcome.intent);
                // v2 slice 3: the actual auto-write. `apply_matched_intent`
                // reads the LIVE status itself (see its own doc) rather than
                // taking one from this loop — two matched messages for the
                // SAME application in one tick (an ordinary ATS thread: a
                // confirmation then a later rejection inside one 15-minute
                // window) used to both receive the SAME pre-tick snapshot
                // here, so the second outcome's compare-and-set raced a
                // status the FIRST outcome had already moved, lost, and was
                // silently dropped forever (its uid already stamped by
                // `mark_seen`, above). Best-effort: a write failure here
                // (e.g. a transient SQLite contention) must not block
                // `mark_seen`/`advance_last_uid` for the REST of this tick's
                // outcomes — `.code()` only (never `.to_string()`/`{e}`), so
                // a "application not found: <id>"-shaped message can't leak
                // through this log line.
                if let Some(applications) = applications.as_deref() {
                    // `outcome.write_authorized` is message-content-derived
                    // (write-gate domain + DMARC pass) and, on its own,
                    // cannot tell a genuinely non-stamping host apart from
                    // one where an attacker's forged header is the ONLY
                    // `Authentication-Results` present -- see
                    // `parser::host_is_known_to_stamp`'s doc. ANDing in
                    // `write_gate_host_ok` (computed above from the
                    // account's own locally-stored `host`, BEFORE it moved
                    // into the tick's `spawn_blocking` closure) closes that
                    // gap for a known-host population.
                    match crate::email_watch::auto_write::apply_matched_intent(
                        applications,
                        store,
                        app_id,
                        outcome.intent,
                        outcome.write_authorized && write_gate_host_ok,
                    ) {
                        // A live application row/status_events row just changed
                        // underneath whatever's on screen right now (e.g. an open
                        // `/applications/$id` detail/timeline page) — the same
                        // event every OTHER backend-initiated write already emits
                        // (`extension_bridge::import_flow`,
                        // `extension_bridge::status_update`). Without it, the
                        // provisional-badge/Accept/Reject UI that makes an
                        // unconfirmed write visible and adjudicable never appears
                        // until the user navigates away and back — silently
                        // defeating the one property ADR-0013 relies on to call
                        // this gate's residual risk acceptable. `Ok(false)` is a
                        // gated no-op (opt-in off, not authorized, no intent, no
                        // valid transition, rejected target, or a lost CAS race)
                        // and must stay silent — emitting for a write that never
                        // happened would be its own (milder) lie to the UI.
                        Ok(true) => {
                            emit_event(
                                app,
                                APPLICATIONS_CHANGED,
                                serde_json::json!({ "applicationId": app_id }),
                            );
                        }
                        Ok(false) => {}
                        Err(e) => {
                            log::warn!(
                                "[email_watch] auto-write failed for a matched application \
                                 (non-fatal): {}",
                                e.code()
                            );
                        }
                    }
                }
            }
        }
    }
    if let Some(uid) = max_uid {
        store.advance_last_uid(uid)?;
    }

    Ok(())
}

/// Title reflects `intent`, not just "a match happened" — this used to be
/// hardcoded "Possible application confirmation" from when the matcher only
/// ever considered `Saved` applications (a confirmation was the only
/// plausible outcome for those). Candidacy widened to every `is_actionable`
/// status (see `matcher`'s own doc), so the SAME card now also has to speak
/// for a rejection reaching an `Interviewing` application, an interview
/// invite, or an offer — announcing all of those as "confirmation" would be
/// actively misleading, not just imprecise. `intent` is `None` when the
/// email matched a candidate but the classifier couldn't place it in one of
/// the four buckets — kept generic rather than guessed. Every case stays
/// hedged ("Possible" / "?"): this is a pattern match on email content, not
/// a verified outcome — the uncertainty lives in the CLASSIFICATION, not
/// just in "did this email belong to this application".
fn notify_title(intent: Option<EmailIntent>) -> &'static str {
    match intent {
        Some(EmailIntent::Confirmation) => "Possible application confirmation",
        Some(EmailIntent::Rejection) => "Possible rejection notice",
        Some(EmailIntent::Interview) => "Possible interview invite",
        Some(EmailIntent::Offer) => "Possible offer notice",
        None => "Possible application update",
    }
}

fn notify_match(app: &AppHandle, matched: &Application, intent: Option<EmailIntent>) {
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
            title: notify_title(intent).to_string(),
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
mod tests;
