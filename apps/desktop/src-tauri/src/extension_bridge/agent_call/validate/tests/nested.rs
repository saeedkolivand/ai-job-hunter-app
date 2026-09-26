//! `check_input` coverage for missing required keys, nested wrapper fields, the paging-key
//! exemption, and uncatalogued commands (`validate.rs`).

use serde_json::json;

use super::super::*;
use super::support::has_command;

// ── missing required top-level key ──────────────────────────────────────────────────────────────

/// Issue #1160's exact target row: `applications_delete` declares BOTH `id` and `keepDocuments` as
/// required (the tauri-client's own `remove({ id, keepDocuments })` has no default/optional for
/// either) — omitting the safety flag must refuse by name, not dispatch and die downstream.
#[test]
fn missing_required_key_names_it_and_the_command() {
    let err = check_input("applications_delete", &json!({ "id": "app-1" })).unwrap_err();
    let detail = err.detail();
    assert!(detail.contains("keepDocuments"), "{detail}");
    assert!(detail.contains("applications_delete"), "{detail}");
}

#[test]
fn every_required_key_present_and_no_unknown_key_passes() {
    assert!(check_input(
        "applications_delete",
        &json!({ "id": "app-1", "keepDocuments": false })
    )
    .is_ok());
}

// ── nested wrapper fields ───────────────────────────────────────────────────────────────────────

#[test]
fn unknown_nested_field_on_a_resolved_wrapper_is_refused() {
    let entry = CATALOGUE
        .iter()
        .find(|e| e.command == "applications_save_from_posting")
        .expect("real catalogue row");
    let req_arg = entry
        .args
        .iter()
        .find(|a| a.name == "req")
        .expect("applications_save_from_posting declares a req wrapper");
    let resolved_fields = req_arg.fields.filter(|f| !f.is_empty()).unwrap_or_else(|| {
        panic!(
            "fixture assumption: req's nested fields must have resolved for this test to mean \
             anything — got {:?}, which is the OTHER (skip-nested-check) case",
            req_arg.fields
        )
    });
    let bogus_field = "totallyMadeUpField";
    assert!(!resolved_fields.contains(&bogus_field));

    let err = check_input(
        "applications_save_from_posting",
        &json!({ "req": { "jobUrl": "https://x", bogus_field: 1 } }),
    )
    .unwrap_err();
    let detail = err.detail();
    // The nested key is fenced (HIGH — security review round 1): `req.` prefixes the fence, the
    // key name itself is wrapped `<command_error>\n...\n</command_error>`, never a bare contiguous
    // `req.totallyMadeUpField` substring — see `fenced_key`'s own doc.
    assert!(detail.contains("req.<command_error>"), "{detail}");
    assert!(detail.contains(bogus_field), "{detail}");
}

#[test]
fn every_recognised_nested_field_on_a_resolved_wrapper_passes() {
    assert!(check_input(
        "applications_save_from_posting",
        &json!({ "req": { "jobUrl": "https://x", "title": "Engineer" } })
    )
    .is_ok());
}

/// `job_preferences_set`'s `prefs` parameter is typed `unknown` on the tauri-client itself. Before
/// A1-r1-AC-1 (MEDIUM), an `unknown`-typed identifier param fell all the way through to `fields:
/// None` (a plain SCALAR — the exact "unknown nested shape" published as a scalar with no
/// `fields` key at all, even though `prefs` genuinely IS an object wrapper) — this test used to pin
/// that as a documented fixture assumption. `findParamBinding` now recognises `unknown` (alongside
/// an inline type literal and `Parameters<Fn>[0]`) as a KNOWN, unresolved wrapper, so `fields` is
/// `Some(&[])`, the SAME shape `resume_pipeline_run`'s `req` covers below — this layer must still
/// not invent a nested check it has no field names for: any object shape under `prefs` is accepted
/// here (the command's own body still validates it).
#[test]
fn an_unknown_typed_wrapper_is_recognised_but_skips_the_nested_check() {
    let entry = CATALOGUE
        .iter()
        .find(|e| e.command == "job_preferences_set")
        .expect("real catalogue row");
    let prefs_arg = entry
        .args
        .iter()
        .find(|a| a.name == "prefs")
        .expect("job_preferences_set declares a prefs wrapper");
    assert!(
        prefs_arg.fields.is_some_and(|f| f.is_empty()),
        "fixture assumption: prefs is typed `unknown` on the tauri-client — A1-r1-AC-1 fixed this \
         to a KNOWN, unresolved wrapper (`Some(&[])`), never `None` (a plain scalar) or a \
         resolved field list; if that ever changes, this test (and its OWN reasoning) needs \
         updating, not deleting"
    );

    assert!(check_input(
        "job_preferences_set",
        &json!({ "prefs": { "anythingAtAll": true, "location": "Remote" } })
    )
    .is_ok());
}

/// The OTHER unresolved shape (CLI review round 1, issue #1158): `resume_pipeline_run`'s `req`
/// has a NAMED type this generator recognised (unlike `prefs` above) but could not resolve the
/// field names of — `fields` is `Some(&[])`, not `None`. `check_input` must treat this the same
/// as the `None` case (skip the nested check), and the MCP `commands` tool surfaces the
/// difference to a caller as `"fields": null` rather than omitting the key (`mcp::tests` covers
/// that wire shape).
#[test]
fn a_wrapper_with_a_recognised_but_unresolved_type_also_skips_the_nested_check() {
    let entry = CATALOGUE
        .iter()
        .find(|e| e.command == "resume_pipeline_run")
        .expect("real catalogue row");
    let req_arg = entry
        .args
        .iter()
        .find(|a| a.name == "req")
        .expect("resume_pipeline_run declares a req wrapper");
    assert_eq!(
        req_arg.fields,
        Some(&[][..]),
        "fixture assumption: req's TYPE is recognised but its fields are not resolvable — if \
         that ever changes (either direction), this test needs updating, not deleting"
    );

    assert!(check_input(
        "resume_pipeline_run",
        &json!({ "req": { "anythingAtAll": true } })
    )
    .is_ok());
}

// ── paging-key exemption ────────────────────────────────────────────────────────────────────────

/// `applications_list`/`ai_generations_list` declare ZERO args (the tauri-client calls them with
/// no second `invoke()` argument at all) yet accept `limit`/`cursor` as THIS layer's own paging
/// keys (`take_list_page_args`, stripped AFTER this check runs) — without the exemption, every
/// real paginated call would refuse itself as an unknown key.
#[test]
fn paging_keys_are_exempt_on_a_paginated_command_with_zero_declared_args() {
    let entry = CATALOGUE
        .iter()
        .find(|e| e.command == "applications_list")
        .expect("real catalogue row");
    assert!(
        entry.args.is_empty(),
        "fixture assumption: applications_list declares no args"
    );

    assert!(check_input("applications_list", &json!({ "limit": 5, "cursor": "10" })).is_ok());
}

/// The exemption is command-scoped, not global: a NON-paginated zero-arg command must still
/// refuse `limit`/`cursor` as unknown keys — the exemption naming
/// `reshape::PAGINATED_LIST_COMMANDS` explicitly, never every zero-arg row.
#[test]
fn paging_keys_are_not_exempt_on_a_non_paginated_command() {
    let entry = CATALOGUE
        .iter()
        .find(|e| e.command == "ai_active_config")
        .expect("real catalogue row");
    assert!(
        entry.args.is_empty(),
        "fixture assumption: ai_active_config declares no args"
    );
    assert!(!PAGINATED_LIST_COMMANDS.contains(&"ai_active_config"));

    let err = check_input("ai_active_config", &json!({ "limit": 5 })).unwrap_err();
    let detail = err.detail();
    assert!(detail.contains("limit"), "{detail}");
    // A1-r1-AC-2 MEDIUM: a zero-arg command used to leave a dangling, content-free
    // `declared keys: ` tail — this must say what WOULD have worked instead.
    assert!(
        detail.contains("this command declares no arguments"),
        "{detail}"
    );
    assert!(!detail.trim_end().ends_with("declared keys:"), "{detail}");
}

// ── uncatalogued commands keep today's behaviour ────────────────────────────────────────────────

#[test]
fn an_uncatalogued_command_is_never_validated() {
    assert!(!has_command("dialog_open_files"));
    assert!(check_input(
        "dialog_open_files",
        &json!({ "whateverKeyAtAll": true, "another": 1 })
    )
    .is_ok());
}
