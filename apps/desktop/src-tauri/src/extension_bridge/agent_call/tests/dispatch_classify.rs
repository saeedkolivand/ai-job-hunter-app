//! `InvokeResponse` classification, error-detail rendering and the pure `gate` decision (`dispatch.rs`, `dispatch_plan.rs`).

use tauri::ipc::{InvokeError, InvokeResponse, InvokeResponseBody};

use super::super::super::agent_cli::policy::Effect;
use super::super::dispatch::{classify_response, invoke_error_detail};
use super::super::*;

#[test]
fn classify_response_maps_ok_json_to_success() {
    let response = InvokeResponse::Ok(InvokeResponseBody::Json(
        json!({ "success": true }).to_string(),
    ));
    match classify_response(response) {
        InvokeOutcome::Success(v) => assert_eq!(v, json!({ "success": true })),
        InvokeOutcome::CommandErr(_) => panic!("InvokeResponse::Ok must map to Success"),
    }
}

#[test]
fn classify_response_maps_ok_raw_bytes_to_success() {
    let response = InvokeResponse::Ok(InvokeResponseBody::Raw(vec![1, 2, 3]));
    match classify_response(response) {
        InvokeOutcome::Success(v) => assert_eq!(v, json!([1, 2, 3])),
        InvokeOutcome::CommandErr(_) => panic!("InvokeResponse::Ok(Raw) must map to Success"),
    }
}

/// The core Finding-1 regression pin: `InvokeResponse::Err` — whether a
/// legitimate command-body `Err` (e.g. `documents_export_document` failing
/// validation) or a Tauri-level rejection (`applications_delete` called
/// without `keepDocuments`) — must NEVER classify as `Success`. Deleting
/// this arm (folding `Err` back into `Success`, the exact original bug)
/// makes this fail while the two tests above keep passing.
#[test]
fn classify_response_maps_err_to_command_err_never_success() {
    let response = InvokeResponse::Err(InvokeError(json!("missing required key keepDocuments")));
    match classify_response(response) {
        InvokeOutcome::CommandErr(v) => {
            assert_eq!(v, json!("missing required key keepDocuments"));
        }
        InvokeOutcome::Success(_) => panic!(
            "InvokeResponse::Err must never classify as Success — this is the exact bug where \
             a failed call reported dispatched:true"
        ),
    }
}

#[test]
fn invoke_error_detail_unquotes_a_bare_string_value() {
    assert_eq!(
        invoke_error_detail(&json!("run not found: run-x")),
        "run not found: run-x"
    );
}

#[test]
fn invoke_error_detail_falls_back_to_json_form_for_a_non_string_value() {
    assert_eq!(invoke_error_detail(&json!({ "code": 42 })), "{\"code\":42}");
}

// ── gate (the gate `dispatch` actually calls) ───────────────────────────
// The exhaustive walk over every real POLICY row lives in
// `extension_bridge::test` (needs `POLICY`, not just a hand-picked sample);
// this covers the 4 variants directly, once each, as the fast/local check.

#[test]
fn gate_dispatches_direct_for_read_and_reversible_regardless_of_confirm() {
    assert!(matches!(
        super::super::gate(Effect::Read, None),
        Ok(Dispatch::Direct)
    ));
    assert!(matches!(
        super::super::gate(Effect::Read, Some("x")),
        Ok(Dispatch::Direct)
    ));
    assert!(matches!(
        super::super::gate(Effect::Reversible, None),
        Ok(Dispatch::Direct)
    ));
    assert!(matches!(
        super::super::gate(Effect::Reversible, Some("x")),
        Ok(Dispatch::Direct)
    ));
}

#[test]
fn gate_refuses_not_exposed_regardless_of_confirm() {
    assert!(matches!(
        super::super::gate(Effect::NotExposed("x"), None),
        Err(Refusal::NotExposed("x"))
    ));
    assert!(matches!(
        super::super::gate(Effect::NotExposed("x"), Some("y")),
        Err(Refusal::NotExposed("x"))
    ));
}

/// Mutation guard for Finding 1 (security review, PR #1087): `gate`'s
/// `Confirmed` branch must carry the ROW'S OWN `source` and the CALLER'S OWN
/// `confirm` value, by construction — never a value `dispatch` has to
/// re-derive or unwrap afterward. Reverting `gate` to the old
/// boolean-returning shape (and re-adding a `confirm.expect(...)` downstream)
/// would still pass every OTHER test here; only checking the carried fields
/// directly, on the exact `ProofSource` `gate` was called with, catches it.
#[test]
fn gate_for_irreversible_refuses_with_no_confirm_and_carries_source_and_confirm_once_present() {
    let source = super::super::super::agent_cli::policy::ProofSource::Count {
        read_command: "notifications_list",
    };
    let irreversible = Effect::Irreversible(source);

    assert!(matches!(
        super::super::gate(irreversible, None),
        Err(Refusal::ConfirmationRequired(_))
    ));

    let Ok(Dispatch::Confirmed {
        source: got_source,
        confirm,
    }) = super::super::gate(irreversible, Some("3"))
    else {
        panic!("expected Ok(Dispatch::Confirmed {{ .. }}) once a confirm was supplied");
    };
    assert_eq!(got_source, source);
    assert_eq!(confirm, "3");
}

// ── call_result_reply shape ───────────────────────────────────────────
