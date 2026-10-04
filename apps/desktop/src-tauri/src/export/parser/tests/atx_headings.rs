//! Markdown ATX heading promotion (§F): `#`…`######` user-authored headings.

use crate::export::parser::parse_line;
use crate::export::types::LineKind;

/// A user-authored custom heading (`## Side Projects`) is promoted to a section
/// heading even though it is neither a known section name nor ALL-CAPS.
#[test]
fn atx_heading_h2_custom_is_section() {
    let line = parse_line("## Side Projects", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Side Projects");
}

/// `### ` (H3) custom heading is likewise promoted, markers stripped.
#[test]
fn atx_heading_h3_custom_is_section() {
    let line = parse_line("### Notable Work", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Notable Work");
}

/// `# Summary` at idx 0 classifies as a heading with the `#` stripped — the ATX
/// rule runs before the idx==0 name/contact block.
#[test]
fn atx_heading_h1_strips_marker_even_at_idx_zero() {
    let line = parse_line("# Summary", 0, &["# Summary"]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Summary");
}

/// Existing known-section behavior is unchanged (no `#` prefix needed).
#[test]
fn known_section_without_hash_still_section() {
    let line = parse_line("Experience", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
}

/// Existing ALL-CAPS behavior is unchanged (no `#` prefix needed).
#[test]
fn all_caps_without_hash_still_section() {
    let line = parse_line("PROFESSIONAL HIGHLIGHTS", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
}

/// A normal prose line (no `#`) is unaffected — still Text.
#[test]
fn prose_line_without_hash_still_text() {
    let line = parse_line("Built and shipped a payments service.", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Text),
        "expected Text, got {:?}",
        line.kind
    );
}

/// A `#hashtag` with no trailing space is NOT an ATX heading — it must fall
/// through to its normal (non-heading) classification.
#[test]
fn hash_without_space_is_not_heading() {
    let line = parse_line("#nospace", 5, &[]);
    assert!(
        !matches!(line.kind, LineKind::SectionHeader),
        "#nospace must NOT be a SectionHeader, got {:?}",
        line.kind
    );
}

/// Bold inside a heading: markers stripped from `text`, but `segments` still
/// tokenize the bold run (`## **Bold Heading**`).
#[test]
fn atx_heading_with_bold_tokenizes_segments() {
    let line = parse_line("## **Bold Heading**", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Bold Heading");
    assert!(
        line.segments
            .iter()
            .any(|s| s.bold && s.text == "Bold Heading"),
        "segments should reflect the bold run; got {:?}",
        line.segments
    );
}

/// Regression: a `---` thematic break is still dropped as Blank — the ATX rule
/// runs AFTER the thematic-break check and never reclassifies it as a heading.
#[test]
fn thematic_break_still_blank_not_heading() {
    let line = parse_line("---", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Blank),
        "expected Blank for thematic break, got {:?}",
        line.kind
    );
}

/// A bare `# ` with empty heading text degrades to Blank (clean is empty) rather
/// than emitting an empty heading.
#[test]
fn empty_atx_heading_falls_through_to_blank() {
    let line = parse_line("# ", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Blank),
        "expected Blank for empty heading, got {:?}",
        line.kind
    );
}

// ── §F bounds: H1–H6 inclusive upper-bound, H7 fall-through, idx-0 Name ────

/// `###### Six` (six hashes) is the inclusive upper bound — must be SectionHeader.
#[test]
fn atx_heading_h6_inclusive_upper_bound() {
    let line = parse_line("###### Six", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "H6 must be SectionHeader (inclusive upper bound), got {:?}",
        line.kind
    );
    assert_eq!(line.text, "Six");
}

/// `####### Seven` (seven hashes) exceeds the ATX limit — must NOT be a heading.
/// It falls through to whatever the normal classification produces (Text / Contact
/// / etc.), but the key requirement is that it is NOT a SectionHeader.
#[test]
fn atx_heading_h7_exceeds_limit_not_heading() {
    let line = parse_line("####### Seven", 5, &[]);
    assert!(
        !matches!(line.kind, LineKind::SectionHeader),
        "7-hash line must NOT be SectionHeader (ATX limit is 6), got {:?}",
        line.kind
    );
}

/// A plain name line at idx 0 with no `#` prefix classifies as `LineKind::Name`,
/// not as a SectionHeader — the ATX rule must not misfire on ordinary prose at
/// the top of the document.
#[test]
fn name_line_at_idx_zero_without_hash_is_name() {
    let line = parse_line("John Doe", 0, &["John Doe"]);
    assert!(
        matches!(line.kind, LineKind::Name),
        "plain line at idx 0 must be Name (no # prefix), got {:?}",
        line.kind
    );
}
