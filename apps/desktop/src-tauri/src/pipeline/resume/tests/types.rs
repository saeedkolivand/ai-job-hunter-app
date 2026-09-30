use super::super::types::ResumeStrategy;

/// A previously-persisted `ResumeStrategy` (from before `sectionOrder` was
/// removed from the struct) must still deserialize — the field is IGNORED,
/// not an error, because the struct has no `deny_unknown_fields` and every
/// field carries `#[serde(default)]`.
#[test]
fn resume_strategy_deserializes_a_legacy_blob_with_the_removed_section_order_key() {
    let legacy = r#"{
        "headlineAngle": "Payments-platform engineer",
        "summaryFocus": ["distributed systems"],
        "sectionOrder": ["summary", "skills", "experience", "projects", "education"],
        "perCompany": [],
        "skillsGroups": []
    }"#;
    let strategy: ResumeStrategy =
        serde_json::from_str(legacy).expect("legacy sectionOrder key must be ignored, not error");
    assert_eq!(strategy.headline_angle, "Payments-platform engineer");
    assert_eq!(strategy.summary_focus, vec!["distributed systems"]);
}
