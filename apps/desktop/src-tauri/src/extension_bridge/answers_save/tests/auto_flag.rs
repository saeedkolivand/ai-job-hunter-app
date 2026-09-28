use super::super::*;

// The `refused`/`is_auto_*` gate pair is table-driven ACROSS write verbs in the crate-level
// `auto_flag_gate_tests` (shared with `status_update`'s equivalent pair) — this file keeps only
// the malformed-flag guard below, which has no `status.update` counterpart.

/// A present-but-non-boolean `auto` (a string, a number, `null`) must be flagged malformed — a
/// silent downgrade to "manual" via `is_auto_answers_save`'s `unwrap_or(false)` would let a
/// malformed automated capture through on the (weaker) autofill opt-in alone, bypassing the
/// dedicated `saveAnswersOnSubmit` consent class this verb's AUTO path requires. A well-formed
/// `auto: true`/`auto: false`, and an absent `auto`, are all byte-identical to today (unaffected).
#[test]
fn auto_flag_is_malformed_only_when_auto_is_present_and_not_a_boolean() {
    assert!(auto_flag_is_malformed(
        &serde_json::json!({ "auto": "true" })
    ));
    assert!(auto_flag_is_malformed(&serde_json::json!({ "auto": 1 })));
    assert!(auto_flag_is_malformed(&serde_json::json!({ "auto": null })));
    assert!(auto_flag_is_malformed(
        &serde_json::json!({ "auto": ["true"] })
    ));

    assert!(!auto_flag_is_malformed(
        &serde_json::json!({ "auto": true })
    ));
    assert!(!auto_flag_is_malformed(
        &serde_json::json!({ "auto": false })
    ));
    assert!(
        !auto_flag_is_malformed(&serde_json::json!({ "url": "x" })),
        "absent `auto` is unaffected — byte-identical to today"
    );
}
