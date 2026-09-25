//! Rows whose `Effect` CLASS a past security review deliberately chose — each one a
//! hand-written pin, so a revert to a freely-dispatchable class cannot pass unnoticed.

use super::super::*;

/// Hand-written pin (security review round 3), mirroring
/// `match_resume_and_match_resume_text_stay_not_exposed_until_a_real_
/// charge_lands`'s own discipline: a revert of any of these four rows
/// back to `Read`/`Irreversible` (freely dispatchable, or dispatchable
/// with a proof that no longer applies) would not be caught by any
/// OTHER test in this file. `ai_test_provider_key`/
/// `ai_list_provider_models` send a caller-supplied `base_url` a
/// keychain secret; `resume::extract_resume` reads a fully
/// caller-controlled filesystem path with no validation;
/// `support_export_diagnostics` had only a vacuous compile-time-constant
/// proof. See each row's own comment for the full argument.
#[test]
fn round_3_destination_and_vacuous_proof_rows_stay_not_exposed() {
    for path in [
        "commands::ai::ai_test_provider_key",
        "commands::ai::ai_list_provider_models",
        "commands::resume::extract_resume",
        "commands::support::support_export_diagnostics",
    ] {
        let entry = POLICY
            .iter()
            .find(|e| e.path == path)
            .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
        assert!(
            matches!(entry.effect, Effect::NotExposed(_)),
            "{path} must stay NotExposed — got {:?}",
            entry.effect
        );
    }
}

/// Hand-written pin (security review round 3, narrowed round 4 — see
/// `round_4_persistent_redirect_and_unbound_proof_rows_stay_not_exposed`
/// for the sibling row this test used to also cover): a revert back to
/// `Reversible` would silently restore free routing-flip dispatch with no
/// confirm and no proof.
#[test]
fn ai_set_active_provider_stays_irreversible() {
    let path = "commands::ai::ai_set_active_provider";
    let entry = POLICY
        .iter()
        .find(|e| e.path == path)
        .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
    let Effect::Irreversible(ProofSource::Scalar {
        read_command,
        path: field_path,
    }) = entry.effect
    else {
        panic!(
            "{path} must stay Irreversible with a Scalar proof — got {:?}",
            entry.effect
        );
    };
    assert_eq!(
        read_command, "ai_active_config",
        "{path}'s proof must keep reading ai_active_config"
    );
    assert_eq!(
        field_path,
        ["activeProvider"].as_slice(),
        "{path}'s proof must keep reading the activeProvider field"
    );
}

/// Hand-written pin (security review round 4): a revert of any of these
/// rows would silently restore a live primitive round 4 closed — see each
/// row's own comment. `ai_set_embedding_config` and `ai_seed_active_config`
/// both persist a caller-supplied `base_url` that every subsequent embed/
/// generate call (résumé/job text, the stored provider API key) then reads
/// back and sends to — worse than a one-shot redirect, permanent until the
/// config is changed again (`ai_seed_active_config` was found independently
/// during this round's re-sweep, not named by the original review).
/// `ai_set_provider_settings` takes a caller-CHOSEN `provider` field
/// unrelated to the confirmed `activeProvider`, so its old Scalar proof
/// never bound to the record it actually rewrites (the module doc's
/// clause-2 NotExposed rule).
#[test]
fn round_4_persistent_redirect_and_unbound_proof_rows_stay_not_exposed() {
    for path in [
        "commands::ai::ai_set_embedding_config",
        "commands::ai::ai_seed_active_config",
        "commands::ai::ai_set_provider_settings",
    ] {
        let entry = POLICY
            .iter()
            .find(|e| e.path == path)
            .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
        assert!(
            matches!(entry.effect, Effect::NotExposed(_)),
            "{path} must stay NotExposed — got {:?}",
            entry.effect
        );
    }
}

/// Hand-written pin (MCP security critique): a revert of this row back to
/// `Read` would silently let the generic tier — and every MCP `call-read`
/// client — hand back the bridge's plaintext pairing token verbatim. No
/// OTHER test in this file would catch that: the row-count tests don't
/// change (an `Effect` swap, not an add/remove), and
/// `not_exposed_rows_carry_a_real_reason` only checks rows that ARE already
/// `NotExposed`.
#[test]
fn extension_bridge_status_stays_not_exposed_so_the_pairing_token_never_reaches_a_caller() {
    let path = "commands::extension_bridge::extension_bridge_status";
    let entry = POLICY
        .iter()
        .find(|e| e.path == path)
        .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
    assert!(
        matches!(entry.effect, Effect::NotExposed(_)),
        "{path} must stay NotExposed — got {:?}",
        entry.effect
    );
}

/// Issue #1164 round 2 (`B1-r2-B2-r2-ACLI-2`): `notifications_list` returns EVERY notification,
/// read and unread, while `notifications_mark_all_read` only flips the unread subset — so the
/// `ProofSource::Count` comment above that row must call the count a superset of the blast
/// radius, never claim it is "exact". Pinned against the source text rather than behaviour
/// because the defect was the COMMENT lying about what the count proves, not the `ProofSource`
/// shape itself (round 1 sanctioned keeping `Count` here).
#[test]
fn mark_all_read_proof_comment_calls_the_count_a_superset_not_exact() {
    // The POLICY table plus its per-domain row files (split out of `policy.rs` for R8).
    const POLICY_RS: &str = concat!(
        include_str!("../../policy.rs"),
        include_str!("../rows_core_and_ai_generation.rs"),
        include_str!("../rows_ai_embeddings_and_config.rs"),
        include_str!("../rows_pipeline_resume_and_documents.rs"),
        include_str!("../rows_discovery_and_account.rs"),
        include_str!("../rows_autopilot_and_notifications.rs"),
        include_str!("../rows_bridge_and_email_watch.rs"),
    );
    let row = POLICY_RS
        .find("notifications_mark_all_read")
        .expect("notifications_mark_all_read row present in the policy table sources");
    let comment = &POLICY_RS[..row];
    let comment_start = comment.rfind("// Same no-inverse argument").expect(
        "notifications_mark_all_read's leading comment block present in the policy table sources",
    );
    let comment = &comment[comment_start..];
    assert!(
        comment.contains("superset"),
        "the comment must call the notifications_list count a superset of what actually \
         flips: {comment}"
    );
    assert!(
        !comment.contains("exact count about to be flipped"),
        "the comment must not claim the total count is the exact count about to flip — only \
         the unread subset flips: {comment}"
    );
}
