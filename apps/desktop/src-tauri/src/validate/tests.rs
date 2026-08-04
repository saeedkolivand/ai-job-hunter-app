//! Validation tests. The `evaluate` logic is unit-tested on synthetic extracted
//! text (deterministic, no rendering), and `validate_and_fix` is exercised
//! end-to-end against real generated PDFs/DOCX to guard against false-positive
//! blocking of valid documents.

use super::*;
use crate::export::types::{LetterLayout, TemplateId};

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

/// Same as [`RESUME`] but with no pre-section contact line, so a contact
/// profile applied via [`req`] is the header's source of truth (H) — needed by
/// checks that specifically exercise a profile-driven header link.
const RESUME_NO_CONTACT_LINE: &str = "\
Jane Doe

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
        letter_layout: LetterLayout::Classic,
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
    // No pre-section contact line — the profile must be the header's source of
    // truth (H) for its link to render at all.
    request.text = RESUME_NO_CONTACT_LINE.to_string();
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
    // No pre-section contact line in the text (H: the profile is only the
    // header's source of truth for a document that has none of its own) — this
    // check exercises exactly that case.
    request.text = RESUME_NO_CONTACT_LINE.to_string();
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

/// H: when the résumé text already carries its own contact line, the profile
/// is a fallback (never applied), so a profile URL that the text's own header
/// doesn't happen to repeat must NOT be flagged — neither as a "leaked" link
/// nor as "missing". This is the common real-world shape (an imported résumé's
/// own email/links vs. a separately-maintained Contact Profile).
#[test]
fn text_derived_header_is_never_checked_against_an_unrelated_profile() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    // RESUME (the default req() text) already has its own pre-section contact
    // line ("jane@example.com"); the profile below shares nothing with it.
    request.contact = Some(profile_with("https://drive.google.com/unrelated"));
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(report.ok, "must not block: {:?}", report.issues);
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.code == "header_url_mismatch" || i.code == "header_url_missing"),
        "a text-derived header must not be checked against an unrelated, unapplied \
         profile: {:?}",
        report.issues
    );
}

/// H: `profile_is_header_source` must parse the SAME text `prepare_resume_render`
/// actually renders from. Left un-extracted, the "### CANDIDATE RESUME ###"
/// marker classifies as a section heading at line 0, `header.contact` on the
/// raw-parsed model looks empty, and the strict parity checks wrongly run
/// against a header that was, in the real render, entirely text-derived —
/// reintroducing the false `header_url_mismatch` block H exists to remove.
#[test]
fn marker_wrapped_text_is_extracted_before_the_header_source_check() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.text = format!(
        "### CANDIDATE RESUME ###\n{RESUME}### JOB ADVERTISEMENT ###\n\
         Some job ad text about a role."
    );
    request.contact = Some(profile_with("https://drive.google.com/unrelated"));
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(report.ok, "must not block: {:?}", report.issues);
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.code == "header_url_mismatch"),
        "marker-wrapped, text-derived header must not be checked against an \
         unrelated profile: {:?}",
        report.issues
    );
}

/// When the profile is only a fallback (text already has its own contact
/// line), a job-board/ATS host reaching the header band is still worth a
/// warning — never blocking (the header is user-owned and visible in the
/// editor), but not a silent skip either.
#[test]
fn job_board_host_in_a_text_derived_header_is_warned_not_blocked() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.text = "Jane Doe\njane@example.com | https://www.indeed.com/cmp/acme\n\n\
                     EXPERIENCE\nAcme Corp  2020 - Present\nSenior Engineer\n\
                     - Led a team of five engineers delivering the core platform\n"
        .to_string();
    request.contact = Some(profile_with("https://example.dev/portfolio"));
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(
        report.ok,
        "a job-board link in a text-derived header must warn, not block: {:?}",
        report.issues
    );
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.code == "header_url_job_board" && i.severity == Severity::Warning),
        "a job-board host in a text-derived header must surface as a warning: {:?}",
        report.issues
    );
}

/// Security re-review: the job-board warning must not depend on a contact
/// profile being present — the people most likely to export a raw imported
/// header untouched are exactly the ones who never filled one in
/// (`request.contact` absent here, the common shape for them, not `Some(...)`
/// as the sibling test above uses).
#[test]
fn job_board_host_is_warned_even_with_no_contact_profile_at_all() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.text = "Jane Doe\njane@example.com | https://www.indeed.com/cmp/acme\n\n\
                     EXPERIENCE\nAcme Corp  2020 - Present\nSenior Engineer\n\
                     - Led a team of five engineers delivering the core platform\n"
        .to_string();
    request.contact = None;
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(
        report.ok,
        "a job-board link in a text-derived header must warn, not block: {:?}",
        report.issues
    );
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.code == "header_url_job_board" && i.severity == Severity::Warning),
        "a job-board host must surface as a warning even with no contact profile at all: {:?}",
        report.issues
    );
}

/// MEDIUM-3 (security re-review): `xing.com` is also a `JOB_BOARD_HOSTS`
/// entry (Xing hosts job listings too), so without a personal-profile
/// exemption a legitimate DACH candidate's own `/profile/…` Xing link warned
/// every time — the same shape of exemption LinkedIn already has via its
/// `/in/` gate.
#[test]
fn personal_xing_profile_in_the_header_is_exempt_from_the_job_board_warning() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.text = "Jane Doe\njane@example.com | https://www.xing.com/profile/Jane_Doe\n\n\
                     EXPERIENCE\nAcme Corp  2020 - Present\nSenior Engineer\n\
                     - Led a team of five engineers delivering the core platform\n"
        .to_string();
    request.contact = None;
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(report.ok, "must not block: {:?}", report.issues);
    assert!(
        !report.issues.iter().any(|i| i.code == "header_url_job_board"),
        "a personal Xing profile must be exempt from the job-board warning: {:?}",
        report.issues
    );
}

/// The exemption is narrow: a Xing URL that is NOT the `/profile/…` shape (a
/// job listing, the same host) must still warn — otherwise the exemption
/// would swallow the exact regression it sits next to.
#[test]
fn non_personal_xing_url_still_warns_as_job_board() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.text = "Jane Doe\njane@example.com | https://www.xing.com/jobs/12345\n\n\
                     EXPERIENCE\nAcme Corp  2020 - Present\nSenior Engineer\n\
                     - Led a team of five engineers delivering the core platform\n"
        .to_string();
    request.contact = None;
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.code == "header_url_job_board" && i.severity == Severity::Warning),
        "a non-personal Xing URL must still warn as job-board: {:?}",
        report.issues
    );
}

/// The blocking path itself, not just its skip/warning branches (the gate
/// narrowed to `profile_is_header_source`, and every other test here exercises
/// a case where it doesn't fire): a body company link genuinely rendering
/// inside the top-144pt header band, while the profile is the header's source
/// of truth, is the exact URL-swap regression `header_url_mismatch` exists to
/// catch — a body/company link is not one of the profile's own fields, so it
/// must block, not warn.
#[test]
fn company_link_leaking_into_the_header_band_is_a_blocking_mismatch() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.text = "\
Jane Doe

EXPERIENCE
[Acme Corp](https://acme.example.com)  2020 - Present
Senior Engineer
- Led a team of five engineers delivering the core platform

SKILLS
- Rust, TypeScript, React
"
    .to_string();
    request.contact = Some(profile_with("https://example.dev/portfolio"));
    let result = validate_and_fix(request, crate::export::pdf::generate_pdf);
    let (_bytes, report) = result.expect("pdf export (validate_and_fix reports, doesn't fail)");
    assert!(
        !report.ok,
        "a company link leaking into the header band must block the export: {:?}",
        report.issues
    );
    assert!(
        report.issues.iter().any(|i| i.code == "header_url_mismatch"
            && i.severity == Severity::Critical
            && i.message.contains("https://acme.example.com")),
        "the company link must be named as the mismatch: {:?}",
        report.issues
    );
}

/// HIGH-2 (security re-review): the mismatch check used to be gated on
/// `profile_is_header_source` — skipped entirely (down to the narrower,
/// job-board-only warning) once the text already had its own contact line,
/// so a NON-job-board company link leaking into the header band went
/// completely unvalidated in the common (text-owns-the-header) case. Fixed:
/// `allowed` is now built from whichever header is actually authoritative
/// for this render, so the exact same URL-swap-regression class this check
/// exists for is caught here too — text-owned header, unrelated profile
/// supplied, company link still blocks.
#[test]
fn company_link_leaking_into_a_text_owned_header_band_still_blocks() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.text = "\
Jane Doe
jane@example.com

EXPERIENCE
[Acme Corp](https://acme.example.com)  2020 - Present
Senior Engineer
- Led a team of five engineers delivering the core platform

SKILLS
- Rust, TypeScript, React
"
    .to_string();
    // A profile is supplied but irrelevant here — text already owns the
    // header, so the profile is never applied; the mismatch must still fire
    // against the text's OWN header links, not this unrelated profile.
    request.contact = Some(profile_with("https://example.dev/portfolio"));
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(
        !report.ok,
        "a company link leaking into a text-owned header band must block: {:?}",
        report.issues
    );
    assert!(
        report.issues.iter().any(|i| i.code == "header_url_mismatch"
            && i.severity == Severity::Critical
            && i.message.contains("https://acme.example.com")),
        "the company link must be named as the mismatch: {:?}",
        report.issues
    );
}

/// The completeness/"missing" check stays scoped to when the profile
/// actually supplied the header — comparing an unrelated profile's links
/// against a text-owned header would otherwise fire a false "missing" for
/// every one of the profile's links, on a document the profile never
/// touched at all.
#[test]
fn missing_check_does_not_fire_against_an_unrelated_profile_on_a_text_owned_header() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.text = RESUME.to_string(); // already has its own "jane@example.com" contact line
    request.contact = Some(profile_with("https://drive.google.com/unrelated"));
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(report.ok, "must not block: {:?}", report.issues);
    assert!(
        !report.issues.iter().any(|i| i.code == "header_url_missing"),
        "an unrelated, unapplied profile's links must not be reported missing: {:?}",
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

fn typst_validate_ats(template_id: TemplateId) -> (Vec<u8>, ExportReport) {
    validate_and_fix(
        req(ExportFormat::Pdf, template_id, true),
        crate::export::pdf::generate_pdf,
    )
    .expect("typst pdf export (ats)")
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
        TemplateId::Cadence,
        TemplateId::Regent,
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
    for id in [
        TemplateId::Atelier,
        TemplateId::Portrait,
        TemplateId::Aria,
        TemplateId::Saffron,
    ] {
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

/// Aria/Saffron (PR4 design two-column templates) must also round-trip the
/// validator in ATS mode — same pattern as `typst_single_column_pdf_passes_validation`,
/// just with `ats_mode: true` so the linearized, photo-dropped render is what's
/// checked (the non-ATS assertion above only covers the two-column render).
#[test]
fn typst_two_column_ats_mode_pdf_passes_validation() {
    for id in [TemplateId::Aria, TemplateId::Saffron] {
        let (bytes, report) = typst_validate_ats(id);
        assert!(!bytes.is_empty(), "{id:?}: empty ATS-mode PDF");
        assert!(
            report.ok,
            "{id:?}: Typst ATS-mode PDF must pass validate_and_fix — issues: {:?}",
            report.issues
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.severity == Severity::Critical),
            "{id:?}: no critical issues expected on a valid ATS-mode Typst PDF, got: {:?}",
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
        letter_layout: LetterLayout::Classic,
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
