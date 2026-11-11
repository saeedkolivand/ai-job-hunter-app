mod fixture_mapping;
mod response_parsing;

#[test]
fn tag_param_respects_jobicy_3_to_20_char_limit() {
    use super::tag_param;
    // Live 400s: 21+ chars, <3 chars.
    assert_eq!(
        tag_param("senior backend engineer").as_deref(),
        Some("senior backend")
    );
    assert_eq!(
        tag_param("devops engineer remote").as_deref(),
        Some("devops engineer")
    );
    assert_eq!(tag_param("i"), None);
    assert_eq!(tag_param("  ab  "), None);
    assert_eq!(tag_param(""), None);
    // Live 200s are kept verbatim.
    assert_eq!(
        tag_param("Frontend Developer").as_deref(),
        Some("Frontend Developer")
    );
    assert_eq!(tag_param("c++").as_deref(), Some("c++"));
    // Single over-long word is hard-cut; length counts chars, not bytes.
    assert_eq!(tag_param(&"x".repeat(30)).unwrap().chars().count(), 20);
    assert_eq!(tag_param(&"ü".repeat(11)).unwrap().chars().count(), 11);
}

#[test]
fn truncated_tag_results_are_narrowed_to_the_full_query() {
    use super::{map_job, matches_all_tokens, Job};
    let job = |title: &str| {
        let j: Job = serde_json::from_value(serde_json::json!({
            "id": 1, "url": "https://jobicy.com/jobs/1", "jobTitle": title
        }))
        .unwrap();
        map_job(j, "jobicy", 0).unwrap()
    };
    assert!(matches_all_tokens(
        &job("Senior Backend Engineer"),
        "senior backend engineer"
    ));
    assert!(!matches_all_tokens(
        &job("Regional HR Manager"),
        "senior backend engineer"
    ));
}

#[test]
fn token_match_ignores_markdown_escapes_in_the_description() {
    use super::{map_job, matches_all_tokens, Job};
    // Real htmd output for this HTML is "snake\_case" (verified).
    let j: Job = serde_json::from_value(serde_json::json!({
        "id": 1, "url": "https://jobicy.com/jobs/1", "jobTitle": "Engineer",
        "jobDescription": "<p>We use snake_case and Node.js.</p>"
    }))
    .unwrap();
    let p = map_job(j, "jobicy", 0).unwrap();
    assert!(p.description.as_deref().unwrap().contains("snake\\_case"));
    assert!(matches_all_tokens(&p, "snake_case node.js"));
}
