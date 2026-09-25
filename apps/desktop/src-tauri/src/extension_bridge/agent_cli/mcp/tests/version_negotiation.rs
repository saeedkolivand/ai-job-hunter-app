use super::*;
// ── initialize — version negotiation ───────────────────────────────────

#[test]
fn an_unsupported_protocol_version_falls_back_to_the_default() {
    let result = initialize_result(&json!({ "protocolVersion": "2099-01-01" }), INSTRUCTIONS);
    assert_eq!(result["protocolVersion"], DEFAULT_VERSION);
}

#[test]
fn every_supported_older_version_is_echoed_back_verbatim() {
    for v in [
        "2025-11-25",
        "2025-06-18",
        "2025-03-26",
        "2024-11-05",
        "2024-10-07",
    ] {
        let result = initialize_result(&json!({ "protocolVersion": v }), INSTRUCTIONS);
        assert_eq!(result["protocolVersion"], v, "must echo {v}");
    }
}

#[test]
fn a_missing_protocol_version_answers_the_default() {
    let result = initialize_result(&json!({}), INSTRUCTIONS);
    assert_eq!(result["protocolVersion"], DEFAULT_VERSION);
}

#[test]
fn initialize_never_names_2026_07_28() {
    for v in ["2025-11-25", "unknown-future-version"] {
        let result = initialize_result(&json!({ "protocolVersion": v }), INSTRUCTIONS);
        assert_ne!(result["protocolVersion"], "2026-07-28");
    }
}
