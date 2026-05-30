//! Validation tests. The `evaluate` logic is unit-tested on synthetic extracted
//! text (deterministic, no rendering), and `validate_and_fix` is exercised
//! end-to-end against real generated PDFs/DOCX to guard against false-positive
//! blocking of valid documents.

use super::*;

fn expected(headings: &[&str]) -> Expected {
    Expected {
        name: Some("Jane Doe".to_string()),
        email: Some("jane@example.com".to_string()),
        headings: headings.iter().map(|s| s.to_string()).collect(),
    }
}

// ─── evaluate: section order / presence ───────────────────────────────────────

#[test]
fn in_order_sections_are_clean() {
    let e = expected(&["EXPERIENCE", "SKILLS", "EDUCATION"]);
    let extracted = "Jane Doe jane@example.com EXPERIENCE lots of work SKILLS rust EDUCATION uni";
    let issues = evaluate(&e, extracted, true, DocumentType::Resume);
    assert!(!has_critical(&issues), "in-order two-column should be clean: {issues:?}");
    assert!(issues.is_empty(), "no issues expected: {issues:?}");
}

#[test]
fn interleaved_two_column_is_critical() {
    let e = expected(&["EXPERIENCE", "SKILLS", "EDUCATION"]);
    // SKILLS (a sidebar section) surfaces before EXPERIENCE → reading order broken.
    let extracted = "Jane Doe jane@example.com SKILLS rust EXPERIENCE lots of work EDUCATION uni";
    let issues = evaluate(&e, extracted, true, DocumentType::Resume);
    assert!(
        issues.iter().any(|i| i.code == "section_order" && i.severity == Severity::Critical),
        "interleaved two-column must be critical: {issues:?}"
    );
}

#[test]
fn out_of_order_single_column_is_only_a_warning() {
    let e = expected(&["EXPERIENCE", "SKILLS", "EDUCATION"]);
    let extracted = "Jane Doe jane@example.com SKILLS rust EXPERIENCE lots of work EDUCATION uni";
    let issues = evaluate(&e, extracted, false, DocumentType::Resume);
    assert!(!has_critical(&issues), "single-column order is non-blocking: {issues:?}");
    assert!(issues.iter().any(|i| i.code == "section_order" && i.severity == Severity::Warning));
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
        issues.iter().any(|i| i.code == "no_extractable_text" && i.severity == Severity::Critical),
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

// ─── pure helpers ─────────────────────────────────────────────────────────────

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

// ─── end-to-end: real renders must not be falsely blocked ─────────────────────

const RESUME: &str = "\
Jane Doe
jane@example.com

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Led a team of five engineers delivering the core platform

SKILLS
- Rust, TypeScript, React

EDUCATION
State University  2013 - 2017
BSc Computer Science
";

fn req(format: ExportFormat, template_id: TemplateId, ats_mode: bool) -> ExportRequest {
    ExportRequest {
        text: RESUME.to_string(),
        format,
        document_type: DocumentType::Resume,
        template_id,
        meta: None,
        ats_mode,
        locale: None,
    }
}

#[test]
fn single_column_pdf_is_not_blocked() {
    let (bytes, report) =
        validate_and_fix(req(ExportFormat::Pdf, TemplateId::Modern, false), |r| {
            crate::export::pdf::generate_pdf(r)
        })
        .expect("pdf export");
    assert!(!bytes.is_empty());
    assert!(report.ok, "a valid single-column resume must export: {:?}", report.issues);
    assert!(report.fixed.is_empty(), "no auto-fix expected for single column");
}

#[test]
fn resume_docx_is_not_blocked() {
    let (bytes, report) =
        validate_and_fix(req(ExportFormat::Docx, TemplateId::Modern, false), |r| {
            crate::export::docx::generate_docx(r)
        })
        .expect("docx export");
    assert!(!bytes.is_empty());
    assert!(report.ok, "{:?}", report.issues);
}

#[test]
fn two_column_pdf_is_never_blocked() {
    let (bytes, report) =
        validate_and_fix(req(ExportFormat::Pdf, TemplateId::TwoColumn, false), |r| {
            crate::export::pdf::generate_pdf(r)
        })
        .expect("pdf export");
    assert!(!bytes.is_empty());
    assert!(report.ok, "two-column export must auto-fix rather than block: {:?}", report.issues);
    // If extraction showed interleaving, the fix linearized to ATS single-column.
    if !report.fixed.is_empty() {
        assert!(report.ats_mode, "a linearize fix was applied but ats_mode is false");
    }
}

#[test]
fn txt_is_returned_unvalidated() {
    let (bytes, report) =
        validate_and_fix(req(ExportFormat::Txt, TemplateId::Modern, false), |r| {
            Ok(crate::export::parser::strip_md(&r.text).into_bytes())
        })
        .expect("txt export");
    assert!(!bytes.is_empty());
    assert!(report.ok);
    assert!(report.issues.is_empty());
    assert!(report.fixed.is_empty());
}
