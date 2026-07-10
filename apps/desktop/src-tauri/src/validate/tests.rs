//! Validation tests. The `evaluate` logic is unit-tested on synthetic extracted
//! text (deterministic, no rendering), and `validate_and_fix` is exercised
//! end-to-end against real generated PDFs/DOCX to guard against false-positive
//! blocking of valid documents.

use super::*;
use crate::export::types::TemplateId;

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
        contact: None,
        accent: None,
    }
}

#[test]
fn single_column_pdf_is_not_blocked() {
    let (bytes, report) = validate_and_fix(
        req(ExportFormat::Pdf, TemplateId::SwissMinimal, false),
        crate::export::pdf::generate_pdf,
    )
    .expect("pdf export");
    assert!(!bytes.is_empty());
    assert!(
        report.ok,
        "a valid single-column resume must export: {:?}",
        report.issues
    );
    assert!(
        report.fixed.is_empty(),
        "no auto-fix expected for single column"
    );
}

#[test]
fn resume_docx_is_not_blocked() {
    let (bytes, report) = validate_and_fix(
        req(ExportFormat::Docx, TemplateId::SwissMinimal, false),
        |r| {
            // `generate_docx` is still on `anyhow::Result`; bridge to the typed error.
            crate::export::docx::generate_docx(r).map_err(crate::error::AppError::from)
        },
    )
    .expect("docx export");
    assert!(!bytes.is_empty());
    assert!(report.ok, "{:?}", report.issues);
}

#[test]
fn two_column_pdf_is_never_blocked() {
    // Atelier is the live two-column template (TwoColumn was deleted).
    let (bytes, report) =
        validate_and_fix(req(ExportFormat::Pdf, TemplateId::Atelier, false), |r| {
            crate::export::pdf::generate_pdf(r)
        })
        .expect("pdf export");
    assert!(!bytes.is_empty());
    assert!(
        report.ok,
        "two-column export must auto-fix rather than block: {:?}",
        report.issues
    );
    // If extraction showed interleaving, the fix linearized to ATS single-column.
    if !report.fixed.is_empty() {
        assert!(
            report.ats_mode,
            "a linearize fix was applied but ats_mode is false"
        );
    }
}

// ─── header link annotations (Typst inline-dict /Annots) ─────────────────────

/// Read every link annotation our renderer wrote, the way the header checks do.
fn rendered_links(bytes: &[u8]) -> Vec<PdfLink> {
    let doc = lopdf::Document::load_mem(bytes).expect("load pdf");
    doc.get_pages()
        .into_values()
        .enumerate()
        .flat_map(|(idx, page_id)| page_link_annotations(&doc, page_id, idx))
        .collect()
}

fn profile_with(website: &str) -> crate::contact_profile::ContactProfile {
    crate::contact_profile::ContactProfile {
        website: Some(website.to_string()),
        ..Default::default()
    }
}

/// Regression: lopdf's `get_page_annotations` only resolves *reference* entries,
/// but Typst writes `/Annots` as inline dictionaries — so the header-link
/// reader used to see zero links. It must now read our own renderer's output.
#[test]
fn reads_inline_dict_link_annotations_from_our_renderer() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.contact = Some(profile_with("https://example.dev/portfolio"));
    let bytes = crate::export::pdf::generate_pdf(&request).expect("pdf");

    let links = rendered_links(&bytes);
    assert!(
        links
            .iter()
            .any(|l| l.url == "https://example.dev/portfolio" && l.page == 0),
        "the contact-profile header link must be read back, got {links:?}"
    );
}

/// The reading regression meant ANY non-empty contact profile produced a phantom
/// "missing from the rendered header" critical and blocked every export. A profile
/// whose link the renderer actually draws must export cleanly.
#[test]
fn contact_profile_export_is_not_falsely_blocked() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.contact = Some(profile_with(
        "https://drive.google.com/file/d/abc123/view?usp=drive_link",
    ));
    let (bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(!bytes.is_empty());
    assert!(
        report.ok,
        "a résumé with a contact profile must export, not block: {:?}",
        report.issues
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.severity == Severity::Critical),
        "no critical header issues expected: {:?}",
        report.issues
    );
}

/// A profile link that genuinely does not surface in the header is advisory
/// (warning), never blocking — a missing contact link does not corrupt the doc.
#[test]
fn missing_header_link_is_warning_not_block() {
    // A `mailto:` is in the profile's header_urls, but a website-only header line
    // can leave it unrendered depending on layout; whatever surfaces, a non-matching
    // profile URL must downgrade to a warning rather than block.
    let mut profile = profile_with("https://example.dev/site");
    profile.extra_links = vec![crate::contact_profile::ContactLink {
        label: String::new(), // empty label → header_markdown never renders it…
        url: "https://example.dev/never-rendered".to_string(), // …but header_urls lists it
    }];
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.contact = Some(profile);
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(report.ok, "missing header link must not block: {report:?}");
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.code == "header_url_missing" && i.severity == Severity::Warning),
        "the unrendered profile link must surface as a warning: {:?}",
        report.issues
    );
}

#[test]
fn txt_is_returned_unvalidated() {
    let (bytes, report) = validate_and_fix(
        req(ExportFormat::Txt, TemplateId::SwissMinimal, false),
        |r| Ok(crate::export::parser::strip_md(&r.text).into_bytes()),
    )
    .expect("txt export");
    assert!(!bytes.is_empty());
    assert!(report.ok);
    assert!(report.issues.is_empty());
    assert!(report.fixed.is_empty());
}

// ─── validate_and_fix on Typst-rendered PDFs ──────────────────────────────────
//
// After Cutover-1 every template goes through the Typst engine. The validator
// must not false-positive on a valid Typst PDF (the coordinate-origin and
// text-positioning characteristics of Typst must not produce
// spurious "empty_anchor_link" or "no_extractable_text" criticals).

/// Helper: render via the now-live generate_pdf (Typst) and run validate_and_fix.
fn typst_validate(template_id: TemplateId) -> (Vec<u8>, ExportReport) {
    validate_and_fix(
        req(ExportFormat::Pdf, template_id, false),
        crate::export::pdf::generate_pdf,
    )
    .expect("typst pdf export")
}

#[test]
fn typst_single_column_pdf_passes_validation() {
    for id in [
        TemplateId::Classic,
        TemplateId::SwissMinimal,
        TemplateId::Academic,
        TemplateId::Meridian,
        TemplateId::Throughline,
        TemplateId::Lebenslauf,
    ] {
        let (bytes, report) = typst_validate(id);
        assert!(!bytes.is_empty(), "{id:?}: empty PDF");
        assert!(
            report.ok,
            "{id:?}: Typst single-column PDF must pass validate_and_fix — issues: {:?}",
            report.issues
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.severity == Severity::Critical),
            "{id:?}: no critical issues expected on a valid Typst PDF, got: {:?}",
            report.issues
        );
    }
}

#[test]
fn typst_two_column_atelier_pdf_passes_validation() {
    for id in [TemplateId::Atelier, TemplateId::Portrait] {
        let (bytes, report) = typst_validate(id);
        assert!(!bytes.is_empty(), "{id:?}: empty PDF");
        assert!(
            report.ok,
            "Typst two-column {id:?} PDF must pass validate_and_fix — issues: {:?}",
            report.issues
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.severity == Severity::Critical),
            "{id:?}: no critical issues expected on a valid Typst PDF, got: {:?}",
            report.issues
        );
    }
}

/// The cover-letter path also runs through Typst; validate that it passes.
#[test]
fn typst_cover_letter_pdf_passes_validation() {
    let request = ExportRequest {
        text: "Jane Doe\njane@example.com\n\nDear Hiring Manager,\n\nI am writing to apply.\n\nSincerely,\nJane Doe".to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
    };
    let (bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("cover letter export");
    assert!(!bytes.is_empty(), "cover letter PDF must not be empty");
    assert!(
        report.ok,
        "Typst cover letter PDF must pass validate_and_fix — issues: {:?}",
        report.issues
    );
}

// ─── canonicalize_url ─────────────────────────────────────────────────────────

/// Query-string case (including values like `Token=ABC`) must be preserved.
/// The old code found '/' at `after.len()` but still treated `?Token=ABC` as
/// part of the authority — so `authority.to_lowercase()` clobbered the token.
#[test]
fn canonicalize_url_preserves_query_case() {
    let url = "https://Example.COM/path?Token=ABC&foo=Bar";
    let canon = canonicalize_url(url);
    // Scheme + host lowercased; path + query case preserved.
    assert_eq!(canon, "https://example.com/path?Token=ABC&foo=Bar");
}

/// A URL with no path separator before the query must not lowercase the query.
#[test]
fn canonicalize_url_query_without_path_slash() {
    let url = "https://Example.COM?Token=ABC";
    let canon = canonicalize_url(url);
    assert_eq!(canon, "https://example.com?Token=ABC");
}

/// Fragment identifiers must not be lowercased or mangled.
#[test]
fn canonicalize_url_preserves_fragment() {
    let url = "https://Example.COM/page#SectionTitle";
    let canon = canonicalize_url(url);
    assert_eq!(canon, "https://example.com/page#SectionTitle");
}

/// A trailing slash on the PATH (before any `?`) is stripped; the query is kept.
#[test]
fn canonicalize_url_strips_path_trailing_slash_before_query() {
    let url = "https://example.com/profile/?Token=ABC";
    let canon = canonicalize_url(url);
    assert_eq!(canon, "https://example.com/profile?Token=ABC");
}

/// Two genuinely different URLs (different hosts / paths) must never compare equal.
#[test]
fn canonicalize_url_different_urls_are_not_equal() {
    let a = canonicalize_url("https://linkedin.com/in/janedoe");
    let b = canonicalize_url("https://github.com/janedoe");
    assert_ne!(
        a, b,
        "different URLs must not collide after canonicalization"
    );
}

/// Regression: a Google Drive URL with a query containing uppercase must not be
/// lowercased — this is the exact URL shape that false-blocked exports.
#[test]
fn canonicalize_url_google_drive_link_is_stable() {
    let url = "https://drive.google.com/file/d/abc123/view?usp=drive_link";
    // Canonicalizing twice must yield the same string (idempotent).
    let once = canonicalize_url(url);
    let twice = canonicalize_url(&once);
    assert_eq!(once, twice, "canonicalize_url must be idempotent");
    // The query value must survive unchanged.
    assert!(
        once.contains("?usp=drive_link"),
        "query must survive: {once}"
    );
}
