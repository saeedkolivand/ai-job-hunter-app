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
    assert_eq!(
        sanitize_markdown("Use **React** today"),
        "Use **React** today"
    );
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

// ── New job-entry detection branches ─────────────────────────────────────────

/// Comma + parenthesized date: the AI's documented output format.
/// "Senior Engineer, Acme Corp (January 2021 – March 2023)" → JobEntry
/// with text = the full line (role + company + period all bold).
#[test]
fn job_entry_paren_date_full_line() {
    let line = parse_line(
        "Senior Engineer, Acme Corp (January 2021 \u{2013} March 2023)",
        5,
        &[],
    );
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry, got {:?}",
        line.kind
    );
    assert!(
        line.text
            .contains("Senior Engineer, Acme Corp (January 2021"),
        "text should contain the full header; got: {:?}",
        line.text
    );
    assert!(
        line.right_text.is_none(),
        "right_text must be None for paren-date format; got: {:?}",
        line.right_text
    );
}

/// Pipe-separated with a year-only range.
/// "Senior Platform Engineer | Globex Corp | 2020 – Present" → JobEntry
#[test]
fn job_entry_pipe_date_segment_year_range() {
    let line = parse_line(
        "Senior Platform Engineer | Globex Corp | 2020 \u{2013} Present",
        5,
        &[],
    );
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry, got {:?}",
        line.kind
    );
    assert!(
        line.text.contains("Senior Platform Engineer"),
        "text should contain the full header; got: {:?}",
        line.text
    );
    assert!(
        line.right_text.is_none(),
        "right_text must be None for pipe-date format; got: {:?}",
        line.right_text
    );
}

/// Pipe-separated with month-year range.
/// "Software Engineer | Beta Inc | Jan 2021 – Mar 2023" → JobEntry
#[test]
fn job_entry_pipe_date_segment_month_year_range() {
    let line = parse_line(
        "Software Engineer | Beta Inc | Jan 2021 \u{2013} Mar 2023",
        5,
        &[],
    );
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry, got {:?}",
        line.kind
    );
}

/// "Distributed Rate Limiter | Open Source | 2021" → JobEntry.
/// Projects (and single-year education) use a bare year, not a range — they must
/// still render as bold entries like Experience, not plain paragraphs.
#[test]
fn job_entry_pipe_single_year() {
    let line = parse_line("Distributed Rate Limiter | Open Source | 2021", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry for a single-year project header, got {:?}",
        line.kind
    );
}

/// A contact line carrying a phone + a bare year but NO email must stay Contact —
/// the `@`-only guard was insufficient (real contacts have a phone, not an email).
#[test]
fn contact_line_phone_and_year_stays_contact() {
    let line = parse_line("Berlin, Germany | +49 30 1234567 | 2021", 5, &[]);
    assert!(
        !matches!(line.kind, LineKind::JobEntry),
        "phone+year contact must NOT be JobEntry, got {:?}",
        line.kind
    );
}

/// Single-separator skill / certification lines with a bare year are ambiguous and
/// must NOT be promoted to entries (a single year only counts with ≥2 separators).
#[test]
fn single_separator_year_is_not_job_entry() {
    for s in ["React • 2021", "AWS Certified • 2023"] {
        let line = parse_line(s, 5, &[]);
        assert!(
            !matches!(line.kind, LineKind::JobEntry),
            "{s:?} must NOT be JobEntry, got {:?}",
            line.kind
        );
    }
}

/// Contact line with email MUST still be Contact even if it has pipes.
/// "Haarlem, NL | jane@example.com | +31 6 1234 5678 | LinkedIn" → Contact
#[test]
fn contact_line_with_email_stays_contact() {
    let line = parse_line(
        "Haarlem, NL | jane@example.com | +31 6 1234 5678 | LinkedIn",
        5,
        &[],
    );
    assert!(
        matches!(line.kind, LineKind::Contact),
        "expected Contact (has '@'), got {:?}",
        line.kind
    );
}

/// Contact line with only pipes and no date MUST still be Contact.
/// "New York | LinkedIn | github.com/jane" → Contact (URL_RE matches)
#[test]
fn contact_line_pipes_no_date_stays_contact() {
    let line = parse_line("New York | linkedin.com/in/jane | github.com/jane", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Contact),
        "expected Contact (URL match), got {:?}",
        line.kind
    );
}

/// Legacy 2-space format still works.
/// "Acme Corp  2020 - Present" → JobEntry (existing behavior preserved)
#[test]
fn job_entry_legacy_two_space_format_preserved() {
    let line = parse_line("Acme Corp  2020 - Present", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry (legacy 2-space), got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Acme Corp");
    assert_eq!(line.right_text.as_deref(), Some("2020 - Present"));
}

/// A normal skills line is not a job entry.
/// "Rust, TypeScript, React, AWS, Docker" → Text
#[test]
fn skills_line_stays_text() {
    let line = parse_line("Rust, TypeScript, React, AWS, Docker", 5, &[]);
    assert!(
        !matches!(line.kind, LineKind::JobEntry),
        "skills line must not be JobEntry, got {:?}",
        line.kind
    );
}

/// A known section header is still detected as SectionHeader, not JobEntry.
#[test]
fn section_header_not_job_entry() {
    let line = parse_line("EXPERIENCE", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
}
