use crate::export::parser::parse_line;
use crate::export::types::LineKind;

#[test]
fn test_numbered_bullet() {
    let line = parse_line("1. First bullet point", 5, &[]);
    assert!(matches!(line.kind, LineKind::Bullet));
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
fn test_job_title_detection() {
    let lines = vec!["Software Engineer  Jan 2020 - Present", "Senior Developer"];
    let line = parse_line("Senior Developer", 1, &lines);
    assert!(matches!(line.kind, LineKind::JobTitle));
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

// ── Next-line-date job entry (owner-reported: "Title · Company" \n date) ────

/// The reported shape: title/company on their own line with NO date, the date
/// range (+ trailing location) on the line right after. None of the same-line
/// patterns above recognize this — it used to fall through to a plain,
/// non-bold `Text` paragraph, silently losing the entry structure entirely.
#[test]
fn job_entry_title_then_bare_date_line() {
    // idx 0 is padded with a heading so the title line isn't literally the
    // document's first line (idx==0 has its own Name/Contact special case).
    let lines = [
        "EXPERIENCE",
        "Senior Frontend Developer \u{b7} ACTINEO GmbH",
        "December 2022 \u{2013} November 2025, K\u{f6}ln, Deutschland",
    ];
    let line = parse_line(lines[1], 1, &lines);
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Senior Frontend Developer \u{b7} ACTINEO GmbH");
    assert_eq!(
        line.right_text.as_deref(),
        Some("December 2022 \u{2013} November 2025")
    );
}

/// The paired half: the date line itself strips the matched date and keeps
/// the trailing location as a JobTitle (subtitle).
#[test]
fn job_entry_date_line_remainder_becomes_job_title() {
    let lines = [
        "EXPERIENCE",
        "Senior Frontend Developer \u{b7} ACTINEO GmbH",
        "December 2022 \u{2013} November 2025, K\u{f6}ln, Deutschland",
    ];
    let line = parse_line(lines[2], 2, &lines);
    assert!(
        matches!(line.kind, LineKind::JobTitle),
        "expected JobTitle for the date-line remainder, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "K\u{f6}ln, Deutschland");
}

/// A pure date line with nothing after it (no location/description) is
/// dropped as Blank — the date was already attached to the entry above, so
/// it must not ALSO render as a stray, duplicate paragraph.
#[test]
fn job_entry_date_line_with_no_remainder_is_blank() {
    let lines = ["Independent / Open-Source R&D", "Dec 2025 \u{2013} Present"];
    let line = parse_line(lines[1], 1, &lines);
    assert!(
        matches!(line.kind, LineKind::Blank),
        "expected Blank, got {:?}",
        line.kind
    );
}

/// Regression guard: a REAL section heading directly followed by a
/// leading DATE-RANGE line (the exact shape the new backward branch matches
/// on: "Certifications" \n "2020 – Present, AWS Certified Solutions
/// Architect") must NOT be swallowed as a consumed job-entry date line — the
/// heading never opened an entry, so treating this as "consumed" would
/// silently drop the "2020 – Present" range instead of rendering it.
#[test]
fn heading_then_leading_date_range_line_does_not_misfire() {
    let lines = [
        "Certifications",
        "2020 \u{2013} Present, AWS Certified Solutions Architect",
    ];
    let heading = parse_line(lines[0], 0, &lines);
    assert!(
        matches!(heading.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        heading.kind
    );
    let next = parse_line(lines[1], 1, &lines);
    assert!(
        !matches!(next.kind, LineKind::Blank),
        "must not drop the date-range-bearing line as Blank, got {:?}",
        next.kind
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
