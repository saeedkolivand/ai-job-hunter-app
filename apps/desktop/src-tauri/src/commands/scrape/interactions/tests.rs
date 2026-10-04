use super::*;

// ── unfence_job_field (security review round 4) ──────────────────────
// Pure — no AppHandle needed, unlike `scrape_persist_job` itself (this
// crate has no `tauri::test` mock-app harness).

#[test]
fn unfence_job_field_strips_a_wrapper_and_otherwise_leaves_the_value_alone() {
    let fenced = crate::prompt_fence::fenced("job_posting", "Senior Engineer", 1_000);
    for (input, expected, why) in [
        // A wrapper a caller echoed back from a fenced read.
        (
            Some(fenced),
            "Senior Engineer",
            "a value round-tripped from a fenced read must not persist the wrapper",
        ),
        // A clean caller-supplied value.
        (
            Some("Senior Engineer".to_string()),
            "Senior Engineer",
            "a clean value is left alone",
        ),
        // A missing value.
        (None, "", "a missing value defaults to the empty string"),
    ] {
        assert_eq!(unfence_job_field(input), expected, "{why}");
    }
}

// The request must deserialize from the camelCase wire shape the renderer
// sends (`jobId`/`interactionType`).
#[test]
fn remove_interaction_request_deserializes_camel_case() {
    let json = r#"{"jobId":"https://example.com/job/1","interactionType":"dismissed"}"#;
    let req: ScrapeRemoveInteractionRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.job_id, "https://example.com/job/1");
    assert_eq!(req.interaction_type, "dismissed");
}
