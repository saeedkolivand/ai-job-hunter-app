use super::*;

#[test]
fn test_parse_inline_md() {
    let segments = parse_inline_md("Use **React** and **TypeScript** here");
    assert_eq!(segments.len(), 5);
    assert!(!segments[0].bold);
    assert!(segments[1].bold);
    assert_eq!(segments[1].text, "React");
}

#[test]
fn test_company_name_not_section() {
    let line = parse_line("NASA ENGINEER", 1, &["Name", "NASA ENGINEER"]);
    assert!(!matches!(line.kind, LineKind::SectionHeader));
}

#[test]
fn test_numbered_bullet() {
    let line = parse_line("1. First bullet point", 5, &[]);
    assert!(matches!(line.kind, LineKind::Bullet));
}

#[test]
fn test_strip_md() {
    assert_eq!(strip_md("**bold**"), "bold");
    assert_eq!(strip_md("# Heading"), "Heading");
    assert_eq!(strip_md("#Heading#"), "Heading");
    assert_eq!(strip_md("## Section"), "Section");
    assert_eq!(strip_md("normal text"), "normal text");
}

#[test]
fn test_section_header_detection() {
    let line = parse_line("work experience", 5, &[]);
    assert!(matches!(line.kind, LineKind::SectionHeader));
}

#[test]
fn test_all_caps_section() {
    let line = parse_line("EXPERIENCE", 5, &[]);
    assert!(matches!(line.kind, LineKind::SectionHeader));
}

#[test]
fn test_bullet_detection() {
    let line = parse_line("• First point", 5, &[]);
    assert!(matches!(line.kind, LineKind::Bullet));
}

#[test]
fn test_job_entry_detection() {
    let line = parse_line("Software Engineer  Jan 2020 - Present", 5, &[]);
    assert!(matches!(line.kind, LineKind::JobEntry));
}

#[test]
fn test_contact_detection() {
    let line = parse_line("john@example.com", 5, &[]);
    assert!(matches!(line.kind, LineKind::Contact));
}

#[test]
fn test_job_title_detection() {
    let lines = vec!["Software Engineer  Jan 2020 - Present", "Senior Developer"];
    let line = parse_line("Senior Developer", 1, &lines);
    assert!(matches!(line.kind, LineKind::JobTitle));
}

#[test]
fn test_multilingual_sections() {
    let line = parse_line("berufserfahrung", 5, &[]);
    assert!(matches!(line.kind, LineKind::SectionHeader));
}

#[test]
fn test_parse_resume() {
    let text = "John Doe\njohn@example.com\n\nExperience\nSoftware Engineer  Jan 2020 - Present";
    let doc = parse_resume(text);
    assert!(doc.has_name);
    assert!(doc.has_contact);
    assert!(doc.section_count > 0);
}

#[test]
fn sanitize_markdown_strips_stray_emphasis_but_keeps_bold() {
    // The observed symptom: lone asterisks leaked by the model.
    assert_eq!(sanitize_markdown("*React and AWS*"), "React and AWS");
    assert_eq!(sanitize_markdown("AWS*-Services"), "AWS-Services");
    // Valid bold survives (the renderer turns it into real bold).
    assert_eq!(sanitize_markdown("Use **React** today"), "Use **React** today");
    // Stray backticks go; in-word underscores (snake_case) are left untouched.
    assert_eq!(sanitize_markdown("a `code` span"), "a code span");
    assert_eq!(sanitize_markdown("create_react_app"), "create_react_app");
}

#[test]
fn typography_fixes_sentence_break_dashes_only() {
    // En-dash glued to the previous word (cover-letter symptom) → spaced en-dash.
    assert_eq!(typography("zu Hause\u{2013} die"), "zu Hause \u{2013} die");
    // ASCII hyphen used as a sentence break → spaced en-dash.
    assert_eq!(typography("zu Hause- die"), "zu Hause \u{2013} die");
    // German suspended hyphen is preserved (NOT turned into a dash).
    assert_eq!(typography("Backend- und Frontend"), "Backend- und Frontend");
    // A tight numeric range and a real compound are left alone.
    assert_eq!(typography("2020\u{2013}2023"), "2020\u{2013}2023");
    assert_eq!(typography("state-of-the-art"), "state-of-the-art");
}
