//! Background sweep that turns a due `next_action_at` into a notification.
//!
//! Mirrors [`crate::email_watch_scheduler`]'s split from its L1 store: the
//! `applications` aggregate (L1) stays Tauri-free and owns only the data
//! (`follow_up_candidates` / `mark_next_action_notified`), and THIS module (L2)
//! is the one that spawns from Tauri `setup` and reaches up into
//! `commands::notifications::push_and_notify` (L3) — via its own `R7_ALLOW`
//! entries rather than growing one on the store.
//!
//! Cadence: one sweep [`STARTUP_GRACE`] after launch (so a reminder that came due
//! while the app was closed surfaces on the next start), then every
//! [`TICK_INTERVAL`]. There is no network or IMAP work here — a sweep is one
//! indexed SQLite read — so it needs no backoff, unlike the email watcher.
//!
//! Dedupe: each row carries a `next_action_notified_at` marker set right before
//! its notification is pushed and cleared by
//! [`crate::applications::ApplicationStore::update_fields`] whenever the due date
//! moves. So one application notifies exactly ONCE per due date, and a
//! rescheduled follow-up notifies again. [`should_notify`] is the pure decision;
//! [`due_follow_ups`] is the pure per-sweep selection (both unit-tested without a
//! live `AppHandle`).

use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::applications::{ApplicationStore, FollowUpCandidate};
use crate::db::now_ms;
use crate::observability::sanitize_reason;

/// How often the sweep runs. A follow-up reminder is a day-grained thing, so a
/// coarse tick is plenty and keeps the app idle-cheap.
const TICK_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Grace period after launch before the first sweep, so the window and stores
/// finish coming up first (mirrors the sibling schedulers).
const STARTUP_GRACE: Duration = Duration::from_secs(20);

/// Upper bound on notifications raised by ONE sweep. Without it, the first sweep
/// after an upgrade could fire one banner per already-overdue application at
/// once. The remainder is not lost — it stays unmarked and surfaces on the next
/// tick, oldest-due first.
const MAX_PER_SWEEP: usize = 5;

/// Notification `kind` for a due follow-up (open string, see
/// [`crate::notifications::AppNotification`]).
const KIND: &str = "application.follow_up";

/// Title of the follow-up notification. English literal, exactly like every
/// other backend-raised notification (`email.match`, autopilot, import) — there
/// is no Rust-side i18n catalogue; the renderer renders `title`/`body` verbatim.
const TITLE: &str = "Follow-up due";

/// Whether this application's follow-up should raise a notification right now.
///
/// Pure so the whole rule is directly unit-testable (mirrors
/// `email_watch_scheduler::is_due`). True only when all three hold:
/// - a reminder is set (`next_action_at`), and it is due or overdue;
/// - no notification was raised for THIS due date yet (`notified_at` is `None` —
///   the marker is cleared whenever the due date changes);
/// - the pursuit is not closed — a rejected/accepted/withdrawn application must
///   not nag. `ghosted` is deliberately NOT terminal (it can revive), so it
///   still reminds.
fn should_notify(c: &FollowUpCandidate, now: u64) -> bool {
    match c.next_action_at {
        None => false,
        Some(due) => due <= now && c.notified_at.is_none() && !c.status.is_terminal(),
    }
}

/// The (bounded) set one sweep notifies: everything [`should_notify`] accepts,
/// most-overdue first, capped at [`MAX_PER_SWEEP`].
fn due_follow_ups(mut candidates: Vec<FollowUpCandidate>, now: u64) -> Vec<FollowUpCandidate> {
    candidates.retain(|c| should_notify(c, now));
    // Oldest due date first, so the longest-overdue follow-ups win the cap.
    candidates.sort_by_key(|c| c.next_action_at.unwrap_or(u64::MAX));
    candidates.truncate(MAX_PER_SWEEP);
    candidates
}

/// Notification body for one due follow-up: `"<title> · <company>"`, degrading
/// to whichever side is present (same shape as the email-watch match card).
fn follow_up_body(title: &str, company: &str) -> String {
    match (title.trim(), company.trim()) {
        ("", "") => "Untitled application".to_string(),
        ("", company) => company.to_string(),
        (title, "") => title.to_string(),
        (title, company) => format!("{title} · {company}"),
    }
}

/// Spawn the sweep loop. `tauri::async_runtime::spawn` — never bare
/// `tokio::spawn`, which has no reactor when `setup` runs.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_GRACE).await;
        tick(&app).await;

        let mut interval = tokio::time::interval(TICK_INTERVAL);
        // `Delay`, not the default `Burst`: after the machine sleeps for hours,
        // Burst would fire every missed tick back-to-back — draining up to
        // MAX_PER_SWEEP notifications per catch-up tick in one blast. A reminder
        // sweep is idempotent and state-driven, so missed ticks are worthless;
        // resume the normal cadence from wake instead.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        interval.tick().await; // consume the immediate tick (first sweep already ran)
        loop {
            interval.tick().await;
            tick(&app).await;
        }
    });
}

/// The BLOCKING half of one sweep: read every candidate, then atomically claim
/// the ones that may be announced. Returns exactly the claimed candidates, in
/// sweep order — the caller notifies for those and nothing else.
///
/// Claim BEFORE notify (the email-watch `mark_seen` ordering) so a crash between
/// the two never re-announces the same due date. A `false` claim means the row
/// changed since the read — rescheduled, deleted, or moved to a terminal stage —
/// so the card is stale; drop it and let the next sweep evaluate the new state.
/// A failed claim likewise skips, rather than risking an un-deduped repeat every
/// 30 minutes.
///
/// Free function taking `&ApplicationStore` so the whole read→claim→select
/// pipeline is directly testable against a real store, without an `AppHandle`.
fn claim_due(store: &ApplicationStore, now: u64) -> Vec<FollowUpCandidate> {
    due_follow_ups(store.follow_up_candidates(), now)
        .into_iter()
        .filter(|c| {
            // `should_notify` only accepts a candidate with a due date, so this
            // is always `Some`; skip rather than unwrap if that stops holding.
            let Some(due_at) = c.next_action_at else {
                return false;
            };
            match store.mark_next_action_notified(&c.id, due_at, now_ms()) {
                Ok(true) => true,
                Ok(false) => {
                    log::debug!(
                        "[reminders] {} changed since the sweep read it — not notifying",
                        c.id
                    );
                    false
                }
                Err(e) => {
                    log::warn!(
                        "[reminders] could not mark {} notified (skipping): {}",
                        c.id,
                        sanitize_reason(&e.to_string())
                    );
                    false
                }
            }
        })
        .collect()
}

/// One sweep.
///
/// The storage half is cheap but genuinely BLOCKING — a `parking_lot` mutex, a
/// full scan of the reminder rows and up to [`MAX_PER_SWEEP`] one-row writes —
/// so it runs on `spawn_blocking`, never inline on a tokio worker (same split as
/// [`crate::email_watch_scheduler`]'s IMAP poll). Notification delivery stays on
/// the async side: `push_and_notify` does a `webview.is_focused()` main-thread
/// round trip per card, which must not happen inside a blocking pool task.
async fn tick(app: &AppHandle) {
    let handle = app.clone();
    let claimed = match tokio::task::spawn_blocking(move || {
        handle
            .try_state::<ApplicationStore>()
            .map(|store| claim_due(&store, now_ms()))
            .unwrap_or_default()
    })
    .await
    {
        Ok(claimed) => claimed,
        // Only a panic (or runtime shutdown) inside the closure gets here; the
        // sweep is idempotent, so dropping this one and retrying next tick is
        // the whole recovery.
        Err(e) => {
            log::warn!(
                "[reminders] sweep task failed: {}",
                sanitize_reason(&e.to_string())
            );
            return;
        }
    };
    for c in &claimed {
        notify_due(app, c);
    }
}

fn notify_due(app: &AppHandle, c: &FollowUpCandidate) {
    let mut search = serde_json::Map::new();
    // The renderer deep-links `/applications?highlight=<id>` (see
    // `lib/notification-route.ts`), the same intent the email-watch card uses.
    search.insert(
        "highlight".to_string(),
        serde_json::Value::String(c.id.clone()),
    );
    crate::commands::notifications::push_and_notify(
        app,
        crate::notifications::NewNotification {
            kind: KIND.to_string(),
            title: TITLE.to_string(),
            body: follow_up_body(&c.title, &c.company),
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
