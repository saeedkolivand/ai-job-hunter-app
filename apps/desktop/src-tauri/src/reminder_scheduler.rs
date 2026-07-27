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
        tick(&app);

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
            tick(&app);
        }
    });
}

/// One sweep. Synchronous and cheap (a single indexed SQLite read plus at most
/// [`MAX_PER_SWEEP`] one-row updates), so it runs inline on the scheduler task
/// rather than through `spawn_blocking`.
fn tick(app: &AppHandle) {
    let Some(store) = app.try_state::<ApplicationStore>() else {
        return;
    };
    for c in due_follow_ups(store.follow_up_candidates(), now_ms()) {
        // `should_notify` only accepts a candidate with a due date, so this is
        // always `Some`; skip rather than unwrap if that ever stops holding.
        let Some(due_at) = c.next_action_at else {
            continue;
        };
        // Stamp the marker FIRST, then notify — so a crash between the two
        // never re-announces the same due date (the email-watch `mark_seen`
        // ordering). The stamp is conditional on the due date this decision was
        // made against: `Ok(false)` means the user rescheduled (or deleted the
        // row) since the read, so this notification is stale — drop it and let
        // the next sweep evaluate the NEW date. A failed stamp likewise skips,
        // rather than risking an un-deduped repeat every 30 minutes.
        match store.mark_next_action_notified(&c.id, due_at, now_ms()) {
            Ok(true) => notify_due(app, &c),
            Ok(false) => log::debug!(
                "[reminders] {} changed since the sweep read it — not notifying",
                c.id
            ),
            Err(e) => log::warn!(
                "[reminders] could not mark {} notified (skipping): {e}",
                c.id
            ),
        }
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
mod tests {
    use super::*;
    use crate::applications::ApplicationStatus;

    const NOW: u64 = 1_800_000_000_000;

    fn candidate(id: &str, due: Option<u64>, notified: Option<u64>) -> FollowUpCandidate {
        FollowUpCandidate {
            id: id.to_string(),
            status: ApplicationStatus::Applied,
            title: "Engineer".to_string(),
            company: "Acme".to_string(),
            next_action_at: due,
            notified_at: notified,
        }
    }

    // ── should_notify ────────────────────────────────────────────────────────

    #[test]
    fn an_overdue_unnotified_reminder_fires() {
        assert!(should_notify(&candidate("a", Some(NOW - 1), None), NOW));
    }

    #[test]
    fn a_reminder_due_exactly_now_fires() {
        // The boundary is inclusive — a reminder set for this instant is due.
        assert!(should_notify(&candidate("a", Some(NOW), None), NOW));
    }

    #[test]
    fn a_future_reminder_does_not_fire() {
        assert!(!should_notify(&candidate("a", Some(NOW + 1), None), NOW));
    }

    #[test]
    fn an_unset_reminder_never_fires() {
        assert!(!should_notify(&candidate("a", None, None), NOW));
        // Even a stale marker on a cleared reminder stays silent.
        assert!(!should_notify(&candidate("a", None, Some(NOW - 1)), NOW));
    }

    #[test]
    fn an_already_notified_due_date_does_not_fire_again() {
        // The dedupe core: the marker is only cleared when the due date moves,
        // so a still-overdue reminder must NOT re-announce on every sweep.
        let c = candidate("a", Some(NOW - 10_000), Some(NOW - 5_000));
        assert!(!should_notify(&c, NOW));
    }

    #[test]
    fn rescheduling_clears_the_marker_and_the_new_date_fires_once() {
        // `update_fields` nulls `notified_at` when the due date changes; from
        // this function's side that is simply a due row with no marker again.
        let rescheduled = candidate("a", Some(NOW - 1), None);
        assert!(should_notify(&rescheduled, NOW));
        let after_firing = candidate("a", Some(NOW - 1), Some(NOW));
        assert!(!should_notify(&after_firing, NOW));
    }

    #[test]
    fn a_closed_pursuit_does_not_nag() {
        for status in [
            ApplicationStatus::Accepted,
            ApplicationStatus::Rejected,
            ApplicationStatus::Withdrawn,
        ] {
            let mut c = candidate("a", Some(NOW - 1), None);
            c.status = status;
            assert!(
                !should_notify(&c, NOW),
                "{status:?} is terminal — a stale reminder must stay silent"
            );
        }
        // `ghosted` is soft-terminal (a ghosted pursuit can revive), so it still
        // reminds — this is the exact `is_terminal` split.
        let mut ghosted = candidate("a", Some(NOW - 1), None);
        ghosted.status = ApplicationStatus::Ghosted;
        assert!(should_notify(&ghosted, NOW));
    }

    // ── due_follow_ups ───────────────────────────────────────────────────────

    #[test]
    fn a_sweep_selects_only_due_rows() {
        let selected = due_follow_ups(
            vec![
                candidate("due", Some(NOW - 1), None),
                candidate("future", Some(NOW + 60_000), None),
                candidate("already", Some(NOW - 1), Some(NOW - 1)),
                candidate("unset", None, None),
            ],
            NOW,
        );
        let ids: Vec<&str> = selected.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["due"]);
    }

    #[test]
    fn a_sweep_is_capped_and_takes_the_most_overdue_first() {
        // One more than the cap, deliberately handed over newest-first so the
        // sort (not the input order) decides.
        let candidates: Vec<FollowUpCandidate> = (0..=MAX_PER_SWEEP)
            .map(|i| candidate(&format!("app{i}"), Some(NOW - (i as u64 + 1) * 1000), None))
            .collect();
        let selected = due_follow_ups(candidates, NOW);
        assert_eq!(selected.len(), MAX_PER_SWEEP, "one sweep is bounded");
        assert_eq!(
            selected[0].id,
            format!("app{MAX_PER_SWEEP}"),
            "the longest-overdue reminder must win the cap"
        );
        // The one that missed the cap keeps no marker, so the next sweep takes it.
        assert!(
            !selected.iter().any(|c| c.id == "app0"),
            "the least-overdue row is deferred, not dropped"
        );
    }

    // ── follow_up_body ───────────────────────────────────────────────────────

    #[test]
    fn body_degrades_gracefully_when_a_side_is_missing() {
        assert_eq!(follow_up_body("Engineer", "Acme"), "Engineer · Acme");
        assert_eq!(follow_up_body("", "Acme"), "Acme");
        assert_eq!(follow_up_body("Engineer", "  "), "Engineer");
        assert_eq!(follow_up_body("  ", ""), "Untitled application");
    }
}
