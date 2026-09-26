//! Tests for `extract`'s `Scalar`/`Lookup` arms (`proof.rs`).

use super::super::super::super::agent_cli::policy::Effect;
use super::super::*;
use serde_json::json;

// ── extract ──────────────────────────────────────────────────────────

#[test]
fn extract_scalar_walks_a_nested_path() {
    let source = ProofSource::Scalar {
        read_command: "ai_spend_summary",
        path: &["today", "inputTokens"],
    };
    let response = json!({ "today": { "inputTokens": 4200 } });
    assert_eq!(
        extract(source, &json!({}), &response),
        Some("4200".to_string())
    );
}

#[test]
fn extract_scalar_with_empty_path_uses_the_bare_response() {
    let source = ProofSource::Scalar {
        read_command: "system_get_version",
        path: &[],
    };
    let response = json!("0.144.0");
    assert_eq!(
        extract(source, &json!({}), &response),
        Some("0.144.0".to_string())
    );
}

/// A real `ActiveAiConfig` fixture (security review round 3, this
/// table's own "only 3 of 31 rows have a real-fixture test" follow-up):
/// `ai_active_config` serializes `active_provider` as `activeProvider` —
/// a hand-typed `json!({"activeProvider": ...})` literal would not catch
/// either field being renamed.
#[test]
fn extract_scalar_resolves_active_provider_from_a_real_active_ai_config_fixture() {
    let source = ProofSource::Scalar {
        read_command: "ai_active_config",
        path: &["activeProvider"],
    };
    let response = serde_json::to_value(crate::ai_config::ActiveAiConfig {
        active_provider: Some("openai-compatible".to_string()),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        extract(source, &json!({}), &response),
        Some("openai-compatible".to_string())
    );
}

#[test]
fn extract_scalar_returns_none_when_no_provider_is_active_yet() {
    // Unseeded install: `active_provider` is `None`, and
    // `skip_serializing_if` drops the key entirely — must resolve to no
    // proof, never a fabricated "null" string a caller could type.
    let source = ProofSource::Scalar {
        read_command: "ai_active_config",
        path: &["activeProvider"],
    };
    let response = serde_json::to_value(crate::ai_config::ActiveAiConfig::default()).unwrap();
    assert_eq!(extract(source, &json!({}), &response), None);
}

/// T5 hardening (round-3 review): the policy test pins `updater_install`'s
/// `ProofSource::Scalar { path: &["version"], .. }` as a LITERAL, and
/// `updater::test` pins `status_reply`'s shape as a SEPARATE literal —
/// nothing ever fed a real `status_reply` output through `extract` using
/// the ACTUAL `updater::updater_install` POLICY row, so renaming
/// `status_reply`'s `version` key would leave both tests green while
/// making this confirm ceremony permanently unsatisfiable. This pulls the
/// real row out of `POLICY` (never a re-typed path) and feeds it a real
/// `UpdaterState`/`status_reply` fixture (never a hand-built response).
#[test]
fn extract_scalar_reads_updater_installs_real_pending_version_off_status_reply() {
    let entry = POLICY
        .iter()
        .find(|e| e.path == "updater::updater_install")
        .expect("updater::updater_install is a real POLICY row");
    let Effect::Irreversible(source) = entry.effect else {
        panic!(
            "updater_install must be Irreversible, got {:?}",
            entry.effect
        );
    };

    let state = crate::updater::UpdaterState {
        pending_version: Some("2.5.0".to_string()),
        ..crate::updater::UpdaterState::default()
    };
    let response = crate::updater::status_reply(&state, None);

    assert_eq!(
        extract(source, &json!({}), &response),
        Some("2.5.0".to_string())
    );
}

#[test]
fn extract_lookup_walks_a_nested_field() {
    let source = ProofSource::Lookup {
        read_command: "applications_get",
        key: "id",
        input: LookupInput::FromCaller(&["id"]),
        path: &["application", "title"],
    };
    let response = json!({ "application": { "title": "Staff Engineer" }, "events": [] });
    assert_eq!(
        extract(source, &json!({ "id": "app-1" }), &response),
        Some("Staff Engineer".to_string())
    );
}

#[test]
fn extract_lookup_returns_none_for_a_null_response() {
    // `autopilot_get` returns `json!(None::<Autopilot>)` (bare `null`)
    // when the id doesn't exist — must not stringify as `"null"`.
    let source = ProofSource::Lookup {
        read_command: "autopilot_get",
        key: "autopilotId",
        input: LookupInput::FromCaller(&["autopilotId"]),
        path: &["name"],
    };
    assert_eq!(
        extract(source, &json!({ "autopilotId": "gone" }), &Value::Null),
        None
    );
}
