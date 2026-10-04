use super::super::validation::{require_non_empty_id, resolve_status_event_action};
use crate::applications::{ApplicationMeta, ApplicationStatus, ApplicationStore};
use crate::error::AppError;

// ── v2 slice 3: accept/reject over the command layer ────────────────────

fn meta_for_status_events() -> ApplicationMeta {
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

/// A fresh store seeded with one `Interviewing` application carrying an
/// unconfirmed, email-derived `Rejected` write — the exact shape
/// accept/reject act on.
fn seeded_store() -> (tempfile::TempDir, ApplicationStore, String) {
    let dir = tempfile::TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = store
        .track_manual("", "", &meta_for_status_events())
        .unwrap();
    store
        .set_status(&id, ApplicationStatus::Interviewing, "")
        .unwrap();
    store
        .transition_status_if_sourced(
            &id,
            ApplicationStatus::Interviewing,
            ApplicationStatus::Rejected,
            Some("email-derived (unconfirmed)"),
            crate::applications::EVENT_SOURCE_EMAIL,
            false,
        )
        .unwrap();
    (dir, store, id)
}

#[test]
fn require_non_empty_id_rejects_empty_and_whitespace_only() {
    assert!(require_non_empty_id("").is_err());
    assert!(require_non_empty_id("   ").is_err());
    assert!(require_non_empty_id("abc123").is_ok());
}

/// MINOR fix: the RETURNED value must be trimmed, not just checked for
/// emptiness after trimming — a caller using the original `id` instead
/// of this fn's return value would forward `" app-1-abcd1234 "`
/// untrimmed, matching zero rows in the store and silently reporting
/// success. Pins the exact reproduction from the fix-forward task.
#[test]
fn require_non_empty_id_returns_the_trimmed_value() {
    assert_eq!(require_non_empty_id(" abc ").unwrap(), "abc");
    assert_eq!(
        require_non_empty_id(" app-1-abcd1234 ").unwrap(),
        "app-1-abcd1234"
    );
}

#[test]
fn accept_over_the_command_layer_matches_the_direct_store_call() {
    // Two IDENTICALLY-seeded stores: one mutated through the command-layer
    // pure core, the other through the store method directly. Both must
    // end up in the SAME observable state.
    let (_dir_a, store_a, id_a) = seeded_store();
    let (_dir_b, store_b, id_b) = seeded_store();
    let event_a = store_a.events(&id_a).into_iter().last().unwrap().event_id;
    let event_b = store_b.events(&id_b).into_iter().last().unwrap().event_id;

    let via_command = resolve_status_event_action(
        &store_a,
        &id_a,
        event_a,
        ApplicationStore::accept_status_event,
    );
    let via_direct = store_b.accept_status_event(&id_b, event_b);
    assert!(via_command.unwrap(), "command-layer path must succeed");
    assert!(via_direct.unwrap(), "direct store call must succeed");

    assert_eq!(
        store_a.get(&id_a).unwrap().status,
        store_b.get(&id_b).unwrap().status
    );
    let last_a = store_a.events(&id_a).into_iter().last().unwrap();
    let last_b = store_b.events(&id_b).into_iter().last().unwrap();
    assert_eq!(last_a.source, last_b.source);
    assert_eq!(last_a.confirmed, last_b.confirmed);
    assert!(last_a.confirmed, "accept must set the confirmed flag");
}

#[test]
fn reject_over_the_command_layer_matches_the_direct_store_call() {
    let (_dir_a, store_a, id_a) = seeded_store();
    let (_dir_b, store_b, id_b) = seeded_store();
    let event_a = store_a.events(&id_a).into_iter().last().unwrap().event_id;
    let event_b = store_b.events(&id_b).into_iter().last().unwrap().event_id;

    let via_command = resolve_status_event_action(
        &store_a,
        &id_a,
        event_a,
        ApplicationStore::reject_status_event,
    );
    let via_direct = store_b.reject_status_event(&id_b, event_b);
    assert!(via_command.unwrap(), "command-layer path must succeed");
    assert!(via_direct.unwrap(), "direct store call must succeed");

    assert_eq!(
        store_a.get(&id_a).unwrap().status,
        store_b.get(&id_b).unwrap().status
    );
    assert_eq!(
        store_a.get(&id_a).unwrap().status,
        ApplicationStatus::Interviewing,
        "reject must revert to the pre-email status"
    );
    assert_eq!(store_a.events(&id_a).len(), store_b.events(&id_b).len());
}

#[test]
fn accept_over_the_command_layer_rejects_an_empty_id_without_touching_the_store() {
    let (_dir, store, id) = seeded_store();
    let event_id = store.events(&id).into_iter().last().unwrap().event_id;
    let result =
        resolve_status_event_action(&store, "", event_id, ApplicationStore::accept_status_event);
    assert!(matches!(result, Err(AppError::Validation(_))));
    // The real (non-empty) application's pending row must be untouched.
    let last = store.events(&id).into_iter().last().unwrap();
    assert!(!last.confirmed);
}

/// MINOR fix, end to end: a whitespace-padded id (renderer input with
/// stray leading/trailing whitespace) must still resolve — before the
/// fix, this cleared `require_non_empty_id`'s check (non-empty after
/// trimming) but then forwarded the UNTRIMMED id to the store, which
/// matches zero rows and silently returns `Ok(false)` — the exact
/// "nothing happened" success this test would NOT have caught if it
/// only checked `require_non_empty_id` in isolation.
#[test]
fn accept_over_the_command_layer_trims_a_whitespace_padded_id() {
    let (_dir, store, id) = seeded_store();
    let event_id = store.events(&id).into_iter().last().unwrap().event_id;
    let padded = format!(" {id} ");
    let result = resolve_status_event_action(
        &store,
        &padded,
        event_id,
        ApplicationStore::accept_status_event,
    );
    assert!(
        result.unwrap(),
        "a whitespace-padded id must still match the real row"
    );
    let last = store.events(&id).into_iter().last().unwrap();
    assert!(last.confirmed, "the real row must have been accepted");
}
