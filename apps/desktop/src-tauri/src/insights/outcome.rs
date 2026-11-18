//! Classify one application's outcome. Pure.

use super::{Outcome, DAY_MS, NO_REPLY_AFTER_DAYS};
use crate::applications::{
    ApplicationStatus, StatusEvent, EVENT_SOURCE_EMAIL, EVENT_SOURCE_EMAIL_REJECT,
};

fn is_positive_stage(status: &str) -> bool {
    matches!(status, "screening" | "interviewing" | "offer" | "accepted")
}

/// Did the application ever reach screening or beyond? True when the current
/// status is such a stage, or when a status-history row says it was.
///
/// An email-derived row counts only once the user has confirmed it and no later
/// reversal (the user rejecting that very suggestion) undid it; reversal rows
/// themselves never count, they only restore an earlier state, and a dismissed
/// email row (rejected without a revert, because the status had moved on) never counts.
pub fn ever_reached_positive(current: ApplicationStatus, events: &[StatusEvent]) -> bool {
    if is_positive_stage(current.as_id()) {
        return true;
    }
    events.iter().enumerate().any(|(i, e)| {
        if !is_positive_stage(&e.to_status) || e.source == EVENT_SOURCE_EMAIL_REJECT || e.dismissed
        {
            return false;
        }
        if e.source != EVENT_SOURCE_EMAIL {
            return true;
        }
        e.confirmed
            && !events[i + 1..].iter().any(|later| {
                later.source == EVENT_SOURCE_EMAIL_REJECT && later.from_status == e.to_status
            })
    })
}

/// The status that counts for outcomes. An email auto-write sets the row's status
/// at once and appends an unreviewed event, so while the status rests on such a
/// write it is a suggestion, not a fact: fall back to the last status that does not
/// (`Applied` when none exists). A reversal (the user rejecting a later suggestion)
/// restores an earlier status, and that status is only as trustworthy as the
/// transition that established it, so it is resolved by that transition's provenance.
pub fn effective_status(current: ApplicationStatus, events: &[StatusEvent]) -> ApplicationStatus {
    match events.last() {
        Some(last)
            if last.is_unconfirmed_email_write() || last.source == EVENT_SOURCE_EMAIL_REJECT =>
        {
            trusted_status(events)
        }
        _ => current,
    }
}

/// The status after `events`, counting only transitions the user stands behind.
fn trusted_status(events: &[StatusEvent]) -> ApplicationStatus {
    let Some((last, before)) = events.split_last() else {
        return ApplicationStatus::Applied;
    };
    if last.is_unconfirmed_email_write() || last.dismissed {
        return trusted_status(before);
    }
    if last.source == EVENT_SOURCE_EMAIL_REJECT {
        // A reversal walks back to `to_status`: trust it as much as whatever set it.
        return match before
            .iter()
            .rposition(|e| e.to_status == last.to_status && !e.dismissed)
        {
            Some(set_by) => trusted_status(&events[..=set_by]),
            None => ApplicationStatus::from_id(&last.to_status),
        };
    }
    ApplicationStatus::from_id(&last.to_status)
}

/// The outcome of one application, or `None` when it is excluded from every
/// count (`saved` was never sent, `withdrawn` was the user's own choice).
pub fn classify(
    status: ApplicationStatus,
    ever_positive: bool,
    applied_at: u64,
    now: u64,
) -> Option<Outcome> {
    use ApplicationStatus as S;
    match status {
        S::Saved | S::Withdrawn => None,
        _ if ever_positive => Some(Outcome::PositiveReply),
        S::Rejected => Some(Outcome::Rejected),
        S::Ghosted => Some(Outcome::NoReply),
        S::Applied if now.saturating_sub(applied_at) > NO_REPLY_AFTER_DAYS * DAY_MS => {
            Some(Outcome::NoReply)
        }
        S::Applied => Some(Outcome::Pending),
        // Screening and later imply `ever_positive`; kept total rather than unreachable.
        S::Screening | S::Interviewing | S::Offer | S::Accepted => Some(Outcome::PositiveReply),
    }
}
