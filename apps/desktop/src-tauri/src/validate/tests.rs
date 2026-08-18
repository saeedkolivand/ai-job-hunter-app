//! Validation tests. The `evaluate` logic is unit-tested on synthetic extracted
//! text (deterministic, no rendering), and `validate_and_fix` is exercised
//! end-to-end against real generated PDFs/DOCX to guard against false-positive
//! blocking of valid documents.

use super::*;
use crate::export::templates::{Template, TemplateTier, CANONICAL_TEMPLATE_IDS};
use crate::export::types::{GenerationMeta, LetterLayout, TemplateId};

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

/// H: `meta.candidate_name` is a FALLBACK ONLY, mirroring the renderers'
/// precedence — must never override a name the text already has. A stale
/// `meta.candidate_name` from an earlier generation (the user has since
/// edited the header) used to unconditionally win here, computing an
/// "expected" name the real render never shows — firing a spurious
/// `missing_name` warning on a perfectly valid, correctly-rendered document.
#[test]
fn expected_name_is_text_derived_when_present_metadata_never_overrides() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.text = "Jane Doe\njane@example.com\n\nSUMMARY\nSome text.".to_string();
    request.meta = Some(GenerationMeta {
        candidate_name: Some("Someone Else".to_string()),
        job_title: None,
        company_name: None,
        target_language: None,
    });
    let expected = expected_from_request(&request);
    assert_eq!(expected.name.as_deref(), Some("Jane Doe"));
}

/// The other side: metadata still fills a genuinely blank header, same as
/// the renderers' own fallback.
#[test]
fn expected_name_falls_back_to_metadata_when_text_has_none() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.text = "jane@example.com\n\nSUMMARY\nSome text.".to_string();
    request.meta = Some(GenerationMeta {
        candidate_name: Some("Jane Smith".to_string()),
        job_title: None,
        company_name: None,
        target_language: None,
    });
    let expected = expected_from_request(&request);
    assert_eq!(expected.name.as_deref(), Some("Jane Smith"));
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
///
/// Security re-review (round 7): this used `req()`'s default text (`RESUME`),
/// which already carries its own "jane@example.com" contact line — so under
/// H's fallback-only semantics the profile is NEVER applied to the header at
/// all, and this test's own claim ("a profile whose link the renderer
/// actually draws") went vacuous: it passed only because nothing involving
/// the profile ran, not because the scenario it names was exercised.
/// `RESUME_NO_CONTACT_LINE` puts the profile back in the header source
/// position, and the assertion now checks the link is genuinely drawn, not
/// just that nothing blocked.
#[test]
fn contact_profile_export_is_not_falsely_blocked() {
    let mut request = req(ExportFormat::Pdf, TemplateId::SwissMinimal, false);
    request.text = RESUME_NO_CONTACT_LINE.to_string();
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
    let links = rendered_links(&bytes);
    assert!(
        links.iter().any(
            |l| l.url == "https://drive.google.com/file/d/abc123/view?usp=drive_link"
                && l.page == 0
        ),
        "the profile's own link must be genuinely drawn, not just absent-of-blocking: {links:?}"
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
        !report
            .issues
            .iter()
            .any(|i| i.code == "header_url_job_board"),
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
    // The warning is advisory, never blocking — the header is user-owned and
    // visible in the editor.
    assert!(
        report.ok,
        "the job-board warning must not block: {:?}",
        report.issues
    );
}

/// CodeRabbit (security re-review, round 7): the mismatch check must NOT run
/// over the full 144pt geometric band — that band is a heuristic, and a
/// genuine, correctly-placed BODY link (a job's own company site) rendering
/// early on the page (a short header + an immediate section) is not "the
/// header's" just because it falls inside the same zone. Flagging it there
/// is exactly the false-block this check exists to avoid, not to cause.
/// Narrowed to the header's own expected link COUNT: with the profile as the
/// header's source (one expected link, its website), only the topmost band
/// link is checked — the job's own company link one line down survives
/// untouched, and the export is not blocked. (A prior round's test asserted
/// the OPPOSITE of this — that the company link DID block — which was
/// itself the false-block bug, not a real regression repro.)
#[test]
fn profile_sourced_header_with_a_genuine_body_link_in_the_band_does_not_false_block() {
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
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(
        report.ok,
        "a genuine body link rendering inside the header band must not false-block: {:?}",
        report.issues
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.code == "header_url_mismatch"),
        "the job's own company link must never be flagged as a header mismatch: {:?}",
        report.issues
    );
}

/// The regression test both reviewers were circling this round: same
/// scenario, but with the TEXT already owning the header (its own contact
/// line present) — the exact case HIGH-2 (prior round) re-pointed the
/// mismatch check onto. Must not false-block either — same narrowing, same
/// reasoning, the profile-sourced sibling above.
#[test]
fn text_owned_header_with_a_genuine_body_link_in_the_band_does_not_false_block() {
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
    // header, so the profile is never applied.
    request.contact = Some(profile_with("https://example.dev/portfolio"));
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(
        report.ok,
        "a genuine body link rendering inside a text-owned header band must not false-block: {:?}",
        report.issues
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.code == "header_url_mismatch"),
        "the job's own company link must never be flagged as a header mismatch: {:?}",
        report.issues
    );
}

/// Post-push security re-review (round 8): a SECOND reviewer read
/// `header_owned_links = header_links.take(allowed.len())` silently checking
/// zero links when `allowed` is empty as an unintentional hole and proposed
/// falling back to the FULL band in that case. Verified empirically (see the
/// commit history / round-8 report) before writing this: that fallback is
/// WRONG and would reintroduce the exact false-block this file's other
/// "…does_not_false_block" tests exist to prevent — a name-only header with
/// no contact profile and no email/phone-with-a-link produces `allowed = {}`
/// (a bare name is a plain `String`, never linked; a phone/location line
/// never gets a `.link` run either), so a job's own company link rendering
/// early on the page (short header, immediate EXPERIENCE section) is body
/// content, not the header's, and must not block. This is the SAME
/// invariant the other two tests pin, for the specific `allowed.is_empty()`
/// case neither of them actually exercises (both have exactly one expected
/// link) — added so the intent is an assertion, not something inferred from
/// `take(0)`'s iterator semantics.
#[test]
fn linkless_header_with_a_genuine_body_link_in_the_band_does_not_false_block() {
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
    // No contact profile, and the text's own header (just a bare name, no
    // contact line at all) supplies no links either — `allowed` is empty.
    request.contact = None;
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export");
    assert!(
        report.ok,
        "a genuine body link inside the band of a linkless header must not false-block: {:?}",
        report.issues
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.code == "header_url_mismatch"),
        "the job's own company link must never be flagged as a header mismatch just because \
         the header itself has no links to compare against: {:?}",
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

/// Assert a rendered export cleared the validator outright.
fn assert_validation_clean(id: TemplateId, label: &str, bytes: &[u8], report: &ExportReport) {
    assert!(!bytes.is_empty(), "{id:?} ({label}): empty PDF");
    assert!(
        report.ok,
        "{id:?} ({label}): Typst PDF must pass validate_and_fix — issues: {:?}",
        report.issues
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.severity == Severity::Critical),
        "{id:?} ({label}): no critical issues expected on a valid Typst PDF, got: {:?}",
        report.issues
    );
}

/// Every canonical template must clear the validator, split by column count so
/// the two layout paths stay explicit — `theme::is_two_column` is the same
/// single source of truth the renderer gates on, so the split can't drift from
/// what actually gets rendered.
///
/// Driven off [`CANONICAL_TEMPLATE_IDS`] rather than hardcoded id lists,
/// because the hardcoded ones silently stopped growing: they covered 8 + 4 of
/// twelve-then-sixteen templates, leaving Cologne Navy, Jake, Awesome and Deedy
/// with no validator coverage at all — including Awesome, which emits its
/// contact hyperlinks from inside `page.background`, precisely the annotation
/// shape the `empty_anchor_link` critical looks for.
///
/// **Cost is deliberate.** This test and
/// [`typst_ats_mode_pdf_passes_validation_for_every_toggle_bearing_template`]
/// below together compile 16 + 7 real Typst PDFs on every run — by far the
/// slowest thing in this file. That is the point: the coverage gap above existed
/// precisely *because* someone kept the list short. **Never trim the list to
/// speed it up** — a template dropped from the matrix is a template with no
/// validator coverage, and nothing else will notice.
///
/// The sanctioned mitigation, if the runtime ever genuinely hurts, is to move
/// the whole-roster matrices behind a slower test target (a `#[ignore]`d
/// nightly/CI job, or a separate `--test` binary) that still runs EVERY
/// template — not to sample a subset here.
#[test]
fn typst_every_canonical_template_pdf_passes_validation() {
    let mut single = 0;
    let mut two_col = 0;
    for id in CANONICAL_TEMPLATE_IDS {
        let (bytes, report) = typst_validate(id);
        let two_column = crate::theme::is_two_column(id);
        assert_validation_clean(
            id,
            if two_column {
                "two-column"
            } else {
                "single-column"
            },
            &bytes,
            &report,
        );
        if two_column {
            two_col += 1;
        } else {
            single += 1;
        }
    }
    // Both arms must actually be exercised — a `is_two_column` that started
    // answering `false` everywhere would otherwise turn this into a
    // single-column-only test without failing.
    assert!(
        single > 0 && two_col > 0,
        "expected both layout paths to be covered; got {single} single-column \
         and {two_col} two-column templates"
    );
}

/// ATS mode is a second render path (linearized, photo dropped, decorative
/// colour dropped) that the validator must also clear — and it is exactly the
/// path the design tier advertises. Every design-tier template surfaces the
/// toggle (`TemplateTier` doc comment), so that is the set checked here.
#[test]
fn typst_ats_mode_pdf_passes_validation_for_every_toggle_bearing_template() {
    let mut checked = 0;
    for id in CANONICAL_TEMPLATE_IDS {
        if Template::get(id).tier != TemplateTier::Design {
            continue;
        }
        let (bytes, report) = typst_validate_ats(id);
        assert_validation_clean(id, "ats", &bytes, &report);
        checked += 1;
    }
    assert!(
        checked >= 6,
        "expected the design tier to surface the ATS toggle on at least six \
         templates; only {checked} were checked"
    );
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

/// Regression for the production incident (a macOS user's PDF export blocked
/// with no way out): a cover letter's opening paragraph can legitimately
/// contain a markdown link (e.g. a company-research URL the AI echoes in its
/// first sentence). With a short letterhead — no date, no recipient block —
/// that link can land inside the same 144pt header band the letterhead's own
/// link renders in. It must never be mistaken for a header link and block the
/// export: a cover letter is never two-column, so `can_linearize` is always
/// false and a false block here has no remediation at all.
///
/// The band precondition is asserted implicitly rather than by measuring the
/// link's rect: verified by reverting the `topmost_n` narrowing in
/// `validate/mod.rs` and re-running, which fails here with exactly
/// `header_url_mismatch` on `acme.example.com`. That failure is only reachable
/// if the body link IS inside the 144pt band, so the fixture cannot silently
/// drift into passing for the wrong reason without the narrowing also
/// becoming untested — at which point this test starts passing on `main` too.
#[test]
fn cover_letter_body_link_in_a_short_letterhead_band_does_not_false_block() {
    let request = ExportRequest {
        text: "\
Jane Doe

Dear Hiring Manager,

I first learned about your team through [Acme Research](https://acme.example.com/about) \
and knew immediately I wanted to apply.

Sincerely,
Jane Doe
"
        .to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: Some(profile_with("https://example.dev/portfolio")),
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let (_bytes, report) =
        validate_and_fix(request, crate::export::pdf::generate_pdf).expect("cover letter export");
    assert!(
        report.ok,
        "a genuine body link inside a short cover letter's header band must not false-block: {:?}",
        report.issues
    );
    assert!(
        !report
            .issues
            .iter()
            .any(|i| i.code == "header_url_mismatch"),
        "the body link must never be flagged as a header mismatch: {:?}",
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

/// Regression: which band links count as "the header's own" must be decided by
/// where they sit on the page, never by `/Annots` emission order.
///
/// `page_link_annotations` returns annotations in array order, which carries no
/// guarantee of vertical position — a two-column template emits its sidebar as
/// its own run, so a body link can precede a header one. Selecting by emission
/// order would then pick the body link, raise a CRITICAL `header_url_mismatch`,
/// and make `validate_and_fix` silently re-render the document single-column.
///
/// The links below are deliberately supplied in the *worst* order: lowest on the
/// page first. Taking the first two by emission order yields the two body links;
/// taking them by geometry yields the header's.
#[test]
fn topmost_n_orders_by_geometry_not_annotation_order() {
    let link = |y: f32, url: &str| PdfLink {
        rect: [0.0, y, 100.0, y + 10.0],
        url: url.to_string(),
        page: 0,
    };
    // PDF user space is bottom-up: a larger y is higher on the page.
    let body_low = link(100.0, "https://acme.example.com/careers");
    let body_mid = link(300.0, "https://acme.example.com/team");
    let header_b = link(700.0, "https://github.com/jane");
    let header_a = link(720.0, "https://linkedin.com/in/jane");
    let emission_order = [&body_low, &body_mid, &header_b, &header_a];

    let picked = topmost_n(&emission_order, 2);

    assert_eq!(
        picked.iter().map(|l| l.url.as_str()).collect::<Vec<_>>(),
        vec!["https://linkedin.com/in/jane", "https://github.com/jane"],
        "must select the two highest links, top-down — selecting by emission \
         order would have picked the body links and false-blocked the export"
    );
}

/// `topmost_n` must not panic or over-take when asked for more links than exist.
#[test]
fn topmost_n_caps_at_the_available_link_count() {
    let only = PdfLink {
        rect: [0.0, 700.0, 100.0, 710.0],
        url: "https://linkedin.com/in/jane".to_string(),
        page: 0,
    };
    assert_eq!(topmost_n(&[&only], 5).len(), 1);
    assert!(topmost_n(&[], 3).is_empty());
}

// ── ADR-002 golden parity, actually enforced ─────────────────────────────────

/// The content every ATS must be able to read back out of an exported résumé,
/// drawn from [`RESUME`] — the same fixture both backends render.
///
/// Anchored on the SOURCE, deliberately. The obvious harness compares the PDF
/// text to the DOCX text, and that is the shape this repo has shipped broken
/// before: two derived values with nothing absolute behind them, so a change
/// that drops a section from BOTH backends keeps the test green while the
/// candidate silently submits a résumé missing their education. Comparing each
/// rendering against the input cannot pass that way.
///
/// Deliberately not the whole fixture verbatim: line breaks, hyphenation,
/// column order and glyph runs legitimately differ between a Typst page and a
/// Word document. What may NOT differ is whether a fact survived.
const PARITY_CONTENT: &[&str] = &[
    "Jane Doe",
    "EXPERIENCE",
    "Acme Corp",
    "Senior Engineer",
    "Led a team of five engineers",
    "SKILLS",
    "Rust",
    "TypeScript",
    "EDUCATION",
    "State University",
    "BSc Computer Science",
];

/// Normalize an extraction for containment checks: collapse whitespace (both
/// backends break lines differently and `strip_xml_tags` injects spaces at
/// every tag boundary) and lowercase.
fn parity_normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// **ADR-002's "golden parity" claim, enforced for the first time.**
///
/// The ADR says the two backends are kept "in golden parity where the design
/// requires, pinned by deterministic golden snapshot tests". The per-backend
/// tests were real, but nothing rendered ONE document through BOTH and checked
/// that the same facts came out — so a backend could silently drop a section
/// and only that backend's own snapshot would notice, if it covered it at all.
///
/// That is not hypothetical for this codebase: DOCX body bold had never
/// rendered at all, and a macOS incident shipped with the two formats
/// disagreeing. An ATS reads the extracted text, so a fact that survives one
/// export and not the other means the candidate submits a materially different
/// résumé depending on the button they pressed.
///
/// Runs the whole canonical roster rather than a sample, for the reason
/// [`typst_every_canonical_template_pdf_passes_validation`] gives: a template
/// nobody rendered is a template nobody validated.
#[test]
fn every_canonical_template_carries_the_same_facts_into_pdf_and_docx() {
    let mut checked = 0;

    for id in CANONICAL_TEMPLATE_IDS {
        let pdf_bytes = crate::export::pdf::generate_pdf(&req(ExportFormat::Pdf, id, false))
            .unwrap_or_else(|e| panic!("{id:?}: pdf export failed: {e}"));
        let docx_bytes = crate::export::docx::generate_docx(&req(ExportFormat::Docx, id, false))
            .unwrap_or_else(|e| panic!("{id:?}: docx export failed: {e}"));

        let pdf = parity_normalize(
            &super::extract_pdf_text(&pdf_bytes)
                .unwrap_or_else(|e| panic!("{id:?}: pdf text extraction failed: {e}")),
        );
        let docx = parity_normalize(
            &super::extract_docx_text(&docx_bytes)
                .unwrap_or_else(|e| panic!("{id:?}: docx text extraction failed: {e}")),
        );

        // Guard the guard: an extractor that silently returns nothing would
        // make every containment check below vacuous.
        assert!(
            pdf.len() > 100 && docx.len() > 100,
            "{id:?}: extraction produced almost nothing (pdf {} chars, docx {} chars) — \
             the parity assertions below would pass vacuously",
            pdf.len(),
            docx.len()
        );

        for fact in PARITY_CONTENT {
            let needle = parity_normalize(fact);
            let in_pdf = pdf.contains(&needle);
            let in_docx = docx.contains(&needle);
            assert!(
                in_pdf && in_docx,
                "{id:?}: {fact:?} survived into {} but not {} — an ATS reads the \
                 extracted text, so the two formats are not the same résumé",
                if in_pdf { "the PDF" } else { "the DOCX" },
                if in_pdf { "the DOCX" } else { "the PDF" }
            );
        }
        checked += 1;
    }

    assert!(
        checked > 0,
        "the canonical roster was empty, so this proved nothing"
    );
}
