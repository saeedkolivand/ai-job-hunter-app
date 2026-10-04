use crate::export::parser::{parse_line, parse_resume};
use crate::export::types::LineKind;

#[test]
fn test_parse_resume() {
    let text = "John Doe\njohn@example.com\n\nExperience\nSoftware Engineer  Jan 2020 - Present";
    let doc = parse_resume(text);
    assert!(doc.has_name);
    assert!(doc.has_contact);
    assert!(doc.section_count > 0);
}

#[test]
fn thematic_breaks_are_dropped_as_blank() {
    // Each form the model emits as a section separator must be dropped so it
    // never renders as stray "---" text doubling the template's own rule.
    for sep in ["---", "***", "___", "----------", "- - -"] {
        let line = parse_line(sep, 3, &[]);
        assert!(
            matches!(line.kind, LineKind::Blank),
            "expected Blank for separator {sep:?}, got {:?}",
            line.kind
        );
    }
}

#[test]
fn em_dash_and_short_runs_are_not_thematic_breaks() {
    // A real em-dash (single glyph) and a 2-char run are content, not breaks.
    assert!(!matches!(
        parse_line("\u{2014}", 3, &[]).kind,
        LineKind::Blank
    ));
    assert!(!matches!(parse_line("--", 3, &[]).kind, LineKind::Blank));
    // A dashed bullet keeps its text — only pure marker runs are breaks.
    assert!(!matches!(
        parse_line("- real bullet", 3, &[]).kind,
        LineKind::Blank
    ));
}

// ── Leading-blank-line regression (PDF extraction emits a leading blank) ────

/// Root-cause regression: PDF-extracted résumé text routinely starts with a
/// blank line before the real header. The name rule must key on the first
/// line WITH CONTENT (idx 1 here), not raw idx 0 — otherwise the name falls
/// through to `is_all_caps_section_heading`, `model_from_resume_text` never
/// sets `seen_section` correctly, and the header renders twice (once from
/// the empty header + once as a bogus body section titled with the name).
/// Asserts BOTH symptoms: the header name is populated correctly AND no
/// section in the model is (incorrectly) headed with the candidate's name.
#[test]
fn leading_blank_line_before_name_still_yields_header_not_section() {
    let model = crate::model::adapter::model_from_resume_text(
        "\nSAEED KOLIVAND\nAI & Full-Stack Engineer\nKöln, Germany | a@b.com | +49 179 1402319\n\nPROFESSIONAL SUMMARY\nSome summary.",
    );

    assert_eq!(
        model.header.name, "SAEED KOLIVAND",
        "leading blank line must not prevent the name from populating the header"
    );
    assert!(
        !model
            .sections
            .iter()
            .any(|s| s.heading == model.header.name),
        "the name must never become a section heading; sections: {:?}",
        model
            .sections
            .iter()
            .map(|s| &s.heading)
            .collect::<Vec<_>>()
    );
}

/// Control case: a genuine section heading as the first line WITH CONTENT
/// (still preceded by a leading blank) must still classify as a heading —
/// proving the fix didn't turn real headings into names.
#[test]
fn leading_blank_line_before_section_heading_still_classifies_as_heading() {
    let lines = ["", "EXPERIENCE", "Some body text"];
    let line = parse_line("EXPERIENCE", 1, &lines);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader for the first content line, got {:?}",
        line.kind
    );
}
