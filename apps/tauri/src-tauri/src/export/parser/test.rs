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
