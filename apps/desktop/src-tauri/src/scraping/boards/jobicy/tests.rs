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
