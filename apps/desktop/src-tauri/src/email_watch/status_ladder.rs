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

/// Live (non-terminal) statuses this rule is allowed to move — delegates
/// DIRECTLY to [`ApplicationStatus::is_terminal`] rather than re-deriving
/// the same classification independently. That independence was ALREADY a
/// shipped bug: this fn used to group `Ghosted` with the three hard-
/// terminal statuses, while the domain type deliberately excludes it
/// ("soft-terminal, reopenable — a ghosted pursuit can still revive"). The
/// two disagreeing meant `next_status` returned `None` for every intent on
/// a ghosted application, and [`crate::email_watch::matcher::best_match`]
/// (which shares this classification via [`is_actionable`] below) never
/// even considered a ghosted application a match candidate — an employer
/// resurfacing after ghosting, exactly the case the domain type exists
/// for, was dropped before the ladder ever saw it. There is now exactly
/// ONE place that decides "is this status closed" — the domain type — and
/// this fn just inverts it for the ladder's own vocabulary, so the two
/// cannot independently drift again.
fn is_live(status: ApplicationStatus) -> bool {
    !status.is_terminal()
}

/// Forward-ladder position for the 6 live statuses (higher = further
/// along; `Ghosted` counts as live now — see [`is_live`]'s doc). Total,
/// never panics: [`next_status`] only ever calls this on a status already
/// known `is_live`, but a HARD-terminal status (`Accepted`/`Rejected`/
/// `Withdrawn`) is still parked at `u8::MAX` so a future caller mistake
/// could only ever look like "no advance available" (fails closed), never
/// manufacture a spurious advance.
fn ladder_rank(status: ApplicationStatus) -> u8 {
    match status {
        // `Ghosted` carries no memory of which live stage it ghosted FROM
        // (the domain type collapses that), so it ranks at the BOTTOM —
        // same as `Saved` — meaning ANY forward-advancing intent
        // (Confirmation/Interview/Offer) counts as reviving it, matching
        // "a ghosted pursuit can still revive" without guessing where it
        // left off. `Rejection` applies unconditionally to any live status
        // regardless of rank (see `next_status`), so this rank never gates
        // that path either way.
        ApplicationStatus::Saved | ApplicationStatus::Ghosted => 0,
        ApplicationStatus::Applied => 1,
        ApplicationStatus::Screening => 2,
        ApplicationStatus::Interviewing => 3,
        ApplicationStatus::Offer => 4,
        ApplicationStatus::Accepted
        | ApplicationStatus::Rejected
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
///   an offer can be rescinded before the candidate accepts it — AND
///   `Ghosted`, which counts as live: see [`is_live`]'s doc for why);
///   everything else only ever advances (`advance_to`), never repeats a
///   no-op write.
/// - **A HARD-TERMINAL `current`** (`Accepted`/`Rejected`/`Withdrawn` —
///   NOT `Ghosted`, which is live per [`is_live`]) **absorbs by
///   default** — a later email (a resend, a stale/reordered
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
mod tests;
