use super::{support::*, *};

/// What the extension bridge's `status.update` guard does: `transition_status_if`
/// `Saved -> Applied` with the note `via extension`. `true` iff the CAS won.
fn extension_marks_applied(store: &ApplicationStore, id: &str) -> bool {
    store
        .transition_status_if(
            id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .unwrap()
}

/// `transition_status_if` (the CAS the extension bridge's `status.update`
/// guard relies on): a matching `from` transitions the row, appends exactly
/// one status event, and sets `applied_at` — same field semantics as
/// `set_status`.
#[test]
fn transition_status_if_matches_from_transitions_and_appends_event() {
    let (_dir, store) = open_store();
    let id = upsert(
        &store,
        "https://x.com/1",
        "b",
        &meta("C", "T"),
        ApplicationOrigin::Saved,
    );
    let events_before = store.events(&id).len();

    let ok = extension_marks_applied(&store, &id);
    assert!(ok, "a matching `from` must transition and return true");

    let app = store.get(&id).unwrap();
    assert_eq!(app.status, ApplicationStatus::Applied);
    assert!(
        app.applied_at.is_some(),
        "applied_at must be set on saved -> applied"
    );

    let events_after = store.events(&id);
    assert_eq!(
        events_after.len(),
        events_before + 1,
        "exactly one event appended"
    );
    let last = events_after.last().unwrap();
    assert_eq!(last.from_status, "saved");
    assert_eq!(last.to_status, "applied");
    assert_eq!(last.note, "via extension");
}

/// A `from` that does NOT match the row's current status is refused
/// (`Ok(false)`) — no write, no event appended. This is the compare-and-set
/// guard itself: it must never transition an unexpected starting status.
#[test]
fn transition_status_if_refuses_when_from_does_not_match_current_status() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T"); // starts `applied`
    let events_before = store.events(&id).len();

    let ok = extension_marks_applied(&store, &id);
    assert!(!ok, "a from mismatch must refuse (Ok(false)), never write");

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Applied,
        "status must be unchanged"
    );
    assert_eq!(
        store.events(&id).len(),
        events_before,
        "no event appended on refusal"
    );
}

/// The lost-race scenario the review flagged: two callers race the same
/// saved->applied transition. Only the FIRST succeeds; the SECOND must see
/// `Ok(false)` (the guard lost the race) — never a second event, never a
/// re-bumped `applied_at`.
#[test]
fn transition_status_if_second_racing_call_loses_and_appends_nothing() {
    let (_dir, store) = open_store();
    let id = upsert(
        &store,
        "https://race.example/1",
        "b",
        &meta("C", "T"),
        ApplicationOrigin::Saved,
    );

    let first = extension_marks_applied(&store, &id);
    assert!(first, "the first call wins the race");
    let applied_at_after_first = store.get(&id).unwrap().applied_at;
    let events_after_first = store.events(&id).len();

    // Simulate the lost race: a second concurrent caller attempts the exact
    // same saved -> applied transition after the first already committed.
    let second = extension_marks_applied(&store, &id);
    assert!(!second, "the second call must lose the race (Ok(false))");

    assert_eq!(
        store.events(&id).len(),
        events_after_first,
        "the losing call must not append a second status event"
    );
    assert_eq!(
        store.get(&id).unwrap().applied_at,
        applied_at_after_first,
        "the losing call must not bump applied_at again"
    );
}

/// A `saved` row CAN already carry a prior `applied_at` — from an earlier
/// `applied -> saved` demotion via the stage picker (`set_status` never clears
/// `applied_at` on a demotion to a pre-apply status). Re-transitioning that row
/// back to `applied` through `transition_status_if` (the extension bridge's
/// guard) must preserve the ORIGINAL `applied_at`, not stamp a fresh `now()` —
/// first-applied-wins, same semantics as `set_status`.
#[test]
fn transition_status_if_preserves_prior_applied_at_after_demotion_round_trip() {
    let (dir, store) = open_store();
    let id = upsert(
        &store,
        "https://demote.example/1",
        "b",
        &meta("C", "T"),
        ApplicationOrigin::Saved,
    );

    // Simulate the applied -> saved round-trip via `set_status` (the stage
    // picker's path): leaving saved sets applied_at, then demoting back to
    // saved must NOT clear it.
    store
        .set_status(&id, ApplicationStatus::Applied, "applied")
        .unwrap();
    let original_applied_at = store.get(&id).unwrap().applied_at;
    assert!(
        original_applied_at.is_some(),
        "leaving saved must set applied_at"
    );

    store
        .set_status(&id, ApplicationStatus::Saved, "demoted back to saved")
        .unwrap();
    assert_eq!(
        store.get(&id).unwrap().applied_at,
        original_applied_at,
        "a saved demotion must not clear the prior applied_at"
    );

    // Pin an unmistakably distinct sentinel directly on the row so the final
    // assertion can't pass by clock-resolution coincidence with `now()` — it
    // must come from a genuine COALESCE preservation, not a lucky same-ms read.
    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute(
            "UPDATE applications SET applied_at = 12345 WHERE id = ?1",
            params![id],
        )
        .unwrap();
    }

    let ok = extension_marks_applied(&store, &id);
    assert!(ok, "saved -> applied must still transition");

    let app = store.get(&id).unwrap();
    assert_eq!(app.status, ApplicationStatus::Applied);
    assert_eq!(
        app.applied_at,
        Some(12345),
        "transition_status_if must preserve the prior applied_at (first-applied-wins), not stamp a fresh now()"
    );
}

/// If the status-event INSERT fails, the whole transaction must roll back —
/// no status flip with a missing history row. Forces the failure by dropping
/// `status_events` out from under the store via a second raw connection to
/// the same db file (same trick the demotion round-trip test above uses to
/// poke the row directly), then asserts the row is UNCHANGED afterward.
#[test]
fn transition_status_if_rolls_back_status_when_event_insert_fails() {
    let (dir, store) = open_store();
    let id = upsert(
        &store,
        "https://rollback.example/1",
        "b",
        &meta("C", "T"),
        ApplicationOrigin::Saved,
    );

    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute("DROP TABLE status_events", []).unwrap();
    }

    let err = store
        .transition_status_if(
            &id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .expect_err("the event insert must fail (no such table) and propagate");
    let _ = err; // exact AppError variant isn't the contract here, only that it's Err

    let app = store.get(&id).unwrap();
    assert_eq!(
        app.status,
        ApplicationStatus::Saved,
        "the status UPDATE must roll back together with the failed event insert"
    );
    assert!(
        app.applied_at.is_none(),
        "applied_at must not be stamped when the whole transaction rolled back"
    );
}
