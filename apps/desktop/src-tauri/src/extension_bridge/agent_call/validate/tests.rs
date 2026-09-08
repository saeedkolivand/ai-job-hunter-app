//! Pulls real rows out of the GENERATED [`CATALOGUE`] rather than a hand-typed fixture — the same
//! "close the gap between the general logic and the real table" discipline `agent_call::proof::
//! tests` already uses for `POLICY`. A hand-typed `CatalogueEntry` would only prove `check_input`'s
//! OWN logic is correct, never that it agrees with what the generator actually emitted for the
//! exact row a real caller will hit.

use serde_json::json;

use super::super::super::agent_cli::policy::{Effect, POLICY};
use super::*;

fn has_command(command: &str) -> bool {
    CATALOGUE.iter().any(|e| e.command == command)
}

#[test]
fn applications_delete_and_applications_save_from_posting_are_real_catalogue_fixtures() {
    // Guards every test below: if the generator ever stops emitting these two rows (or their
    // shape changes), the fixture assumptions the rest of this file leans on need updating, not
    // a mystifying failure three tests down.
    assert!(has_command("applications_delete"));
    assert!(has_command("applications_save_from_posting"));
    assert!(has_command("job_preferences_set"));
    assert!(has_command("applications_list"));
}

// ── unknown top-level key ───────────────────────────────────────────────────────────────────────

#[test]
fn unknown_top_level_key_is_refused() {
    let err = check_input(
        "applications_set_status",
        &json!({ "id": "app-1", "status": "applied", "nonsenseKeyThatDoesNotExist": true }),
    )
    .unwrap_err();
    assert!(matches!(err, Refusal::InvalidInput(_)));
    let detail = err.detail();
    assert!(detail.contains("nonsenseKeyThatDoesNotExist"), "{detail}");
    assert!(detail.contains("applications_set_status"), "{detail}");
}

/// Issue #1158 member 3's exact shape: the caller sends the WRONG top-level key (`url` instead of
/// the real `req` wrapper) — refused as unknown, never silently accepted into an empty record.
#[test]
fn a_wrong_wrapper_guess_is_refused_as_an_unknown_top_level_key() {
    let err = check_input(
        "applications_save_from_posting",
        &json!({ "url": "https://example.com/job/1" }),
    )
    .unwrap_err();
    let detail = err.detail();
    assert!(detail.contains('`') && detail.contains("url"), "{detail}");
    assert!(
        detail.contains("req"),
        "{detail}: must name the real declared key"
    );
}

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
    assert!(
        !req_arg.fields.is_empty(),
        "fixture assumption: req's nested fields must have resolved for this test to mean \
         anything — got an empty field list, which is the OTHER (skip-nested-check) case"
    );
    let bogus_field = "totallyMadeUpField";
    assert!(!req_arg.fields.contains(&bogus_field));

    let err = check_input(
        "applications_save_from_posting",
        &json!({ "req": { "jobUrl": "https://x", bogus_field: 1 } }),
    )
    .unwrap_err();
    let detail = err.detail();
    assert!(detail.contains("req.totallyMadeUpField"), "{detail}");
}

#[test]
fn every_recognised_nested_field_on_a_resolved_wrapper_passes() {
    assert!(check_input(
        "applications_save_from_posting",
        &json!({ "req": { "jobUrl": "https://x", "title": "Engineer" } })
    )
    .is_ok());
}

/// `job_preferences_set`'s `prefs` parameter is typed `unknown` on the tauri-client itself (there
/// is no request struct to resolve field names from) — the generator emits an EMPTY `fields` list
/// for it, and this layer must not invent a nested check it has no data for: any object shape
/// under `prefs` is accepted here (the command's own body still validates it).
#[test]
fn a_wrapper_with_unresolved_nested_fields_skips_the_nested_check_entirely() {
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
        prefs_arg.fields.is_empty(),
        "fixture assumption: prefs is typed `unknown` on the tauri-client, so this generator has \
         no field list to resolve — if that ever changes, this test (and its OWN reasoning) needs \
         updating, not deleting"
    );

    assert!(check_input(
        "job_preferences_set",
        &json!({ "prefs": { "anythingAtAll": true, "location": "Remote" } })
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

// ── #1160 ordering: caught before gate would ask to confirm ────────────────────────────────────

/// The #1160 ordering fix, proved directly rather than only trusted from reading `dispatch`'s own
/// source: for the SAME real `Irreversible` row and the SAME missing-required-key input, `gate`
/// alone (unaware of the catalogue) would ask for a `--confirm` proof — `check_input` must refuse
/// FIRST, so `dispatch` never reaches that ceremony for an input that could never have completed
/// the underlying command anyway (a missing `keepDocuments` used to surface as `invoke_error`
/// AFTER an approved confirm; see this row's own entry in `catalogue.rs`).
#[test]
fn a_missing_required_key_on_a_real_irreversible_row_is_caught_before_gate_would_ask_to_confirm() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "commands::applications::applications_delete")
        .expect("applications_delete is a real POLICY row");
    assert!(
        matches!(entry.effect, Effect::Irreversible(_)),
        "fixture assumption: applications_delete must still be Irreversible"
    );

    let missing_keep_documents = json!({ "id": "app-1" });

    // What `check_input` decides — the SAME call `dispatch` makes first.
    let validation_err = check_input("applications_delete", &missing_keep_documents).unwrap_err();
    assert!(validation_err.detail().contains("keepDocuments"));

    // What `gate` ALONE — unaware of the catalogue — would have decided for this exact call with
    // no `--confirm` supplied: a DIFFERENT refusal (`confirmation_required`). Proves the ordering
    // is observable, not just a code-order comment: skipping `check_input` would ask the caller
    // to prove a delete it could never actually complete.
    let gated = super::super::gate(entry.effect, None);
    assert!(matches!(gated, Err(Refusal::ConfirmationRequired(_))));
}

#[test]
fn a_non_object_input_is_treated_as_carrying_no_top_level_keys() {
    // `applications_delete` requires both `id` and `keepDocuments` — a non-object input must
    // refuse on the MISSING key, never panic on the failed `as_object()`.
    let err = check_input("applications_delete", &json!("not an object")).unwrap_err();
    let detail = err.detail();
    assert!(
        detail.contains("id") || detail.contains("keepDocuments"),
        "{detail}"
    );
}
