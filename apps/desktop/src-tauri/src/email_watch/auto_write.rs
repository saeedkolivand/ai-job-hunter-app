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

use crate::applications::{ApplicationStatus, ApplicationStore};
use crate::email_watch::intent::{next_status, EmailIntent};
use crate::email_watch::EmailWatchStore;
use crate::error::AppResult;

/// Apply one classified [`EmailIntent`] to `application_id`'s CURRENT
/// status. Never reimplements the ladder — [`next_status`] decides the
/// target, and [`ApplicationStore::transition_status_if_sourced`] (the SAME
/// atomic compare-and-set every other caller in this crate uses) performs
/// the write.
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
///   [`crate::email_watch::parser::dmarc_pass_aligned`]'s doc for the full
///   accounting, including why an authserv-id check does not help either.
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
/// 4. [`next_status`] itself says no-op — a terminal, still-CONFIRMED-or-
///    user-set `current_status` (absorbing by design), or the intent
///    doesn't advance the ladder;
/// 5. the user already rejected a write LANDING AT `target` for this
///    application — keyed on `target` alone, not the `(current_status,
///    target)` pair, so a detour through a different live status can't
///    re-apply a target the user has already told us was wrong (see
///    [`ApplicationStore::was_transition_rejected`]'s doc);
/// 6. the compare-and-set itself loses (the application's status changed
///    since the caller last read it — the same race every other
///    `transition_status_if`-family caller already tolerates).
///
/// **Hard constraint: always writes `confirmed = false`.** Nothing in this
/// function, or reachable from it, may ever pass `true` for the write this
/// function performs — the unconfirmed row IS the whole safety model.
pub fn apply_matched_intent(
    applications: &ApplicationStore,
    email_watch: &EmailWatchStore,
    application_id: &str,
    current_status: ApplicationStatus,
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
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::applications::{ApplicationMeta, ApplicationOrigin};

    /// Fresh `ApplicationStore` + `EmailWatchStore`, each in its own temp
    /// dir (they are separate `.db` files in the real app too — no shared
    /// state to seed beyond each store's own migrations). `auto_write_enabled`
    /// now DEFAULTS OFF (see the `add_auto_write_enabled` migration's own
    /// doc), so this helper connects and explicitly opts IN — every test in
    /// this file below is testing WRITE GATES other than the opt-in toggle
    /// itself, and needs auto-write actually reachable to exercise them; the
    /// one test that IS about the toggle
    /// (`the_auto_write_toggle_off_blocks_the_write_entirely`) explicitly
    /// opts back OUT.
    fn stores() -> (TempDir, ApplicationStore, TempDir, EmailWatchStore) {
        let apps_dir = TempDir::new().unwrap();
        let applications = ApplicationStore::open(apps_dir.path()).unwrap();
        let email_dir = TempDir::new().unwrap();
        let email_watch = EmailWatchStore::open(&email_dir.path().to_path_buf()).unwrap();
        email_watch
            .connect("jane@example.com", "imap.example.com", 993)
            .unwrap();
        assert!(email_watch.set_auto_write_enabled(true).unwrap());
        (apps_dir, applications, email_dir, email_watch)
    }

    fn meta() -> ApplicationMeta {
        ApplicationMeta {
            company: "Acme".into(),
            title: "Engineer".into(),
            candidate: "Jane".into(),
            brief: String::new(),
            job_description: String::new(),
            answers: vec![],
            job_summary: String::new(),
            salary_min: None,
            salary_max: None,
            salary_currency: None,
        }
    }

    /// A fresh `saved` application id.
    fn saved_app(applications: &ApplicationStore) -> String {
        applications
            .upsert_for_origin(
                "https://x.example/1",
                "b",
                &meta(),
                ApplicationOrigin::Saved,
                None,
            )
            .unwrap()
    }

    #[test]
    fn a_decided_intent_writes_an_unconfirmed_status_change() {
        let (_d1, applications, _d2, email_watch) = stores();
        let id = saved_app(&applications);

        let wrote = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Saved,
            Some(EmailIntent::Confirmation),
            true,
        )
        .unwrap();
        assert!(wrote);

        let app = applications.get(&id).unwrap();
        assert_eq!(app.status, ApplicationStatus::Applied);

        let last = applications.events(&id).into_iter().last().unwrap();
        assert_eq!(last.source, crate::applications::EVENT_SOURCE_EMAIL);
        assert!(
            !last.confirmed,
            "an email-derived write must NEVER land confirmed"
        );
    }

    #[test]
    fn a_cold_unrecognized_sender_never_writes_even_with_a_decided_intent() {
        // The security-review finding: attribution is entirely
        // attacker-supplied (fingerprint is subject-regex-only), so a
        // single cold email must never move a status regardless of how
        // confidently the intent classified.
        let (_d1, applications, _d2, email_watch) = stores();
        let id = saved_app(&applications);

        let wrote = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Saved,
            Some(EmailIntent::Confirmation),
            false,
        )
        .unwrap();
        assert!(
            !wrote,
            "a cold/unauthorized sender (write_authorized=false) must never write"
        );
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Saved
        );
    }

    #[test]
    fn a_none_intent_never_writes_even_with_a_recognized_sender() {
        // German confirmations (zero discriminating phrases) and any other
        // genuinely-undecided message classify as `None` — this must be a
        // real, directly-testable no-op, not just "the caller happened not
        // to call this function".
        let (_d1, applications, _d2, email_watch) = stores();
        let id = saved_app(&applications);

        let wrote = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Saved,
            None,
            true,
        )
        .unwrap();
        assert!(!wrote, "a None intent must never write");
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Saved
        );
    }

    #[test]
    fn the_auto_write_toggle_off_blocks_the_write_entirely() {
        let (_d1, applications, _d2, email_watch) = stores();
        let id = saved_app(&applications);
        // `stores()` already connected (and opted IN) — `set_auto_write_enabled`
        // guards on `address IS NOT NULL` (same concurrent-clear discipline
        // as `set_enabled`), which that connect already satisfies.
        let toggled = email_watch.set_auto_write_enabled(false).unwrap();
        assert!(toggled, "the toggle write must succeed once connected");

        let wrote = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Saved,
            Some(EmailIntent::Confirmation),
            true,
        )
        .unwrap();
        assert!(!wrote, "the toggle off must block the write");
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Saved
        );
    }

    #[test]
    fn a_non_advancing_intent_is_a_silent_no_op() {
        // Confirmation intent while already at Offer: next_status says no-op
        // (never a downgrade) — this must propagate as a clean `false`, not
        // an error, and must not touch the status or append anything.
        let (_d1, applications, _d2, email_watch) = stores();
        let id = saved_app(&applications);
        applications
            .set_status(&id, ApplicationStatus::Offer, "")
            .unwrap();
        let events_before = applications.events(&id).len();

        let wrote = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Offer,
            Some(EmailIntent::Confirmation),
            true,
        )
        .unwrap();
        assert!(!wrote);
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Offer
        );
        assert_eq!(applications.events(&id).len(), events_before);
    }

    #[test]
    fn a_second_later_email_does_not_reapply_a_status_the_user_already_rejected() {
        let (_d1, applications, _d2, email_watch) = stores();
        let id = saved_app(&applications);
        applications
            .set_status(&id, ApplicationStatus::Interviewing, "")
            .unwrap();

        // Email 1: a rejection intent auto-writes Interviewing -> Rejected,
        // unconfirmed.
        let wrote_first = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Interviewing,
            Some(EmailIntent::Rejection),
            true,
        )
        .unwrap();
        assert!(wrote_first);
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Rejected
        );

        // The user rejects it — status reverts to Interviewing.
        let pending_event_id = applications
            .events(&id)
            .into_iter()
            .last()
            .unwrap()
            .event_id;
        let reverted = applications
            .reject_status_event(&id, pending_event_id)
            .unwrap();
        assert!(reverted);
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Interviewing
        );
        let events_after_reject = applications.events(&id).len();

        // Email 2 (later): the SAME intent, from the SAME now-current status
        // — must NOT re-apply Rejected.
        let wrote_second = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Interviewing,
            Some(EmailIntent::Rejection),
            true,
        )
        .unwrap();
        assert!(
            !wrote_second,
            "a later email must not re-apply a status the user already rejected"
        );
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Interviewing,
            "status must stay exactly where the user left it"
        );
        assert_eq!(
            applications.events(&id).len(),
            events_after_reject,
            "the blocked second write must append nothing"
        );
    }

    // -- terminal-override: an unconfirmed email-derived terminal is NOT
    // absorbing forever; a user-set or confirmed one still is -------------

    #[test]
    fn a_later_email_supersedes_its_own_unconfirmed_terminal_write() {
        // The exact "one cold email freezes the application" shape, minus
        // the cold part: a legitimate (write_authorized = true) rejection email
        // auto-writes Rejected (unconfirmed). Nobody has reviewed it yet.
        // A later, genuinely different email must be able to supersede the
        // still-unconfirmed Rejected, not be silently dropped.
        let (_d1, applications, _d2, email_watch) = stores();
        let id = saved_app(&applications);
        applications
            .set_status(&id, ApplicationStatus::Interviewing, "")
            .unwrap();

        let wrote_first = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Interviewing,
            Some(EmailIntent::Rejection),
            true,
        )
        .unwrap();
        assert!(wrote_first);
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Rejected
        );
        assert!(applications.current_status_is_unconfirmed_email_write(&id));

        let wrote_second = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Rejected,
            Some(EmailIntent::Interview),
            true,
        )
        .unwrap();
        assert!(
            wrote_second,
            "a later email must be able to supersede its OWN still-unconfirmed \
             terminal write -- an application must not freeze forever"
        );
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Interviewing
        );
        let last = applications.events(&id).into_iter().last().unwrap();
        assert_eq!(last.source, crate::applications::EVENT_SOURCE_EMAIL);
        assert!(!last.confirmed);
    }

    #[test]
    fn a_confirmed_terminal_status_still_absorbs_every_later_email() {
        // Once the user (or an Accept) confirms the terminal status, it is
        // no longer speculation -- it goes back to absorbing, exactly like
        // a user-set terminal always has.
        let (_d1, applications, _d2, email_watch) = stores();
        let id = saved_app(&applications);
        applications
            .set_status(&id, ApplicationStatus::Interviewing, "")
            .unwrap();

        apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Interviewing,
            Some(EmailIntent::Rejection),
            true,
        )
        .unwrap();
        let pending_event_id = applications
            .events(&id)
            .into_iter()
            .last()
            .unwrap()
            .event_id;
        assert!(applications
            .accept_status_event(&id, pending_event_id)
            .unwrap());
        assert!(!applications.current_status_is_unconfirmed_email_write(&id));

        let wrote = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Rejected,
            Some(EmailIntent::Interview),
            true,
        )
        .unwrap();
        assert!(
            !wrote,
            "a CONFIRMED terminal status must still absorb, same as a user-set one"
        );
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Rejected
        );
    }

    #[test]
    fn a_matched_unconfirmed_terminal_is_a_candidate_and_a_later_email_supersedes_it_end_to_end() {
        // The fix-forward task's required check: NOT just that `next_status`
        // allows the override (already pinned above) -- that the MATCHER
        // ALSO still treats an unconfirmed-email-derived terminal as a
        // candidate, so a later correcting email actually reaches
        // `next_status` at all. Chains matcher -> ladder -> write by hand
        // (the same three steps `email_watch_scheduler::run_check_inner`
        // performs across a real tick, which needs live IMAP I/O and can't
        // be unit-tested directly).
        let (_d1, applications, _d2, email_watch) = stores();
        let id = saved_app(&applications);
        applications
            .set_status(&id, ApplicationStatus::Interviewing, "")
            .unwrap();

        // Email 1: rejection auto-writes an unconfirmed terminal Rejected.
        let wrote_first = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Interviewing,
            Some(EmailIntent::Rejection),
            true,
        )
        .unwrap();
        assert!(wrote_first);
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Rejected
        );

        // MATCHER step: with the app now Rejected-but-unconfirmed, it must
        // still be found as a candidate -- exactly what the caller
        // (`email_watch_scheduler`) computes via
        // `current_status_is_unconfirmed_email_write` and threads through
        // `poller::run_tick` into `matcher::best_match`.
        let app_row = applications.get(&id).unwrap();
        let mut unconfirmed_ids = std::collections::HashSet::new();
        if applications.current_status_is_unconfirmed_email_write(&id) {
            unconfirmed_ids.insert(id.clone());
        }
        let candidates = crate::email_watch::parser::Candidates {
            company: Some(app_row.company.clone()),
            title: None,
        };
        let matched = crate::email_watch::matcher::best_match(
            &candidates,
            std::slice::from_ref(&app_row),
            false,
            &unconfirmed_ids,
        );
        assert_eq!(
            matched.map(|s| s.application_id),
            Some(id.clone()),
            "an unconfirmed email-derived terminal must still be a matcher candidate, or \
             the next_status terminal-override fix is dead code one layer up"
        );

        // LADDER + WRITE step: email 2, a genuinely different intent, must
        // supersede the still-unconfirmed terminal end to end.
        let wrote_second = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Rejected,
            Some(EmailIntent::Interview),
            true,
        )
        .unwrap();
        assert!(wrote_second, "the later email must supersede end to end");
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Interviewing
        );
    }

    #[test]
    fn a_user_set_terminal_status_absorbs_even_with_a_recognized_sender() {
        let (_d1, applications, _d2, email_watch) = stores();
        let id = saved_app(&applications);
        applications
            .set_status(&id, ApplicationStatus::Withdrawn, "user withdrew")
            .unwrap();

        let wrote = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Withdrawn,
            Some(EmailIntent::Interview),
            true,
        )
        .unwrap();
        assert!(!wrote, "a user-set terminal status must stay absorbing");
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Withdrawn
        );
    }
}
