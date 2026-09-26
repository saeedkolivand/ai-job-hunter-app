//! The #1160/#1160-r2 ordering fix, driven through the REAL `plan`/`gate` decision rather than
//! `check_input` in isolation, plus the `T | undefined`-is-optional catalogue coverage
//! (`validate.rs`).

use serde_json::json;

use super::super::super::super::agent_cli::policy::{Effect, POLICY};
use super::super::*;

// ── #1160/#1160-r2 ordering: driven through the real `plan`, not proved by inference ────────────

/// The #1160 ordering fix, proved by calling the REAL decision fn `dispatch` calls (`plan`), not
/// by calling `check_input`/`gate` separately and inferring what their order must be (CLI review
/// round 2 — MEDIUM: that inference-based version stayed green even if `dispatch`'s own two lines
/// were swapped, since it never drove `dispatch`'s actual code path). For the SAME real
/// `Irreversible` row and the SAME missing-required-key input, `gate` alone (unaware of the
/// catalogue) would ask for a `--confirm` proof — `plan` must refuse on the missing key FIRST, so
/// a caller is never asked to prove a delete it could never have completed anyway (a missing
/// `keepDocuments` used to surface as `invoke_error` AFTER an approved confirm; see this row's own
/// entry in `catalogue.rs`). A mutation swapping `plan`'s two checks fails this test.
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

    // The REAL decision `dispatch` makes — not `check_input`/`gate` called separately.
    let decision =
        super::super::super::plan(entry, "applications_delete", &missing_keep_documents, None);
    match decision {
        Err(Refusal::InvalidInput(detail)) => assert!(detail.contains("keepDocuments")),
        Ok(_) => panic!("expected InvalidInput naming keepDocuments, got Ok"),
        Err(other) => panic!(
            "expected InvalidInput naming keepDocuments, got {:?}",
            other.detail()
        ),
    }
}

/// #1160 round-2 (MEDIUM): a `NotExposed` row must refuse with its OWN cause even when the
/// caller's `input` also fails catalogue validation — the lesser cause (bad arguments) must never
/// mask the definitive one (this command can never be dispatched at all). `ai_test_provider_key`
/// is a real `NotExposed` row that carries declared args (`provider`, `baseUrl`), so an unknown
/// key on it is exactly the case CLI review round 2 found reaching `InvalidInput` instead.
#[test]
fn a_not_exposed_row_refuses_with_its_own_cause_even_with_invalid_input() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "commands::ai::ai_test_provider_key")
        .expect("ai_test_provider_key is a real POLICY row");
    assert!(
        matches!(entry.effect, Effect::NotExposed(_)),
        "fixture assumption: ai_test_provider_key must still be NotExposed"
    );

    let bogus_input = json!({ "bogus": 1 });
    let decision = super::super::super::plan(entry, "ai_test_provider_key", &bogus_input, None);
    match decision {
        Err(Refusal::NotExposed(_)) => {}
        Ok(_) => panic!("expected NotExposed even though `input` also fails validation, got Ok"),
        Err(other) => panic!(
            "expected NotExposed even though `input` also fails validation, got {:?}",
            other.detail()
        ),
    }
}

/// A1-r1-SEC-2 MEDIUM, driven through the REAL `plan` `dispatch` calls (not the isolated fn):
/// `applications_save_from_posting`'s `{"req":{}}` shape passes `check_input` (every top-level key
/// is present, nothing unknown) but must still refuse — an empty wrapper on a Reversible row is
/// never a dispatchable no-op.
#[test]
fn an_empty_required_wrapper_on_a_real_reversible_row_is_refused_by_plan() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "commands::applications::applications_save_from_posting")
        .expect("applications_save_from_posting is a real POLICY row");
    assert!(
        matches!(entry.effect, Effect::Reversible),
        "fixture assumption: applications_save_from_posting must still be Reversible"
    );

    let empty_req = json!({ "req": {} });
    let decision =
        super::super::super::plan(entry, "applications_save_from_posting", &empty_req, None);
    match decision {
        Err(Refusal::InvalidInput(detail)) => assert!(detail.contains("req")),
        Ok(_) => panic!("expected InvalidInput for an empty required wrapper, got Ok"),
        Err(other) => panic!(
            "expected InvalidInput for an empty required wrapper, got {:?}",
            other.detail()
        ),
    }
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

// ── `T | undefined` is optional too (MEDIUM — CLI review round 1) ──────────────────────────────

/// `setSalaryExpectation: (salaryExpectation: string | undefined) => ...` — optional via a union
/// with `undefined`, not `?`/a default. `JSON.stringify` drops an `undefined`-valued property, so
/// the renderer's own "clear this value" call site sends `{}`; before the generator's
/// `isOptionalUnion` fix this arg was catalogued `required: true`, refusing the app's OWN
/// clear-path with `missing required key`.
#[test]
fn a_union_with_undefined_param_is_catalogued_as_optional() {
    let entry = CATALOGUE
        .iter()
        .find(|e| e.command == "job_preferences_set_salary_expectation")
        .expect("real catalogue row");
    let arg = entry
        .args
        .iter()
        .find(|a| a.name == "salaryExpectation")
        .expect("declares a salaryExpectation arg");
    assert!(
        !arg.required,
        "salaryExpectation: string | undefined must be optional — the renderer's own clear-value \
         call site omits it entirely"
    );

    assert!(
        check_input("job_preferences_set_salary_expectation", &json!({})).is_ok(),
        "the renderer's own clear-value payload ({{}}) must not be refused as missing a required key"
    );
}
