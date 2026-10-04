use tempfile::TempDir;

use super::*;
use crate::applications::{ApplicationMeta, ApplicationOrigin, ApplicationStatus, StatusEvent};

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
struct Fixture {
    _apps_dir: TempDir,
    applications: ApplicationStore,
    _email_dir: TempDir,
    email_watch: EmailWatchStore,
}

impl Fixture {
    fn new() -> Self {
        let apps_dir = TempDir::new().unwrap();
        let applications = ApplicationStore::open(apps_dir.path()).unwrap();
        let email_dir = TempDir::new().unwrap();
        let email_watch = EmailWatchStore::open(&email_dir.path().to_path_buf()).unwrap();
        email_watch
            .connect("jane@example.com", "imap.example.com", 993)
            .unwrap();
        assert!(email_watch.set_auto_write_enabled(true).unwrap());
        Self {
            _apps_dir: apps_dir,
            applications,
            _email_dir: email_dir,
            email_watch,
        }
    }

    /// A fresh `saved` application id.
    fn saved_app(&self) -> String {
        self.applications
            .upsert_for_origin(
                "https://x.example/1",
                "b",
                &meta(),
                ApplicationOrigin::Saved,
                None,
            )
            .unwrap()
    }

    fn apply(&self, id: &str, intent: Option<EmailIntent>, write_authorized: bool) -> bool {
        apply_matched_intent(
            &self.applications,
            &self.email_watch,
            id,
            intent,
            write_authorized,
        )
        .unwrap()
    }

    fn status(&self, id: &str) -> ApplicationStatus {
        self.applications.get(id).unwrap().status
    }

    fn set_status(&self, id: &str, status: ApplicationStatus, note: &str) {
        self.applications.set_status(id, status, note).unwrap();
    }

    fn event_count(&self, id: &str) -> usize {
        self.applications.events(id).len()
    }

    fn last_event(&self, id: &str) -> StatusEvent {
        self.applications.events(id).into_iter().last().unwrap()
    }
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

#[test]
fn a_decided_intent_writes_an_unconfirmed_status_change() {
    let fx = Fixture::new();
    let id = fx.saved_app();

    let wrote = fx.apply(&id, Some(EmailIntent::Confirmation), true);
    assert!(wrote);

    let app = fx.applications.get(&id).unwrap();
    assert_eq!(app.status, ApplicationStatus::Applied);

    let last = fx.last_event(&id);
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
    let fx = Fixture::new();
    let id = fx.saved_app();

    let wrote = fx.apply(&id, Some(EmailIntent::Confirmation), false);
    assert!(
        !wrote,
        "a cold/unauthorized sender (write_authorized=false) must never write"
    );
    assert_eq!(fx.status(&id), ApplicationStatus::Saved);
}

#[test]
fn a_none_intent_never_writes_even_with_a_recognized_sender() {
    // German confirmations (zero discriminating phrases) and any other
    // genuinely-undecided message classify as `None` — this must be a
    // real, directly-testable no-op, not just "the caller happened not
    // to call this function".
    let fx = Fixture::new();
    let id = fx.saved_app();

    let wrote = fx.apply(&id, None, true);
    assert!(!wrote, "a None intent must never write");
    assert_eq!(fx.status(&id), ApplicationStatus::Saved);
}

#[test]
fn the_auto_write_toggle_off_blocks_the_write_entirely() {
    let fx = Fixture::new();
    let id = fx.saved_app();
    // `Fixture::new()` already connected (and opted IN) — `set_auto_write_enabled`
    // guards on `address IS NOT NULL` (same concurrent-clear discipline
    // as `set_enabled`), which that connect already satisfies.
    let toggled = fx.email_watch.set_auto_write_enabled(false).unwrap();
    assert!(toggled, "the toggle write must succeed once connected");

    let wrote = fx.apply(&id, Some(EmailIntent::Confirmation), true);
    assert!(!wrote, "the toggle off must block the write");
    assert_eq!(fx.status(&id), ApplicationStatus::Saved);
}

#[test]
fn a_non_advancing_intent_is_a_silent_no_op() {
    // Confirmation intent while already at Offer: next_status says no-op
    // (never a downgrade) — this must propagate as a clean `false`, not
    // an error, and must not touch the status or append anything.
    let fx = Fixture::new();
    let id = fx.saved_app();
    fx.set_status(&id, ApplicationStatus::Offer, "");
    let events_before = fx.event_count(&id);

    let wrote = fx.apply(&id, Some(EmailIntent::Confirmation), true);
    assert!(!wrote);
    assert_eq!(fx.status(&id), ApplicationStatus::Offer);
    assert_eq!(fx.event_count(&id), events_before);
}

#[test]
fn a_second_later_email_does_not_reapply_a_status_the_user_already_rejected() {
    let fx = Fixture::new();
    let id = fx.saved_app();
    fx.set_status(&id, ApplicationStatus::Interviewing, "");

    // Email 1: a rejection intent auto-writes Interviewing -> Rejected,
    // unconfirmed.
    let wrote_first = fx.apply(&id, Some(EmailIntent::Rejection), true);
    assert!(wrote_first);
    assert_eq!(fx.status(&id), ApplicationStatus::Rejected);

    // The user rejects it — status reverts to Interviewing.
    let pending_event_id = fx.last_event(&id).event_id;
    let reverted = fx
        .applications
        .reject_status_event(&id, pending_event_id)
        .unwrap();
    assert!(reverted);
    assert_eq!(fx.status(&id), ApplicationStatus::Interviewing);
    let events_after_reject = fx.event_count(&id);

    // Email 2 (later): the SAME intent, from the SAME now-current status
    // — must NOT re-apply Rejected.
    let wrote_second = fx.apply(&id, Some(EmailIntent::Rejection), true);
    assert!(
        !wrote_second,
        "a later email must not re-apply a status the user already rejected"
    );
    assert_eq!(
        fx.status(&id),
        ApplicationStatus::Interviewing,
        "status must stay exactly where the user left it"
    );
    assert_eq!(
        fx.event_count(&id),
        events_after_reject,
        "the blocked second write must append nothing"
    );
}

/// MAJOR fix: two matched messages for the SAME application in one
/// tick — an ordinary ATS thread, a confirmation then a later
/// rejection inside one 15-minute window — used to both be handed the
/// SAME pre-tick status snapshot by `email_watch_scheduler`'s loop
/// (see [`apply_matched_intent`]'s own doc). The first call's CAS
/// would succeed and move the row; the second's CAS then raced the
/// STALE snapshot against a row the first call had ALREADY changed,
/// lost, and returned `Ok(false)` — silently, permanently dropping the
/// second message (its uid was already stamped by `mark_seen` before
/// any of this ran, so a later tick never reconsiders it). This test
/// mirrors the scheduler's own call shape exactly: TWO calls back to
/// back for the SAME application id, with nothing in between re-
/// reading or re-setting status — that gap is now closed INSIDE
/// `apply_matched_intent` itself (it reads live status per call), not
/// by this test doing the caller's job for it. Both must land.
#[test]
fn two_matched_outcomes_for_one_application_in_one_tick_both_land() {
    let fx = Fixture::new();
    let id = fx.saved_app(); // starts `Saved`
    let events_before = fx.event_count(&id);

    // Outcome A: a confirmation email — Saved -> Applied.
    let wrote_first = fx.apply(&id, Some(EmailIntent::Confirmation), true);
    assert!(wrote_first, "the first outcome must land");
    assert_eq!(fx.status(&id), ApplicationStatus::Applied);

    // Outcome B: a later rejection in the SAME tick — must roll forward
    // from what outcome A just wrote (Applied -> Rejected), not from the
    // stale pre-tick Saved snapshot outcome A itself started from.
    let wrote_second = fx.apply(&id, Some(EmailIntent::Rejection), true);
    assert!(
        wrote_second,
        "the second outcome in the same tick must not be silently \
             dropped by racing a stale snapshot"
    );
    assert_eq!(fx.status(&id), ApplicationStatus::Rejected);
    assert_eq!(
        fx.event_count(&id),
        events_before + 2,
        "both writes must have appended their own event — neither was lost"
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
    let fx = Fixture::new();
    let id = fx.saved_app();
    fx.set_status(&id, ApplicationStatus::Interviewing, "");

    let wrote_first = fx.apply(&id, Some(EmailIntent::Rejection), true);
    assert!(wrote_first);
    assert_eq!(fx.status(&id), ApplicationStatus::Rejected);
    assert!(fx
        .applications
        .current_status_is_unconfirmed_email_write(&id));

    let wrote_second = fx.apply(&id, Some(EmailIntent::Interview), true);
    assert!(
        wrote_second,
        "a later email must be able to supersede its OWN still-unconfirmed \
             terminal write -- an application must not freeze forever"
    );
    assert_eq!(fx.status(&id), ApplicationStatus::Interviewing);
    let last = fx.last_event(&id);
    assert_eq!(last.source, crate::applications::EVENT_SOURCE_EMAIL);
    assert!(!last.confirmed);
}

#[test]
fn a_confirmed_terminal_status_still_absorbs_every_later_email() {
    // Once the user (or an Accept) confirms the terminal status, it is
    // no longer speculation -- it goes back to absorbing, exactly like
    // a user-set terminal always has.
    let fx = Fixture::new();
    let id = fx.saved_app();
    fx.set_status(&id, ApplicationStatus::Interviewing, "");

    fx.apply(&id, Some(EmailIntent::Rejection), true);
    let pending_event_id = fx.last_event(&id).event_id;
    assert!(fx
        .applications
        .accept_status_event(&id, pending_event_id)
        .unwrap());
    assert!(!fx
        .applications
        .current_status_is_unconfirmed_email_write(&id));

    let wrote = fx.apply(&id, Some(EmailIntent::Interview), true);
    assert!(
        !wrote,
        "a CONFIRMED terminal status must still absorb, same as a user-set one"
    );
    assert_eq!(fx.status(&id), ApplicationStatus::Rejected);
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
    let fx = Fixture::new();
    let id = fx.saved_app();
    fx.set_status(&id, ApplicationStatus::Interviewing, "");

    // Email 1: rejection auto-writes an unconfirmed terminal Rejected.
    let wrote_first = fx.apply(&id, Some(EmailIntent::Rejection), true);
    assert!(wrote_first);
    assert_eq!(fx.status(&id), ApplicationStatus::Rejected);

    // MATCHER step: with the app now Rejected-but-unconfirmed, it must
    // still be found as a candidate -- exactly what the caller
    // (`email_watch_scheduler`) computes via
    // `current_status_is_unconfirmed_email_write` and threads through
    // `poller::run_tick` into `matcher::best_match`.
    let app_row = fx.applications.get(&id).unwrap();
    let mut unconfirmed_ids = std::collections::HashSet::new();
    if fx
        .applications
        .current_status_is_unconfirmed_email_write(&id)
    {
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
    let wrote_second = fx.apply(&id, Some(EmailIntent::Interview), true);
    assert!(wrote_second, "the later email must supersede end to end");
    assert_eq!(fx.status(&id), ApplicationStatus::Interviewing);
}

#[test]
fn a_user_set_terminal_status_absorbs_even_with_a_recognized_sender() {
    let fx = Fixture::new();
    let id = fx.saved_app();
    fx.set_status(&id, ApplicationStatus::Withdrawn, "user withdrew");

    let wrote = fx.apply(&id, Some(EmailIntent::Interview), true);
    assert!(!wrote, "a user-set terminal status must stay absorbing");
    assert_eq!(fx.status(&id), ApplicationStatus::Withdrawn);
}
