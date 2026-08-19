//! v2 slice 2: turn one classified [`EmailIntent`] into a real, but always
//! **UNCONFIRMED**, [`Application`](crate::applications::Application) status
//! write. Nothing here writes `confirmed: true` — ever (see
//! [`apply_matched_intent`]'s doc, and [`crate::email_watch::intent`]'s
//! module doc on the classifier's recorded precision limit, which is why
//! adjudication — not withholding the write — is the safety model).
//!
//! Not yet wired into [`super::poller`]'s tick itself: `run_tick` stays pure
//! matching (see its own module doc), and the SCHEDULER (L2, the one place
//! in this module family with `AppHandle` reach) is the natural future
//! caller once it exists; that wiring is a later slice.

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
/// Every one of the following is a legitimate no-op (`Ok(false)`), never an
/// error:
/// - [`EmailWatchStore::auto_write_enabled`] is off (checked FIRST, before
///   computing anything);
/// - [`next_status`] itself says no-op — a terminal `current_status`, or the
///   intent doesn't advance the ladder. (A `None` *intent* is not this
///   function's concern at all: it takes a concrete [`EmailIntent`], never
///   an `Option` — the caller simply never calls this for a message
///   `crate::email_watch::intent::classify_intent` returned `None` for, so
///   "a `None` intent must not write anything" holds by construction, not
///   by a runtime check here);
/// - this exact `(current_status, target)` transition was already rejected
///   by the user for this application
///   ([`ApplicationStore::was_transition_rejected`]) — a later email must
///   not re-apply a status the user has already told us was wrong;
/// - the compare-and-set itself loses (the application's status changed
///   since the caller last read it — the same race every other
///   `transition_status_if`-family caller already tolerates).
///
/// **Hard constraint: always writes `confirmed = false`.** Nothing in this
/// function, or reachable from it, may ever pass `true` for the write this
/// function performs — the unconfirmed row IS the whole safety model.
pub fn apply_matched_intent(
    applications: &ApplicationStore,
    email_watch: &EmailWatchStore,
    application_id: &str,
    current_status: ApplicationStatus,
    intent: EmailIntent,
) -> AppResult<bool> {
    if !email_watch.auto_write_enabled() {
        return Ok(false);
    }
    let Some(target) = next_status(intent, current_status) else {
        return Ok(false);
    };
    if applications.was_transition_rejected(application_id, current_status, target) {
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
    /// state to seed beyond each store's own migrations).
    fn stores() -> (TempDir, ApplicationStore, TempDir, EmailWatchStore) {
        let apps_dir = TempDir::new().unwrap();
        let applications = ApplicationStore::open(apps_dir.path()).unwrap();
        let email_dir = TempDir::new().unwrap();
        let email_watch = EmailWatchStore::open(&email_dir.path().to_path_buf()).unwrap();
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
            EmailIntent::Confirmation,
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
    fn the_auto_write_toggle_off_blocks_the_write_entirely() {
        let (_d1, applications, _d2, email_watch) = stores();
        let id = saved_app(&applications);
        // `set_auto_write_enabled` guards on `address IS NOT NULL` (same
        // concurrent-clear discipline as `set_enabled`) — connect first, as
        // a real caller (IPC, a later slice) always would before this
        // toggle is reachable at all.
        email_watch
            .connect("jane@example.com", "imap.example.com", 993)
            .unwrap();
        let toggled = email_watch.set_auto_write_enabled(false).unwrap();
        assert!(toggled, "the toggle write must succeed once connected");

        let wrote = apply_matched_intent(
            &applications,
            &email_watch,
            &id,
            ApplicationStatus::Saved,
            EmailIntent::Confirmation,
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
            EmailIntent::Confirmation,
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
            EmailIntent::Rejection,
        )
        .unwrap();
        assert!(wrote_first);
        assert_eq!(
            applications.get(&id).unwrap().status,
            ApplicationStatus::Rejected
        );

        // The user rejects it — status reverts to Interviewing.
        let reverted = applications.reject_latest_status_event(&id).unwrap();
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
            EmailIntent::Rejection,
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
}
