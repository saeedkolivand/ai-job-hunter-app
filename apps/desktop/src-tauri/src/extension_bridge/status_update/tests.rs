//! `resolve_status_update`'s saved→applied transition + refusal paths, then
//! `status_result_reply`'s wire shape. Dispatch-classification: `frame_advance::tests::
//! classify_write`. AUTO-flag gate: `auto_flag_gate_tests` (shared with `answers_save`).

use super::super::test_support::{app_meta, open_store};
use super::*;
use crate::applications::{ApplicationOrigin, ApplicationStatus};

/// The happy path: a `saved` Application at the exact url transitions to
/// `applied` — the status event is appended with the fixed note "via
/// extension" (no page-derived text) and `applied_at` is set.
#[test]
fn resolve_status_update_transitions_saved_to_applied_and_appends_event() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/status-1";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Backend Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let out = resolve_status_update(&store, &json!({ "url": url, "to": "applied" }))
        .expect("saved -> applied must succeed");
    assert_eq!(out.application_id, id);
    assert_eq!(out.status, "applied");

    let row = store.get(&id).unwrap();
    assert_eq!(row.status, ApplicationStatus::Applied);
    assert!(row.applied_at.is_some(), "applied_at must be set");

    let events = store.events(&id);
    let last = events.last().expect("set_status must append an event");
    assert_eq!(last.to_status, "applied");
    assert_eq!(
        last.note, "via extension",
        "the status event note must be a short fixed string — never page-derived text"
    );
}

/// An AUTOMATED write (`auto: true`, Task #22's gesture submit-watcher)
/// appends a DISTINCT history note from a deliberate popup click — mirrors
/// `handle_status_update`'s notification-body discrimination — so the
/// Application's own status-event history records HOW it was applied.
#[test]
fn resolve_status_update_auto_write_appends_distinct_history_note() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/status-auto-1";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Auto-tracked Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    resolve_status_update(
        &store,
        &json!({ "url": url, "to": "applied", "auto": true }),
    )
    .expect("saved -> applied must succeed for an auto write");

    let events = store.events(&id);
    let last = events.last().expect("set_status must append an event");
    assert_eq!(last.to_status, "applied");
    assert_eq!(
        last.note, "auto-tracked via extension",
        "an auto write must record a distinct history note from a manual click"
    );
}

/// Regression guard for the non-atomic-guard review finding (HIGH): the
/// desktop-side guard is `ApplicationStore::transition_status_if`, a single
/// atomic compare-and-set. Simulate two callers racing the exact same
/// `saved -> applied` transition by calling it twice directly — only the
/// FIRST may succeed; the SECOND must lose the race (`Ok(false)`), appending
/// no second status event and never re-bumping `applied_at`.
#[test]
fn transition_status_if_second_call_after_already_applied_loses_the_race() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/race-1";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Race Condition Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let first = store
        .transition_status_if(
            &id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .unwrap();
    assert!(first, "the first caller wins the race");
    let applied_at_after_first = store.get(&id).unwrap().applied_at;
    let events_after_first = store.events(&id).len();

    // A second caller racing the same transition after the first already
    // committed (e.g. two extension clicks, or a retry) must lose — never a
    // second write, never a duplicate status event.
    let second = store
        .transition_status_if(
            &id,
            ApplicationStatus::Saved,
            ApplicationStatus::Applied,
            Some("via extension"),
        )
        .unwrap();
    assert!(!second, "the second racing caller must lose (Ok(false))");
    assert_eq!(
        store.events(&id).len(),
        events_after_first,
        "the losing call must not append a second status event"
    );
    assert_eq!(
        store.get(&id).unwrap().applied_at,
        applied_at_after_first,
        "the losing call must not re-bump applied_at"
    );
}

/// A row that is ALREADY `applied` refuses — this verb can never re-fire the
/// transition (no "re-apply", no bumping `applied_at` again).
#[test]
fn resolve_status_update_refuses_when_already_applied() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/status-2";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Staff Engineer"),
            ApplicationOrigin::Saved,
            Some(true), // already applied
        )
        .unwrap();

    let err = resolve_status_update(&store, &json!({ "url": url, "to": "applied" })).unwrap_err();
    assert!(err.to_string().contains("no longer saved"), "got: {err}");
}

/// A row mid-pipeline (picked: `interviewing`) also refuses — the allowlist is
/// `saved -> applied` ONLY, not "anything not yet applied".
#[test]
fn resolve_status_update_refuses_mid_pipeline_status() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/status-3";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Platform Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();
    store
        .set_status(&id, ApplicationStatus::Interviewing, "test setup")
        .unwrap();

    let err = resolve_status_update(&store, &json!({ "url": url, "to": "applied" })).unwrap_err();
    assert!(err.to_string().contains("no longer saved"), "got: {err}");

    // The refusal must never have written anything — status stays exactly as
    // the test setup left it.
    let row = store.get(&id).unwrap();
    assert_eq!(row.status, ApplicationStatus::Interviewing);
}

/// No Application exists for the url at all → a clear "couldn't find" refusal,
/// never a create-on-miss.
#[test]
fn resolve_status_update_rejects_when_no_match() {
    let (_dir, store) = open_store();
    let err = resolve_status_update(
        &store,
        &json!({ "url": "https://jobs.example.com/posting/none", "to": "applied" }),
    )
    .unwrap_err();
    assert!(err.to_string().contains("couldn't find"), "got: {err}");
}

/// An empty url is a Validation error — never a panic.
#[test]
fn resolve_status_update_rejects_empty_url() {
    let (_dir, store) = open_store();
    let err = resolve_status_update(&store, &json!({ "url": "", "to": "applied" })).unwrap_err();
    assert!(err.to_string().contains("required"));
}

/// A non-empty but malformed url (an explicit non-http(s) scheme — the same
/// chokepoint `resolve_applied_check_rejects_non_http_scheme` exercises) hits
/// the DISTINCT "url is not a valid http(s) URL" sentinel — never confused
/// with "url is required" (empty input, tested above) or "couldn't find" (a
/// well-formed url with no matching row, tested below).
#[test]
fn resolve_status_update_rejects_malformed_url() {
    let (_dir, store) = open_store();
    let err = resolve_status_update(
        &store,
        &json!({ "url": "javascript:alert(1)", "to": "applied" }),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("not a valid"),
        "a malformed/dangerous-scheme url must hit the distinct invalid-url sentinel, got: {err}"
    );
}

/// `to` must literal-match "applied" — re-validated on the Rust side
/// independently of the TS zod literal. Any other value (including a missing
/// field) is rejected BEFORE the store is even queried.
#[test]
fn resolve_status_update_rejects_to_not_applied() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/status-4";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "QA Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let wrong_value =
        resolve_status_update(&store, &json!({ "url": url, "to": "interviewing" })).unwrap_err();
    assert!(
        wrong_value.to_string().contains("unsupported"),
        "got: {wrong_value}"
    );

    let missing_field = resolve_status_update(&store, &json!({ "url": url })).unwrap_err();
    assert!(
        missing_field.to_string().contains("unsupported"),
        "got: {missing_field}"
    );

    // Neither rejected attempt may have written anything.
    let row = store.list().into_iter().find(|a| a.job_url == url).unwrap();
    assert_eq!(row.status, ApplicationStatus::Saved);
}

/// `status_result_reply` builds a well-formed `status.update` success envelope
/// (mirrors `applied_result_reply_carries_type_and_found_flag`).
#[test]
fn status_result_reply_carries_ok_true_and_fields() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/status-5";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Reply Test Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();
    let outcome = resolve_status_update(&store, &json!({ "url": url, "to": "applied" }));
    let reply = status_result_reply("req-11", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::STATUS_RESULT);
    assert_eq!(v["reqId"], "req-11");
    assert_eq!(v["payload"]["ok"], true);
    assert_eq!(v["payload"]["status"], "applied");
    assert!(v["payload"]["applicationId"].is_string());
}

/// UNLIKE `applied.result`, a `status.update` failure carries a fixed,
/// user-facing `error` string — this verb's errors are surfaced to the user
/// (it answers a deliberate click), so the reply must never fold the failure
/// into a silent/blank payload.
#[test]
fn status_result_reply_carries_user_facing_error() {
    let (_dir, store) = open_store();
    let outcome = resolve_status_update(
        &store,
        &json!({ "url": "https://jobs.example.com/posting/none", "to": "applied" }),
    );
    let reply = status_result_reply("req-12", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::STATUS_RESULT);
    assert_eq!(v["payload"]["ok"], false);
    assert!(v["payload"]["error"]
        .as_str()
        .unwrap()
        .contains("couldn't find"));
}
