//! The text-level readback checks (`evaluate`, `expected_from_request`, `normalize`) on
//! synthetic extracted text: deterministic, no rendering.

use super::{support::*, *};

fn expected(headings: &[&str]) -> Expected {
    Expected {
        name: Some("Jane Doe".to_string()),
        email: Some("jane@example.com".to_string()),
        headings: headings.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn in_order_sections_are_clean() {
    let e = expected(&["EXPERIENCE", "SKILLS", "EDUCATION"]);
    let extracted = "Jane Doe jane@example.com EXPERIENCE lots of work SKILLS rust EDUCATION uni";
    let issues = evaluate(&e, extracted, true, DocumentType::Resume);
    assert!(
        !has_critical(&issues),
        "in-order two-column should be clean: {issues:?}"
    );
    assert!(issues.is_empty(), "no issues expected: {issues:?}");
}

#[test]
fn interleaved_two_column_is_warning_not_blocking() {
    let e = expected(&["EXPERIENCE", "SKILLS", "EDUCATION"]);
    // SKILLS (a sidebar section) surfaces before EXPERIENCE in extraction order.
    // That is inherent to a two-column layout (the sidebar is a separate column),
    // not a defect — so it must be a non-blocking WARNING, never critical. A
    // critical here made `validate_and_fix` silently re-render single-column,
    // overriding the user's explicit two-column + ATS-off choice.
    let extracted = "Jane Doe jane@example.com SKILLS rust EXPERIENCE lots of work EDUCATION uni";
    let issues = evaluate(&e, extracted, true, DocumentType::Resume);
    assert!(
        !has_critical(&issues),
        "two-column reading order must not block: {issues:?}"
    );
    assert!(
        issues
            .iter()
            .any(|i| i.code == "section_order" && i.severity == Severity::Warning),
        "interleaved two-column must surface as a warning: {issues:?}"
    );
}

#[test]
fn out_of_order_single_column_is_only_a_warning() {
    let e = expected(&["EXPERIENCE", "SKILLS", "EDUCATION"]);
    let extracted = "Jane Doe jane@example.com SKILLS rust EXPERIENCE lots of work EDUCATION uni";
    let issues = evaluate(&e, extracted, false, DocumentType::Resume);
    assert!(
        !has_critical(&issues),
        "single-column order is non-blocking: {issues:?}"
    );
    assert!(issues
        .iter()
        .any(|i| i.code == "section_order" && i.severity == Severity::Warning));
}

#[test]
fn missing_section_is_a_warning_not_a_block() {
    let e = expected(&["EXPERIENCE", "SKILLS", "EDUCATION"]);
    let extracted = "Jane Doe jane@example.com EXPERIENCE lots of work EDUCATION uni"; // SKILLS dropped
    let issues = evaluate(&e, extracted, false, DocumentType::Resume);
    assert!(!has_critical(&issues));
    assert!(issues.iter().any(|i| i.code == "missing_section"));
}

#[test]
fn no_extractable_text_is_critical() {
    let e = expected(&["EXPERIENCE", "SKILLS"]);
    let issues = evaluate(&e, "   ", true, DocumentType::Resume);
    assert!(
        issues
            .iter()
            .any(|i| i.code == "no_extractable_text" && i.severity == Severity::Critical),
        "empty extraction must be critical: {issues:?}"
    );
}

#[test]
fn missing_name_and_email_are_warnings() {
    let e = expected(&["EXPERIENCE"]);
    let extracted = "Somebody Else nobody@nowhere.test EXPERIENCE lots of work here too";
    let issues = evaluate(&e, extracted, false, DocumentType::Resume);
    assert!(!has_critical(&issues));
    assert!(issues.iter().any(|i| i.code == "missing_name"));
    assert!(issues.iter().any(|i| i.code == "missing_email"));
}

/// H: `meta.candidate_name` is a FALLBACK ONLY, mirroring the renderers'
/// precedence — must never override a name the text already has. A stale
/// `meta.candidate_name` from an earlier generation (the user has since
/// edited the header) used to unconditionally win here, computing an
/// "expected" name the real render never shows — firing a spurious
/// `missing_name` warning on a perfectly valid, correctly-rendered document.
///
/// The other side: metadata still fills a genuinely blank header, same as
/// the renderers' own fallback.
#[test]
fn expected_name_is_text_derived_when_present_metadata_never_overrides() {
    for (text, meta_name, name) in [
        // Text has a name: a stale `meta.candidate_name` never wins over it.
        (
            "Jane Doe\njane@example.com\n\nSUMMARY\nSome text.",
            "Someone Else",
            "Jane Doe",
        ),
        // Text has none: metadata fills the genuinely blank header.
        (
            "jane@example.com\n\nSUMMARY\nSome text.",
            "Jane Smith",
            "Jane Smith",
        ),
    ] {
        let request = ExportRequest {
            text: text.to_string(),
            meta: Some(GenerationMeta {
                candidate_name: Some(meta_name.to_string()),
                job_title: None,
                company_name: None,
                target_language: None,
            }),
            ..req(ExportFormat::Pdf, TemplateId::SwissMinimal, false)
        };
        assert_eq!(expected_from_request(&request).name.as_deref(), Some(name));
    }
}

#[test]
fn normalize_collapses_to_lowercase_alphanumeric() {
    assert_eq!(normalize("  Hello,  World!  "), "hello world");
    assert_eq!(normalize("EXPERIENCE"), "experience");
}

#[test]
fn strip_xml_tags_keeps_run_text_separated() {
    let xml = "<w:p><w:r><w:t>Hello</w:t></w:r><w:r><w:t>World</w:t></w:r></w:p>";
    assert_eq!(normalize(&strip_xml_tags(xml)), "hello world");
    assert_eq!(strip_xml_tags("a &amp; b").trim(), "a & b");
}
