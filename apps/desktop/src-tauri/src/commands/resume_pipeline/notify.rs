//! The run-terminal notification (ADR-016) — the one place a finished staged
//! résumé run reaches the Notification Center.
//!
//! A quality run is minutes at best and up to 75 wall-clock minutes at the top
//! effort tier, so the user is normally somewhere else when it lands. Every
//! other long/background outcome in the app already announces itself the same
//! way (`autopilot.new_jobs`, `email.match`, `application.follow_up`,
//! `import.result`, `ai.generation_truncated`), and this is that flow's entry.
//!
//! ## Copy is English literals, not i18n keys
//!
//! Checked, not assumed: `NewNotification.title`/`body` are rendered verbatim by
//! the bell, the in-app toast and the OS banner, and every existing backend
//! source pushes plain English (see `reminder_scheduler::TITLE`'s note). There is
//! no Rust-side catalogue to key into, so a "key" here would render as a raw
//! token. Nothing in `packages/translations` is involved.
//!
//! ## The decision is pure
//!
//! [`run_notification`] takes no `AppHandle` for the same reason
//! `hooks::terminal_state` and `hooks::apply_stop` do not: this crate has no
//! Tauri test harness, and a decision only provable by reading the code is not a
//! guard. [`notify_terminal`] is the two-line side effect around it.

use tauri::AppHandle;

use crate::commands::match_resume::JobPostingMeta;
use crate::commands::notifications::OsBanner;
use crate::notifications::{NewNotification, NotificationRoute};

/// Notification `kind` for a finished staged résumé run. An OPEN string, like
/// every other source — a new kind needs no codebase change beyond picking one.
const KIND: &str = "resume.pipeline_run";

/// Where the bell row and the OS banner navigate.
///
/// The SURFACE, not the run: there is no per-run route (the report panel and the
/// runs list live in the job detail pane's TailoredResumePanel on the jobs
/// page), and the renderer's `resolveNotificationRoute` falls back to `/` for
/// anything outside the router's static union — so an invented deep link would
/// silently land the user on the dashboard instead. A job-scoped search param
/// can be added here later without changing anything else.
const ROUTE: &str = "/jobs";

const TITLE_COMPLETED: &str = "Résumé ready";
/// `needsReview` is NOT a failure and must not read like one — the document
/// exists and is usable; what is outstanding is a per-bullet verdict.
const TITLE_NEEDS_REVIEW: &str = "Résumé ready — needs review";
const TITLE_FAILED: &str = "Résumé run failed";
const TITLE_CANCELLED: &str = "Résumé run cancelled";

/// Char budget for the SCRAPED half of the body.
///
/// The label is attacker-influenceable (a posting title is scraped text) and it
/// comes FIRST in every body below, while the store clamps the whole body to
/// [`crate::notifications::MAX_BODY_CHARS`] — so an unbounded label does not
/// merely look bad, it consumes the entire clamp and DISPLACES the fixed clause
/// that says what happened ("2 flagged claims need a Keep or Remove decision"),
/// leaving a card that is pure attacker text. Sized so every clause below still
/// fits with room to spare; a title longer than this is not a title.
const LABEL_CAP: usize = 160;

/// `"<title> · <company>"`, degrading to whichever side the cached posting has —
/// the same body shape the email-watch card and the follow-up reminder use, so
/// the inbox reads consistently. Local rather than hoisted out of
/// `reminder_scheduler`: the fallback text differs per surface, and the shape is
/// six lines.
///
/// Clamped to [`LABEL_CAP`] with an ellipsis, char-boundary safe (the store's
/// own clamp counts characters too, for the same multi-byte reason).
fn posting_label(title: &str, company: &str) -> String {
    let label = match (title.trim(), company.trim()) {
        ("", "") => "Untitled posting".to_string(),
        ("", company) => company.to_string(),
        (title, "") => title.to_string(),
        (title, company) => format!("{title} · {company}"),
    };
    if label.chars().count() <= LABEL_CAP {
        return label;
    }
    label.chars().take(LABEL_CAP - 1).chain(['…']).collect()
}

/// What is left to do on a run that ended in `needsReview`.
///
/// The ZERO case is real, not defensive: a run also stays in review when the
/// only blocker is a Critical the Keep/Remove panel cannot clear
/// (`factual.dropped_role` names an ABSENCE, so it is deliberately not listed —
/// see `report::still_needs_review`). Announcing "0 flagged claims need a
/// decision" there would be both wrong and useless.
fn review_note(unresolved: usize) -> String {
    match unresolved {
        0 => "the integrity report has a finding the per-bullet review cannot clear.".to_string(),
        1 => "1 flagged claim needs a Keep or Remove decision.".to_string(),
        n => format!("{n} flagged claims need a Keep or Remove decision."),
    }
}

/// The notification a run in `status` should raise, or `None` when there is
/// nothing to announce.
///
/// `None` for a non-terminal status (`running`) and for any token this module
/// does not know: the four terminal states are the contract's whole vocabulary,
/// and inventing copy for a fifth one would announce something nobody can act
/// on.
///
/// `unresolved` is [`super::report::unresolved_count`] over the PERSISTED
/// wrapper — the same number the review panel shows — and is read only on the
/// `needsReview` branch.
pub(crate) fn run_notification(
    status: &str,
    unresolved: usize,
    job_title: &str,
    company: &str,
) -> Option<NewNotification> {
    let posting = posting_label(job_title, company);
    let (title, body) = match status {
        super::STATUS_COMPLETED => (TITLE_COMPLETED, posting),
        super::STATUS_NEEDS_REVIEW => (
            TITLE_NEEDS_REVIEW,
            format!("{posting} — {}", review_note(unresolved)),
        ),
        // No provider/error text: this string is persisted to
        // `notifications.json` and can surface as an OS banner outside the app,
        // and a provider error can carry fragments of the prompt. The run row
        // and the job event both carry the actual message.
        super::STATUS_FAILED => (
            TITLE_FAILED,
            format!("{posting} — the run stopped before it produced a résumé you can use."),
        ),
        // Worth announcing even though the user asked for it: a cancel takes
        // effect at the next stage boundary, which can be several minutes after
        // the click if a provider call is in flight. This is "it has actually
        // stopped now".
        super::STATUS_CANCELLED => (TITLE_CANCELLED, format!("{posting} — the run has stopped.")),
        _ => return None,
    };
    Some(NewNotification {
        kind: KIND.to_string(),
        title: title.to_string(),
        body,
        route: Some(NotificationRoute {
            to: ROUTE.to_string(),
            search: None,
        }),
    })
}

/// Push the terminal notification for one finished run.
///
/// [`OsBanner::WhenUnfocused`] — the same policy as every source except
/// autopilot: the in-app toast already covers the focused case, and `Always` is
/// reserved for outcomes that arrive with no user in the loop at all. A silent
/// no-op for a non-terminal status.
pub(crate) fn notify_terminal(
    app: &AppHandle,
    status: &str,
    unresolved: usize,
    meta: &JobPostingMeta,
) {
    let Some(notification) = run_notification(status, unresolved, &meta.title, &meta.company)
    else {
        return;
    };
    crate::commands::notifications::push_and_notify(app, notification, OsBanner::WhenUnfocused);
}
