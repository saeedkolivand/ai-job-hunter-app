use super::*;

// ── next_status: the pure ladder rule ───────────────────────────────────

/// The pure ladder rule, case by case: a live status only ever moves forward (a same-stage or
/// backward intent is a no-op), `Rejection` applies from every live status, and a hard-terminal
/// status absorbs every intent.
#[test]
fn next_status_ladder() {
    use ApplicationStatus as S;
    use EmailIntent as I;
    let cases: &[(EmailIntent, ApplicationStatus, Option<ApplicationStatus>)] = &[
        (I::Confirmation, S::Saved, Some(S::Applied)),
        (I::Confirmation, S::Offer, None),
        (I::Confirmation, S::Applied, None),
        (I::Interview, S::Saved, Some(S::Interviewing)),
        (I::Interview, S::Interviewing, None),
        (I::Interview, S::Offer, None),
        (I::Offer, S::Saved, Some(S::Offer)),
        (I::Offer, S::Offer, None),
        (I::Rejection, S::Saved, Some(S::Rejected)),
        (I::Rejection, S::Applied, Some(S::Rejected)),
        (I::Rejection, S::Screening, Some(S::Rejected)),
        (I::Rejection, S::Interviewing, Some(S::Rejected)),
        // An offer can be rescinded by the employer before the candidate
        // accepts it — rejection is allowed from `Offer` too.
        (I::Rejection, S::Offer, Some(S::Rejected)),
        // Terminal statuses: nothing moves them, not even Rejection.
        (I::Rejection, S::Accepted, None),
        (I::Rejection, S::Rejected, None),
        // MAJOR fix: `Ghosted` is live now (see `is_live`'s own doc), not
        // grouped with the hard-terminal statuses — INVERTED from an earlier
        // version of this test (`rejection_from_ghosted_is_a_noop`, asserting
        // `None`) that pinned exactly the predicate-disagreement bug this
        // fixes. `Rejection` applies unconditionally to any live status, so a
        // ghosted application confirmed dead by a later email must land
        // `Rejected`, not stay stuck.
        (I::Rejection, S::Ghosted, Some(S::Rejected)),
        // The employer resurfacing after ghosting — the domain type's own
        // stated reason `Ghosted` is excluded from `is_terminal` — must reach
        // the ladder: a confirmation/interview/offer intent advances a
        // ghosted application exactly like it would from `Saved` (both rank
        // 0; see `ladder_rank`'s doc for why).
        (I::Confirmation, S::Ghosted, Some(S::Applied)),
        (I::Interview, S::Ghosted, Some(S::Interviewing)),
        (I::Offer, S::Ghosted, Some(S::Offer)),
        (I::Rejection, S::Withdrawn, None),
        (I::Confirmation, S::Accepted, None),
        (I::Interview, S::Rejected, None),
        (I::Offer, S::Withdrawn, None),
    ];
    for &(intent, current, expected) in cases {
        assert_eq!(
            next_status(intent, current, false),
            expected,
            "{intent:?} from {current:?}"
        );
    }
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
