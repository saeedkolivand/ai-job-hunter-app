use super::{support::*, *};

#[test]
fn save_then_generate_merges_into_one_application() {
    let (_dir, store) = open_store();

    let saved_id = saved(&store, "https://acme.com/job/1?x=1");
    let gen_id = upsert(
        &store,
        "https://www.acme.com/job/1/",
        "linkedin",
        &meta("", "Senior Engineer"),
        ApplicationOrigin::Generate,
    );

    assert_eq!(saved_id, gen_id, "same normalized url must merge");
    let all = store.list();
    assert_eq!(all.len(), 1);
    let app = &all[0];
    assert_eq!(app.status, ApplicationStatus::Applied);
    assert!(app.applied_at.is_some());
    assert_eq!(app.title, "Senior Engineer");
    assert_eq!(app.company, "Acme");
}

/// Documents WHY `ApplyByEmailTab::persistDraft` refuses to save for a URL-less
/// Application. `row_by_job_url_conn` returns `None` unconditionally for an
/// empty url, so `upsert_for_origin` can never merge and mints a BRAND-NEW
/// `applied` Application on every call. Any renderer surface that saves
/// per-keystroke/per-action for a URL-less job therefore forks a duplicate
/// application each time — the guard in the email tab is what prevents that.
#[test]
fn upsert_for_origin_forks_a_new_application_for_every_empty_url_save() {
    let (_dir, store) = open_store();

    let first = upsert(
        &store,
        "",
        "linkedin",
        &meta("Acme", "Engineer"),
        ApplicationOrigin::Generate,
    );
    let second = upsert(
        &store,
        "",
        "linkedin",
        &meta("Acme", "Engineer"),
        ApplicationOrigin::Generate,
    );

    assert_ne!(
        first, second,
        "an empty url can never match an existing row, so each save mints a new id"
    );
    let all = store.list();
    assert_eq!(all.len(), 2, "two phantom Applications, not one merged row");
    assert!(
        all.iter().all(|a| a.status == ApplicationStatus::Applied),
        "each phantom is created already `applied`, so it sorts to the top of the list"
    );
}

#[test]
fn applied_job_urls_excludes_saved() {
    let (_dir, store) = open_store();
    upsert(
        &store,
        "https://a.com/1",
        "b",
        &meta("A", "T"),
        ApplicationOrigin::Saved,
    );
    upsert(
        &store,
        "https://b.com/2",
        "b",
        &meta("B", "T"),
        ApplicationOrigin::Generate,
    );
    let applied = store.applied_job_urls();
    assert!(applied.contains("https://b.com/2"));
    assert!(!applied.contains("https://a.com/1"), "saved is not applied");
}

/// Round-4 fix T3-cont (PR #1182 round-5) — a query failure (locked/corrupt
/// DB) must be `None`, distinguishable from "queried fine, applied to
/// nothing" (`Some(empty)`); `applied_job_urls` collapses both to empty for
/// its own best-effort callers, but `agent_read`'s `store_present` needs the
/// distinction. Forced with the same second-raw-connection/DROP TABLE trick
/// as `a_broken_follow_up_query_degrades_to_an_empty_sweep_instead_of_panicking`,
/// including its own two-schema-arm note: the connection's cached schema can
/// let one call still succeed (degrading per-row) before a later call
/// re-prepares and fails outright.
#[test]
fn applied_job_urls_checked_returns_none_when_the_query_fails() {
    let (dir, store) = open_store();
    upsert(
        &store,
        "https://gamma.example/1",
        "b",
        &meta("Gamma", "T"),
        ApplicationOrigin::Generate,
    );
    assert_eq!(
        store.applied_job_urls_checked().unwrap().len(),
        1,
        "baseline: the query succeeds before the table goes away"
    );

    {
        let conn = Connection::open(dir.path().join("applications.db")).unwrap();
        conn.execute("DROP TABLE applications", []).unwrap();
    }

    // The first call after the drop may still run against the connection's
    // stale cached schema (degrading per-row, per the sibling test's own
    // "arm 1" note); the goal here is the SECOND call, which re-prepares
    // against the now-broken schema and must fail outright.
    let _ = store.applied_job_urls_checked();
    assert!(
        store.applied_job_urls_checked().is_none(),
        "a query failure must read as None, never the same shape as an empty result"
    );
}

#[test]
fn set_status_appends_event_and_sets_applied_at() {
    let (_dir, store) = open_store();
    let id = upsert(&store, "", "", &meta("C", "T"), ApplicationOrigin::Saved);
    assert_eq!(store.get(&id).unwrap().status, ApplicationStatus::Saved);
    assert!(store.get(&id).unwrap().applied_at.is_none());

    store
        .set_status(&id, ApplicationStatus::Interviewing, "phone screen")
        .unwrap();
    let app = store.get(&id).unwrap();
    assert_eq!(app.status, ApplicationStatus::Interviewing);
    assert!(app.applied_at.is_some(), "leaving saved sets applied_at");

    let events = store.events(&id);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].from_status, "saved");
    assert_eq!(events[1].to_status, "interviewing");
    assert_eq!(events[1].note, "phone screen");
}

#[test]
fn update_fields_patches_only_provided() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T");
    edit(&store, &id, |p| {
        p.notes = Some("call back Tuesday".into());
        p.next_action_at = Some(Some(123));
        p.contact_name = Some("Recruiter".into());
    });
    let app = store.get(&id).unwrap();
    assert_eq!(app.notes, "call back Tuesday");
    assert_eq!(app.next_action_at, Some(123));
    assert_eq!(app.contact_name, "Recruiter");
    assert_eq!(app.comp, "");
}

#[test]
fn delete_removes_application_and_events() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T");
    store.delete(&id, true).unwrap();
    assert!(store.get(&id).is_none());
    assert!(store.events(&id).is_empty());
}

/// MEDIUM: `update_fields` null-vs-absent semantics.
///
/// - `Some(None)` for `next_action_at` must CLEAR the field to `None`.
/// - `None` for `next_action_at` must leave the prior value UNCHANGED.
#[test]
fn update_fields_next_action_at_null_clears_and_absent_preserves() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T");

    // Set a value.
    set_reminder(&store, &id, Some(999));
    assert_eq!(
        store.get(&id).unwrap().next_action_at,
        Some(999),
        "precondition: value set"
    );

    // Passing `Some(None)` must CLEAR the value.
    set_reminder(&store, &id, None);
    assert_eq!(
        store.get(&id).unwrap().next_action_at,
        None,
        "Some(None) must clear next_action_at"
    );

    // Set value again.
    set_reminder(&store, &id, Some(456));
    assert_eq!(
        store.get(&id).unwrap().next_action_at,
        Some(456),
        "precondition: value re-set"
    );

    // Passing `None` (field absent) must PRESERVE the prior value.
    patch(&store, &id, Patch::default()).unwrap();
    assert_eq!(
        store.get(&id).unwrap().next_action_at,
        Some(456),
        "None must leave next_action_at unchanged"
    );
}

/// MEDIUM: `set_status` must advance `updated_at` — assert `>=` old value while
/// also confirming a status_event was appended, which together proves the call
/// was not a no-op.  We avoid `>` because ms-resolution clocks can tick the same
/// value; the event-count assertion is the correctness proof.
#[test]
fn set_status_bumps_updated_at_and_appends_event() {
    let (_dir, store) = open_store();
    let id = track(&store, "C", "T");

    let before = store.get(&id).unwrap().updated_at;
    let events_before = store.events(&id).len();

    store
        .set_status(&id, ApplicationStatus::Screening, "moved to screening")
        .unwrap();

    let after = store.get(&id).unwrap();
    // updated_at must not go backwards.
    assert!(
        after.updated_at >= before,
        "updated_at must advance after set_status (before={before}, after={})",
        after.updated_at
    );
    // The status event is the hard proof that set_status actually ran.
    let events_after = store.events(&id).len();
    assert_eq!(
        events_after,
        events_before + 1,
        "set_status must append exactly one new status event"
    );
    let last_event = store.events(&id).into_iter().last().unwrap();
    assert_eq!(last_event.to_status, "screening");
    assert_eq!(last_event.note, "moved to screening");
}

// ── Gap 2: applied_job_urls excludes saved, includes any non-saved status ─────
//
// The existing `applied_job_urls_excludes_saved` test only checks `saved` vs
// `applied`.  This test also checks that after a `saved` Application is advanced
// to a non-saved status it IS included, covering the transition edge.

#[test]
fn applied_job_urls_includes_application_after_status_leaves_saved() {
    let (_dir, store) = open_store();

    // Create a saved Application.
    let id = upsert(
        &store,
        "https://beta.com/job/1",
        "linkedin",
        &meta("Beta", "Dev"),
        ApplicationOrigin::Saved,
    );

    // Must NOT be in applied_job_urls while still saved.
    assert!(
        !store.applied_job_urls().contains("https://beta.com/job/1"),
        "saved Application must not appear in applied_job_urls"
    );

    // Advance to Screening (a non-saved, non-applied status).
    store
        .set_status(&id, ApplicationStatus::Screening, "phone screen booked")
        .unwrap();

    // NOW it must appear.
    assert!(
        store.applied_job_urls().contains("https://beta.com/job/1"),
        "Application must appear in applied_job_urls after leaving saved"
    );
}
