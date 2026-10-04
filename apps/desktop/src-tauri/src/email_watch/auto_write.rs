//! v2 slice 2/3: turn one classified [`EmailIntent`] into a real, but always
//! **UNCONFIRMED**, [`Application`](crate::applications::Application) status
//! write. Nothing here writes `confirmed: true` — ever (see
//! [`apply_matched_intent`]'s doc, and [`crate::email_watch::intent`]'s
//! module doc on the classifier's recorded precision limit, which is why
//! adjudication — not withholding the write — is the safety model).
//!
//! Wired into the runtime path from [`super::super::email_watch_scheduler`]
//! (L2, the one place in this module family with `AppHandle` reach) — NOT
//! from [`super::poller`]'s tick itself, which stays pure matching (see its
//! own module doc). See `email_watch_scheduler::run_check_inner`'s own doc
//! for exactly where in the tick this is called.
use crate::applications::ApplicationStore;
use crate::email_watch::intent::{next_status, EmailIntent};
use crate::email_watch::EmailWatchStore;
use crate::error::AppResult;

/// Apply one classified [`EmailIntent`] to `application_id`'s CURRENT
/// status. Never reimplements the ladder — [`next_status`] decides the
/// target, and [`ApplicationStore::transition_status_if_sourced`] (the SAME
/// atomic compare-and-set every other caller in this crate uses) performs
/// the write.
///
/// **`current_status` is read HERE, by this function, immediately before
/// deciding the target — never accepted as a caller-supplied argument.**
/// This used to take `current_status: ApplicationStatus` from the caller,
/// which in `email_watch_scheduler`'s tick loop was a SINGLE pre-tick
/// snapshot (`matcher::best_match`'s match, taken once before the loop)
/// reused for every outcome in that tick. Two matched messages for the
/// SAME application in one tick — an ordinary ATS thread, e.g. a
/// confirmation then a later rejection inside one 15-minute window — both
/// received the identical stale status: the first outcome's CAS succeeded
/// and moved the row; the second's CAS then raced the STALE value against
/// the row the first outcome had ALREADY changed, lost, and returned
/// `Ok(false)` — indistinguishable from "a real external writer beat us to
/// it", so the second message's write was silently, permanently dropped
/// (its uid was already stamped by `mark_seen` before this ever ran, so a
/// later tick never reconsiders it). Reading live status inside this
/// function, immediately before each call's own CAS, means the SECOND
/// outcome in that same sequential loop sees the FIRST outcome's write
/// and rolls forward from it correctly. This does not weaken the CAS
/// itself — `transition_status_if_sourced` still atomically re-validates
/// at write time and still fails closed against a genuinely concurrent
/// external writer (the read-then-CAS gap inside this function is exactly
/// as wide as it always was for that case); it only removes the SELF-
/// INFLICTED staleness of a caller passing a snapshot from before other
/// outcomes in the same loop had already run.
///
/// **`write_authorized` gates the write on AUTHENTICATED sender provenance —
/// a cold or spoofed email must never write.**
/// [`crate::email_watch::parser::fingerprint`]'s subject match is a REGEX
/// over attacker-controlled text; anyone who knows a user applied to a
/// company can send a subject/body that fingerprints AND classifies.
/// Without a provenance gate, that one email would freeze the application
/// (see [`next_status`]'s terminal-absorption doc) as far as automation is
/// concerned, silently — not data loss, a silent STOP to tracking.
///
/// A bare sender-domain string match is NOT sufficient authentication.
/// `write_authorized` (the caller's — the ONLY caller is
/// `email_watch_scheduler`, NOT `MessageOutcome::write_authorized` alone;
/// that field is [`crate::email_watch::parser::Fingerprint::
/// write_gate_domain`] AND [`crate::email_watch::parser::EmailHeader::
/// dmarc_pass`], and the scheduler ADDITIONALLY ANDs in
/// [`crate::email_watch::parser::host_is_known_to_stamp`] before it ever
/// reaches this function) is BEST-EFFORT, not a closed gate — three prior
/// review rounds each found a NEW way a confident sentence here was wrong,
/// so state the mechanism plainly rather than asserting a conclusion:
///
/// - `linkedin.com`/`indeed.com` (messaging relays that routinely carry
///   attacker-authored content from their own genuinely-authentic
///   infrastructure) ARE closed — dropped from the write-gate domain list
///   entirely, regardless of their own DMARC posture.
/// - A free ATS-tenant signup sending attacker-controlled content from a
///   still-listed vendor domain is NOT closed — accepted, documented
///   residual, not assumed away.
/// - `host_is_known_to_stamp` closes exactly ONE narrow case: a message
///   whose ONLY `Authentication-Results` header is forged, because the
///   host never stamps anything at all. It does NOT prove the host stamps
///   a `dmarc=` clause for every sender the message's `From:` domain might
///   name — DMARC evaluation can legitimately produce no `dmarc=` section
///   for a given domain. A determined sender picks a `From:` domain for
///   exactly that reason, then supplies the header's ONLY `dmarc=` text
///   themselves via the same envelope-echo mechanism
///   [`crate::email_watch::auth_results`]'s own doc describes. One clause,
///   one section, indistinguishable from real grammar by content alone —
///   because it genuinely IS real grammar by the time anything here reads
///   it. Two candidate fixes for this were measured and both failed; see
///   `parser::dmarc_pass_aligned`'s doc for the full accounting, including
///   why an authserv-id check does not help either (not a rustdoc link —
///   that fn is private to `parser`, unreachable from a sibling module).
///
/// What actually mitigates the open residual: (1)
/// [`EmailWatchStore::auto_write_enabled`] defaults OFF, so the gate is
/// opt-in and nobody is exposed without deliberately turning it on; (2)
/// every write this function can ever produce lands UNCONFIRMED (see this
/// fn's own "Hard constraint" note below) and needs the user's own
/// adjudication before it is trusted — a backstop that does not depend on
/// this gate, or anything upstream of it, being correct. Closing it
/// properly needs verification that does not trust the header at all
/// (independent DKIM/SPF/DMARC re-verification against DNS); that is a
/// larger, deliberately NOT-built feature (new dependency, new network
/// egress this feature's design avoids), not a fifth parser round.
///
/// Two WIDER signals were considered and deliberately NOT implemented: the
/// sender domain matching the application's own company domain (no existing
/// field derives a company's expected email domain — a real new piece of
/// logic, not a seam) and in-thread linkage via `References`/`In-Reply-To`
/// (would need a further IMAP `HEADER.FIELDS` widening — a cost to approve,
/// not to spend silently).
///
/// **Gated in this exact order** (each a legitimate no-op — `Ok(false)`,
/// never an error):
/// 1. [`EmailWatchStore::auto_write_enabled`] is off;
/// 2. `write_authorized` is `false` — an unrecognised sender, OR a
///    recognised one without an aligned DMARC `pass`;
/// 3. `intent` is `None` — [`crate::email_watch::intent::classify_intent`]
///    decided nothing (a real, testable no-op here, not merely "the caller
///    happened not to call this" — the caller passes `MessageOutcome::
///    intent` straight through);
/// 4. the application no longer exists (read fails) — vanishingly narrow,
///    but a target that vanished mid-tick (e.g. `remove`d concurrently)
///    must not panic or write against nothing;
/// 5. [`next_status`] itself says no-op — a terminal, still-CONFIRMED-or-
///    user-set status (absorbing by design), or the intent doesn't
///    advance the ladder;
/// 6. the user already rejected a write LANDING AT `target` for this
///    application — keyed on `target` alone, not the `(current_status,
///    target)` pair, so a detour through a different live status can't
///    re-apply a target the user has already told us was wrong (see
///    [`ApplicationStore::was_transition_rejected`]'s doc);
/// 7. the compare-and-set itself loses — status changed in the narrow
///    window between this function's own read (gate 4) and the write,
///    which by now can only be a genuinely concurrent EXTERNAL writer
///    (the user's own hand, or another IPC call), never a stale snapshot
///    from earlier in the same tick — that class is what gate 4 removed.
///
/// **Hard constraint: always writes `confirmed = false`.** Nothing in this
/// function, or reachable from it, may ever pass `true` for the write this
/// function performs — the unconfirmed row IS the whole safety model.
pub fn apply_matched_intent(
    applications: &ApplicationStore,
    email_watch: &EmailWatchStore,
    application_id: &str,
    intent: Option<EmailIntent>,
    write_authorized: bool,
) -> AppResult<bool> {
    if !email_watch.auto_write_enabled() {
        return Ok(false);
    }
    if !write_authorized {
        return Ok(false);
    }
    let Some(intent) = intent else {
        return Ok(false);
    };
    // MAJOR fix: read LIVE, right here, right before deciding the target —
    // never trust a caller's snapshot. See this fn's own doc for the
    // two-matched-messages-in-one-tick bug this closes.
    let Some(current) = applications.get(application_id) else {
        return Ok(false); // vanished mid-tick — safe no-op, not an error
    };
    let current_status = current.status;
    let current_is_unconfirmed_email_write =
        applications.current_status_is_unconfirmed_email_write(application_id);
    let Some(target) = next_status(intent, current_status, current_is_unconfirmed_email_write)
    else {
        return Ok(false);
    };
    if applications.was_transition_rejected(application_id, target) {
        return Ok(false);
    }
    applications.transition_status_if_sourced(
        application_id,
        current_status,
        target,
        Some("email-derived (unconfirmed)"),
        crate::applications::EVENT_SOURCE_EMAIL,
        false,
    )
}

#[cfg(test)]
mod tests;
