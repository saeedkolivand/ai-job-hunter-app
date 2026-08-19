//! The pure status-ladder rule: what one classified
//! [`crate::email_watch::intent::EmailIntent`] should do to an
//! application's current status. Split out of `intent.rs` (R8 LOC cap) —
//! same store/family, no behavior change from the split itself. Consumes an
//! already-classified intent; decides nothing about which application or
//! whether to write, see [`crate::email_watch::matcher`] and
//! [`crate::email_watch::auto_write`] for those.

use crate::applications::ApplicationStatus;
use crate::email_watch::intent::EmailIntent;

// ── The pure status-ladder rule (consumes an intent, decides on nothing yet) ──

/// Live (non-terminal) statuses this rule is allowed to move. The other 4
/// (`Accepted`/`Rejected`/`Ghosted`/`Withdrawn`) are final outcomes — see
/// [`next_status`]'s doc for why nothing moves them further.
fn is_live(status: ApplicationStatus) -> bool {
    matches!(
        status,
        ApplicationStatus::Saved
            | ApplicationStatus::Applied
            | ApplicationStatus::Screening
            | ApplicationStatus::Interviewing
            | ApplicationStatus::Offer
    )
}

/// Forward-ladder position for the 5 live statuses (higher = further along).
/// [`next_status`] only ever calls this on a status already known `is_live`,
/// but the match stays total (never panics) — a terminal status is parked at
/// `u8::MAX` so a future caller mistake could only ever look like "no
/// advance available" (fails closed), never manufacture a spurious advance.
fn ladder_rank(status: ApplicationStatus) -> u8 {
    match status {
        ApplicationStatus::Saved => 0,
        ApplicationStatus::Applied => 1,
        ApplicationStatus::Screening => 2,
        ApplicationStatus::Interviewing => 3,
        ApplicationStatus::Offer => 4,
        ApplicationStatus::Accepted
        | ApplicationStatus::Rejected
        | ApplicationStatus::Ghosted
        | ApplicationStatus::Withdrawn => u8::MAX,
    }
}

/// `target` only if it is strictly further along the ladder than `current`
/// — never a regression, and never a same-stage no-op write.
fn advance_to(target: ApplicationStatus, current: ApplicationStatus) -> Option<ApplicationStatus> {
    (ladder_rank(target) > ladder_rank(current)).then_some(target)
}

/// The status one classified [`EmailIntent`] maps to, independent of
/// `current` — the raw target `next_status` then either advances to,
/// applies unconditionally (`Rejection`), or overrides a stale terminal
/// with.
fn intent_target(intent: EmailIntent) -> ApplicationStatus {
    match intent {
        EmailIntent::Rejection => ApplicationStatus::Rejected,
        EmailIntent::Confirmation => ApplicationStatus::Applied,
        EmailIntent::Interview => ApplicationStatus::Interviewing,
        EmailIntent::Offer => ApplicationStatus::Offer,
    }
}

/// Whether an application currently at `status` is one this whole
/// email-tracking system is allowed to act on at all — live, OR terminal but
/// itself an unconfirmed email-derived write (`unconfirmed_email_write`; see
/// [`crate::applications::ApplicationStore::
/// current_status_is_unconfirmed_email_write`]).
///
/// **The single shared eligibility predicate** — [`next_status`] below and
/// [`crate::email_watch::matcher::best_match`]'s candidate filter BOTH call
/// THIS function rather than each re-deriving the same condition, precisely
/// so the two can never independently drift out of agreement. They decide
/// different things (this fn: "may `next_status` write anything at all for
/// this status". `best_match`: "is this application even a match
/// candidate") but MUST agree on which statuses qualify — if the matcher
/// excluded a status `next_status` would otherwise be willing to move, the
/// terminal-override fix above would silently become dead code one layer up
/// (the exact defect class a repo-wide lesson already flagged: two callers
/// of one predicate can have opposite needs, but here they must not — see
/// `matcher::tests::matcher_and_next_status_eligibility_never_disagree`,
/// the property test that pins this).
pub(super) fn is_actionable(status: ApplicationStatus, unconfirmed_email_write: bool) -> bool {
    is_live(status) || unconfirmed_email_write
}

/// What one classified [`EmailIntent`] should do to `current`'s status.
/// `None` means no-op — don't write anything — never a downgrade.
///
/// `current_is_unconfirmed_email_write` is the provenance the security
/// review's "one cold email freezes an application forever" finding forced:
/// whether `current` was ITSELF set by a still-unconfirmed, email-derived
/// write (see [`crate::applications::StatusEvent::source`]/[`crate::
/// applications::StatusEvent::confirmed`] — the caller determines this via
/// [`crate::applications::ApplicationStore::current_status_is_unconfirmed_email_write`],
/// this fn stays pure/store-free). It changes exactly one thing:
///
/// - **A LIVE `current` behaves exactly as before**, regardless of this
///   flag — never regress the ladder; `Rejection` wins unconditionally from
///   any live status (`Saved` through `Offer`, including `Offer` itself —
///   an offer can be rescinded before the candidate accepts it); everything
///   else only ever advances (`advance_to`), never repeats a no-op write.
/// - **A TERMINAL `current`** (`Accepted`/`Rejected`/`Ghosted`/`Withdrawn`)
///   **absorbs by default** — a later email (a resend, a stale/reordered
///   notification, or a genuinely new event this 4-way classifier has no
///   intent for, like a rescinded offer) is out of scope: silently
///   overwriting a final — usually user-set or user-accepted — outcome
///   would be worse than requiring a human to handle that rare edge case.
///   **UNLESS `current_is_unconfirmed_email_write` is `true`**: the
///   terminal status is itself unreviewed speculation from a classifier
///   with a recorded precision limit (a cold/attacker-supplied email can
///   set it — see `crate::email_watch::auto_write`'s own sender-provenance
///   gate, the OTHER half of this fix), so a LATER email's intent may
///   supersede it outright — any DIFFERENT target, not bound by the ladder
///   ordering at all (the value being "regressed from" was never
///   trustworthy to begin with). The degenerate case (the new target
///   equals the current, already-unconfirmed terminal) is still a no-op —
///   nothing actually changed.
///
/// This fn's own gate is just [`is_actionable`] — see that fn's doc for why
/// it, and not a second copy of the same condition, is what decides "may
/// this status move at all".
pub fn next_status(
    intent: EmailIntent,
    current: ApplicationStatus,
    current_is_unconfirmed_email_write: bool,
) -> Option<ApplicationStatus> {
    if !is_actionable(current, current_is_unconfirmed_email_write) {
        return None;
    }
    let target = intent_target(intent);
    if is_live(current) {
        return if intent == EmailIntent::Rejection {
            Some(target)
        } else {
            advance_to(target, current)
        };
    }
    // `is_actionable` returned true and `current` is not live, so this
    // status is terminal AND `current_is_unconfirmed_email_write` — the
    // override branch.
    (target != current).then_some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── next_status: the pure ladder rule ───────────────────────────────────

    #[test]
    fn confirmation_from_saved_advances_to_applied() {
        assert_eq!(
            next_status(EmailIntent::Confirmation, ApplicationStatus::Saved, false),
            Some(ApplicationStatus::Applied)
        );
    }

    #[test]
    fn confirmation_from_offer_is_a_noop_not_a_downgrade() {
        assert_eq!(
            next_status(EmailIntent::Confirmation, ApplicationStatus::Offer, false),
            None
        );
    }

    #[test]
    fn confirmation_from_applied_is_a_noop_same_stage() {
        assert_eq!(
            next_status(EmailIntent::Confirmation, ApplicationStatus::Applied, false),
            None
        );
    }

    #[test]
    fn interview_from_saved_skips_forward_to_interviewing() {
        assert_eq!(
            next_status(EmailIntent::Interview, ApplicationStatus::Saved, false),
            Some(ApplicationStatus::Interviewing)
        );
    }

    #[test]
    fn interview_from_interviewing_is_a_noop() {
        assert_eq!(
            next_status(
                EmailIntent::Interview,
                ApplicationStatus::Interviewing,
                false
            ),
            None
        );
    }

    #[test]
    fn interview_from_offer_is_a_noop_not_a_downgrade() {
        assert_eq!(
            next_status(EmailIntent::Interview, ApplicationStatus::Offer, false),
            None
        );
    }

    #[test]
    fn offer_intent_from_saved_skips_forward_to_offer() {
        assert_eq!(
            next_status(EmailIntent::Offer, ApplicationStatus::Saved, false),
            Some(ApplicationStatus::Offer)
        );
    }

    #[test]
    fn offer_intent_from_offer_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Offer, ApplicationStatus::Offer, false),
            None
        );
    }

    #[test]
    fn rejection_from_saved_advances_to_rejected() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Saved, false),
            Some(ApplicationStatus::Rejected)
        );
    }

    #[test]
    fn rejection_from_applied_advances_to_rejected() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Applied, false),
            Some(ApplicationStatus::Rejected)
        );
    }

    #[test]
    fn rejection_from_screening_advances_to_rejected() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Screening, false),
            Some(ApplicationStatus::Rejected)
        );
    }

    #[test]
    fn rejection_from_interviewing_advances_to_rejected() {
        assert_eq!(
            next_status(
                EmailIntent::Rejection,
                ApplicationStatus::Interviewing,
                false
            ),
            Some(ApplicationStatus::Rejected)
        );
    }

    #[test]
    fn rejection_from_offer_advances_to_rejected() {
        // An offer can be rescinded by the employer before the candidate
        // accepts it — rejection is allowed from `Offer` too.
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Offer, false),
            Some(ApplicationStatus::Rejected)
        );
    }

    // ── terminal statuses: nothing moves them, not even Rejection ──────────

    #[test]
    fn rejection_from_accepted_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Accepted, false),
            None
        );
    }

    #[test]
    fn rejection_from_already_rejected_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Rejected, false),
            None
        );
    }

    #[test]
    fn rejection_from_ghosted_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Ghosted, false),
            None
        );
    }

    #[test]
    fn rejection_from_withdrawn_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Rejection, ApplicationStatus::Withdrawn, false),
            None
        );
    }

    #[test]
    fn confirmation_from_accepted_is_a_noop() {
        assert_eq!(
            next_status(
                EmailIntent::Confirmation,
                ApplicationStatus::Accepted,
                false
            ),
            None
        );
    }

    #[test]
    fn interview_from_already_rejected_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Interview, ApplicationStatus::Rejected, false),
            None
        );
    }

    #[test]
    fn offer_intent_from_withdrawn_is_a_noop() {
        assert_eq!(
            next_status(EmailIntent::Offer, ApplicationStatus::Withdrawn, false),
            None
        );
    }

    #[test]
    fn next_status_gate_agrees_with_is_actionable_for_every_status() {
        // Mirror of `matcher::tests::matcher_and_next_status_eligibility_never_disagree`
        // from the OTHER side: `next_status` already gates on `is_actionable`
        // as its first line, so this is guaranteed by construction — but
        // proves it empirically so a future edit that reintroduces a
        // hand-rolled condition here (instead of calling `is_actionable`)
        // gets caught. `Rejection` is used as the probe intent because it is
        // the ONE intent that unconditionally produces `Some` from every
        // live status (never a same-stage no-op), so `is_some()` cleanly
        // signals "was actionable" for every case except the single
        // degenerate one called out below.
        for &status in ApplicationStatus::ALL {
            for unconfirmed in [false, true] {
                let expected_actionable = is_actionable(status, unconfirmed);
                let result = next_status(EmailIntent::Rejection, status, unconfirmed);
                if !expected_actionable {
                    assert_eq!(
                        result, None,
                        "{status:?} (unconfirmed={unconfirmed}) must be a no-op when \
                         is_actionable is false"
                    );
                } else if status != ApplicationStatus::Rejected {
                    assert_eq!(
                        result,
                        Some(ApplicationStatus::Rejected),
                        "{status:?} (unconfirmed={unconfirmed}) must be actionable when \
                         is_actionable is true"
                    );
                }
                // status == Rejected AND actionable only happens when
                // unconfirmed is true (Rejected is never live) — that's the
                // degenerate "target == current" no-op the terminal-override
                // branch itself returns None for, not a disagreement about
                // actionability; deliberately excluded above.
            }
        }
    }
}
