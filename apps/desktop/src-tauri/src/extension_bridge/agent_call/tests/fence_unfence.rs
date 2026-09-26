//! Tests for the inbound unfence mirror (`reshape/unfence.rs`).

use super::super::reshape::*;
use super::super::*;

/// The exact shape `commands::scrape::scrape_persist_job`'s OWN
/// `unfence_job_field` already fixed at its one call site — pinned here at
/// the centralised chokepoint too, so a future writer needs no per-call-site
/// code to get the same protection.
#[test]
fn unfence_named_fields_recursive_strips_a_wrapper_a_caller_echoed_back() {
    let mut input = json!({
        "title": "<job_posting>\nStaff Engineer\n</job_posting>",
        "company": "<job_posting>\nAcme Corp\n</job_posting>",
        "id": "job-1",
    });
    unfence_named_fields_recursive(&mut input);
    assert_eq!(input["title"].as_str().unwrap(), "Staff Engineer");
    assert_eq!(input["company"].as_str().unwrap(), "Acme Corp");
    // Never touches a field that isn't a known posting-text carrier.
    assert_eq!(input["id"].as_str().unwrap(), "job-1");
}

#[test]
fn unfence_named_fields_recursive_is_a_no_op_for_a_clean_value_never_fenced() {
    let mut input = json!({ "title": "Staff Engineer", "company": "Acme Corp" });
    unfence_named_fields_recursive(&mut input);
    assert_eq!(input["title"].as_str().unwrap(), "Staff Engineer");
    assert_eq!(input["company"].as_str().unwrap(), "Acme Corp");
}

/// Reaches a wrapper nested under a wrapper key AND inside an array element
/// under a listed field — the same depth/array coverage
/// `fence_named_fields_recursive` gets, mirrored on the reverse direction.
#[test]
fn unfence_named_fields_recursive_reaches_nested_objects_and_array_elements() {
    let mut input = json!({
        "job": { "description": "<job_posting>\nWe need a backend engineer.\n</job_posting>" },
        "requirements": ["<job_posting>\nRust\n</job_posting>", "SQL"],
    });
    unfence_named_fields_recursive(&mut input);
    assert_eq!(
        input["job"]["description"].as_str().unwrap(),
        "We need a backend engineer."
    );
    assert_eq!(input["requirements"][0].as_str().unwrap(), "Rust");
    assert_eq!(input["requirements"][1].as_str().unwrap(), "SQL");
}

/// #1162 regression: the round-3 AC-7 fix taught the OUTBOUND walk to fence a
/// notification's `title`/`body` under the distinct `app_notification` tag, but
/// left the inbound mirror stripping only `job_posting` for those same two field
/// names — a caller echoing a notification title straight back into a write
/// persisted the literal `<app_notification>…</app_notification>` markup. Round-trips
/// a notification-shaped row through `fence_scraped_fields` then
/// `unfence_named_fields_recursive` and asserts the echo comes back bare.
#[test]
fn unfence_named_fields_recursive_strips_an_app_notification_wrapper_a_caller_echoed_back() {
    let mut data = json!({
        "id": "n-1",
        "kind": "application.follow_up",
        "title": "Staff Engineer follow-up",
        "body": "Your application to Acme Corp is due.",
        "createdAt": 0,
        "read": false,
    });
    fence_scraped_fields(&mut data);
    let title = data["title"].as_str().unwrap();
    let body = data["body"].as_str().unwrap();
    assert!(
        title.starts_with("<app_notification>\n"),
        "precondition: a notification's title must be fenced under app_notification: {title}"
    );

    // A caller echoes the fenced title/body straight back into a write's `--input`.
    let mut echoed = json!({ "title": title, "body": body });
    unfence_named_fields_recursive(&mut echoed);
    assert_eq!(
        echoed["title"].as_str().unwrap(),
        "Staff Engineer follow-up",
        "an app_notification wrapper must be stripped, not persisted verbatim"
    );
    assert_eq!(
        echoed["body"].as_str().unwrap(),
        "Your application to Acme Corp is due."
    );
}

// ── Frame-cap refusal (issue #1135) ──────────────────────────────────────
