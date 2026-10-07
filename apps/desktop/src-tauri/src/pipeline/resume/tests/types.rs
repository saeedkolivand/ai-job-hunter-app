use super::super::types::{JobAnalysis, ResumeStrategy};

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

/// An artifact cached before `redFlags` and the per-company identity fields
/// left the model-facing shape must still parse (unknown keys are ignored), and
/// the new schema must no longer ask for them.
#[test]
fn old_artifacts_with_the_removed_fields_still_deserialize() {
    let analysis: JobAnalysis = serde_json::from_str(
        r#"{"roleTitle":"Eng","mustHave":["Rust"],"redFlags":["no on-call info"]}"#,
    )
    .expect("old analysis parses");
    assert_eq!(analysis.must_have, ["Rust"]);

    let strategy: ResumeStrategy = serde_json::from_str(
        r#"{"perCompany":[{"company":"Acme","title":"Dev","dates":"2020","angle":"a","emphasis":[],"condensed":true}]}"#,
    )
    .expect("old strategy parses");
    assert!(strategy.per_company[0].condensed);

    assert!(!JobAnalysis::schema().to_string().contains("redFlags"));
    let schema = ResumeStrategy::schema().to_string();
    for gone in ["\"title\"", "\"dates\"", "\"condensed\""] {
        assert!(
            !schema.contains(gone),
            "{gone} still in the strategy schema"
        );
    }
    assert!(!ResumeStrategy::EXAMPLE.contains("condensed"));
}
