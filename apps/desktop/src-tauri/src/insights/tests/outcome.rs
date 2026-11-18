use super::support::*;
use crate::applications::StatusEvent;
use crate::applications::{ApplicationStatus as S, EVENT_SOURCE_EMAIL, EVENT_SOURCE_EMAIL_REJECT};
use crate::insights::{classify, effective_status, ever_reached_positive, Outcome};

fn outcome_of(
    status: S,
    events: &[crate::applications::StatusEvent],
    applied_at: u64,
) -> Option<Outcome> {
    {
        let status = effective_status(status, events);
        classify(
            status,
            ever_reached_positive(status, events),
            applied_at,
            NOW,
        )
    }
}

#[test]
fn reached_screening_then_rejected_is_a_positive_reply() {
    let history = [
        user_event("applied"),
        user_event("screening"),
        user_event("rejected"),
    ];
    assert_eq!(
        outcome_of(S::Rejected, &history, NOW - DAY),
        Some(Outcome::PositiveReply)
    );
}

#[test]
fn reached_an_interview_then_ghosted_is_a_positive_reply() {
    let history = [
        user_event("applied"),
        user_event("interviewing"),
        user_event("ghosted"),
    ];
    assert_eq!(
        outcome_of(S::Ghosted, &history, NOW - 90 * DAY),
        Some(Outcome::PositiveReply)
    );
}

#[test]
fn a_current_positive_stage_needs_no_history() {
    for status in [S::Screening, S::Interviewing, S::Offer, S::Accepted] {
        assert_eq!(
            outcome_of(status, &[], NOW - DAY),
            Some(Outcome::PositiveReply),
            "{status:?}"
        );
    }
}

#[test]
fn rejected_without_ever_reaching_a_stage_is_rejected() {
    let history = [user_event("applied"), user_event("rejected")];
    assert_eq!(
        outcome_of(S::Rejected, &history, NOW - 5 * DAY),
        Some(Outcome::Rejected)
    );
}

#[test]
fn ghosted_without_a_positive_stage_is_no_reply() {
    let history = [user_event("applied"), user_event("ghosted")];
    assert_eq!(
        outcome_of(S::Ghosted, &history, NOW - 2 * DAY),
        Some(Outcome::NoReply)
    );
}

#[test]
fn saved_and_withdrawn_are_excluded_even_after_a_positive_stage() {
    let history = [user_event("screening"), user_event("withdrawn")];
    assert_eq!(outcome_of(S::Withdrawn, &history, NOW - DAY), None);
    assert_eq!(outcome_of(S::Saved, &[], NOW - DAY), None);
}

#[test]
fn applied_is_pending_until_strictly_past_twenty_one_days() {
    let on_the_line = NOW - 21 * DAY;
    assert_eq!(
        outcome_of(S::Applied, &[], on_the_line),
        Some(Outcome::Pending)
    );
    assert_eq!(
        outcome_of(S::Applied, &[], on_the_line - 1),
        Some(Outcome::NoReply)
    );
    assert_eq!(outcome_of(S::Applied, &[], NOW), Some(Outcome::Pending));
}

#[test]
fn an_applied_at_in_the_future_is_pending_not_an_underflow() {
    assert_eq!(
        outcome_of(S::Applied, &[], NOW + DAY),
        Some(Outcome::Pending)
    );
}

/// The real store shape: an email auto-write flips the status AND appends an unconfirmed
/// event, so the latest event always agrees with the current status.
#[test]
fn an_unconfirmed_email_suggestion_does_not_count_as_a_positive_reply() {
    let history = [
        user_event("applied"),
        event("screening", EVENT_SOURCE_EMAIL, false),
    ];
    assert_eq!(effective_status(S::Screening, &history), S::Applied);
    assert_eq!(
        outcome_of(S::Screening, &history, NOW - DAY),
        Some(Outcome::Pending)
    );
}

#[test]
fn an_unconfirmed_email_rejection_is_not_a_rejection_yet() {
    let history = [
        user_event("applied"),
        event("rejected", EVENT_SOURCE_EMAIL, false),
    ];
    assert_eq!(
        outcome_of(S::Rejected, &history, NOW - DAY),
        Some(Outcome::Pending)
    );
    assert_eq!(
        outcome_of(S::Rejected, &history, NOW - 30 * DAY),
        Some(Outcome::NoReply)
    );
}

#[test]
fn an_unconfirmed_email_ghosted_is_not_no_reply_yet() {
    let history = [
        user_event("applied"),
        event("ghosted", EVENT_SOURCE_EMAIL, false),
    ];
    assert_eq!(
        outcome_of(S::Ghosted, &history, NOW - DAY),
        Some(Outcome::Pending)
    );
}

#[test]
fn an_unconfirmed_suggestion_falls_back_to_the_last_settled_status_and_chains_are_skipped() {
    let history = [
        user_event("applied"),
        user_event("screening"),
        event("interviewing", EVENT_SOURCE_EMAIL, false),
        event("rejected", EVENT_SOURCE_EMAIL, false),
    ];
    assert_eq!(effective_status(S::Rejected, &history), S::Screening);
    assert_eq!(
        outcome_of(S::Rejected, &history, NOW - DAY),
        Some(Outcome::PositiveReply)
    );
}

#[test]
fn a_reviewed_email_status_is_taken_at_face_value() {
    let history = [
        user_event("applied"),
        event("rejected", EVENT_SOURCE_EMAIL, true),
    ];
    assert_eq!(effective_status(S::Rejected, &history), S::Rejected);
    assert_eq!(
        outcome_of(S::Rejected, &history, NOW - DAY),
        Some(Outcome::Rejected)
    );
}

#[test]
fn a_confirmed_email_suggestion_counts() {
    let history = [
        user_event("applied"),
        event("screening", EVENT_SOURCE_EMAIL, true),
    ];
    assert!(ever_reached_positive(S::Applied, &history));
}

#[test]
fn a_rejected_email_suggestion_does_not_count_even_though_it_was_reviewed() {
    // `reject_status_event` marks the original reviewed (confirmed) AND appends a reversal row.
    let mut reversal = event("applied", EVENT_SOURCE_EMAIL_REJECT, true);
    reversal.from_status = "screening".into();
    let history = [
        user_event("applied"),
        event("screening", EVENT_SOURCE_EMAIL, true),
        reversal,
    ];
    assert!(!ever_reached_positive(S::Applied, &history));
}

#[test]
fn a_reversal_row_never_counts_as_reaching_a_stage() {
    // The user rejected an email "rejected": the reversal restores `screening`, which the
    // earlier confirmed user event already established; alone it must not count.
    let mut reversal = event("screening", EVENT_SOURCE_EMAIL_REJECT, true);
    reversal.from_status = "rejected".into();
    assert!(!ever_reached_positive(S::Applied, &[reversal]));
}

#[test]
fn a_dismissed_email_suggestion_never_counts_even_when_the_status_moved_on_by_hand() {
    // Unconfirmed email Screening, then the user marked it Rejected by hand, then dismissed
    // the suggestion: the dismissal cannot revert, and it is not an acceptance.
    let history = [
        user_event("applied"),
        dismissed_event("screening"),
        user_event("rejected"),
    ];
    assert!(!ever_reached_positive(S::Rejected, &history));
    assert_eq!(
        outcome_of(S::Rejected, &history, NOW - DAY),
        Some(Outcome::Rejected)
    );
}

/// Two email writes (`first`, then a rejection) and the user dismissing the rejection:
/// the reversal restores `first`'s status.
fn dismissed_rejection_after(first: StatusEvent) -> [StatusEvent; 4] {
    let mut reversal = event("interviewing", EVENT_SOURCE_EMAIL_REJECT, true);
    reversal.from_status = "rejected".into();
    [
        user_event("applied"),
        first,
        dismissed_event("rejected"),
        reversal,
    ]
}

#[test]
fn a_reversal_back_to_an_unconfirmed_email_status_is_not_trusted() {
    let history = dismissed_rejection_after(event("interviewing", EVENT_SOURCE_EMAIL, false));
    assert_eq!(effective_status(S::Interviewing, &history), S::Applied);
    assert_eq!(
        outcome_of(S::Interviewing, &history, NOW - DAY),
        Some(Outcome::Pending),
        "never a positive reply"
    );
}

#[test]
fn a_reversal_back_to_a_user_confirmed_status_is_trusted() {
    let history = dismissed_rejection_after(user_event("interviewing"));
    assert_eq!(effective_status(S::Interviewing, &history), S::Interviewing);
    assert_eq!(
        outcome_of(S::Interviewing, &history, NOW - DAY),
        Some(Outcome::PositiveReply)
    );
}
