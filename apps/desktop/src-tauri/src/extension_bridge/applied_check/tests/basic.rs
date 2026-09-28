use super::super::*;

use super::super::super::test_support::{app_meta, open_store};
use crate::applications::ApplicationOrigin;

// ─────────────────────────────────────────────────────────────────────────────
// C. applied.check — pure store lookup (found saved / found applied / not
// found / malformed url). `resolve_applied_check` is a private, synchronous fn
// (no `AppHandle`), so unlike `handle_import` it IS directly unit-testable —
// these tests exercise the exact boundary `handle_applied_check` calls.
// ─────────────────────────────────────────────────────────────────────────────

/// No Application exists yet for the url → `found: false`, everything else
/// `None`, never an error.
#[test]
fn resolve_applied_check_not_found_when_no_application() {
    let (_dir, store) = open_store();
    let payload = json!({ "url": "https://jobs.example.com/posting/none" });
    let out = resolve_applied_check(&store, &payload).unwrap();
    assert!(!out.found);
    assert!(out.application_id.is_none());
    assert!(out.status.is_none());
    assert!(out.applied_at.is_none());
}

/// A `saved` Application (not yet applied) is found with status "saved", the
/// posting's title, and NO `appliedAt` (it hasn't left `saved`).
#[test]
fn resolve_applied_check_found_saved_has_no_applied_at() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/saved-1";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Backend Engineer"),
            ApplicationOrigin::Saved,
            None,
        )
        .unwrap();

    let out = resolve_applied_check(&store, &json!({ "url": url })).unwrap();
    assert!(out.found);
    assert_eq!(out.status.as_deref(), Some("saved"));
    assert_eq!(out.title.as_deref(), Some("Backend Engineer"));
    assert!(out.applied_at.is_none());
}

/// An `applied` Application is found with status "applied" and a non-null
/// `appliedAt` (epoch ms) — the field the popup formats into a date.
#[test]
fn resolve_applied_check_found_applied_carries_applied_at() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/applied-1";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Staff Engineer"),
            ApplicationOrigin::Saved,
            Some(true),
        )
        .unwrap();

    let out = resolve_applied_check(&store, &json!({ "url": url })).unwrap();
    assert!(out.found);
    assert_eq!(out.status.as_deref(), Some("applied"));
    assert!(
        out.applied_at.is_some(),
        "an applied row must carry applied_at"
    );
}

/// An empty url is a Validation error — never a panic, never a false `found`.
#[test]
fn resolve_applied_check_rejects_empty_url() {
    let (_dir, store) = open_store();
    let err = resolve_applied_check(&store, &json!({ "url": "" })).unwrap_err();
    assert!(err.to_string().contains("required"));
}

/// A non-http(s) url normalizes to empty and is rejected the same way
/// `handle_import` rejects it (dangerous explicit schemes never round-trip).
#[test]
fn resolve_applied_check_rejects_non_http_scheme() {
    let (_dir, store) = open_store();
    let err = resolve_applied_check(&store, &json!({ "url": "javascript:alert(1)" })).unwrap_err();
    assert!(err.to_string().contains("not a valid"));
}

/// `applied_result_reply` builds a well-formed `applied.result` envelope
/// carrying `found`/`status`/`appliedAt` on success (mirrors
/// `profile_result_reply_carries_type_and_req_id`).
#[test]
fn applied_result_reply_carries_type_and_found_flag() {
    let (_dir, store) = open_store();
    let url = "https://jobs.example.com/posting/reply-1";
    store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "QA Engineer"),
            ApplicationOrigin::Saved,
            Some(true),
        )
        .unwrap();
    let outcome = resolve_applied_check(&store, &json!({ "url": url }));
    let reply = applied_result_reply("req-9", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::APPLIED_RESULT);
    assert_eq!(v["reqId"], "req-9");
    assert_eq!(v["payload"]["found"], true);
    assert_eq!(v["payload"]["status"], "applied");
    assert!(v["payload"]["appliedAt"].is_number());
}

/// A malformed (empty) url produces `{ found: false, error }` — never a panic,
/// never a bare `{error}` without `found` (the extension's guard requires it).
#[test]
fn applied_result_reply_carries_error_on_malformed_url() {
    let (_dir, store) = open_store();
    let outcome = resolve_applied_check(&store, &json!({ "url": "" }));
    let reply = applied_result_reply("req-10", outcome);
    let v: serde_json::Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::APPLIED_RESULT);
    assert_eq!(v["payload"]["found"], false);
    assert!(v["payload"]["error"].as_str().unwrap().contains("required"));
}
