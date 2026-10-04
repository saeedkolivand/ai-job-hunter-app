//! Header link annotations read back from our renderer, and which of them a contact profile,
//! a text-owned header or a job-board host may raise.

use super::{support::*, *};

/// Read every link annotation our renderer wrote, the way the header checks do.
pub(super) fn rendered_links(bytes: &[u8]) -> Vec<PdfLink> {
    let doc = lopdf::Document::load_mem(bytes).expect("load pdf");
    doc.get_pages()
        .into_values()
        .enumerate()
        .flat_map(|(idx, page_id)| page_link_annotations(&doc, page_id, idx))
        .collect()
}

/// A résumé whose own contact line carries `link`.
fn resume_with_header_link(link: &str) -> String {
    format!(
        "Jane Doe\njane@example.com | {link}\n\n\
         EXPERIENCE\nAcme Corp  2020 - Present\nSenior Engineer\n\
         - Led a team of five engineers delivering the core platform\n"
    )
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
    let (bytes, report) = export_pdf(request);
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
    let (_bytes, report) = export_pdf(request);
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
///
/// H: `profile_is_header_source` must parse the SAME text `prepare_resume_render`
/// actually renders from. Left un-extracted, the "### CANDIDATE RESUME ###"
/// marker classifies as a section heading at line 0, `header.contact` on the
/// raw-parsed model looks empty, and the strict parity checks wrongly run
/// against a header that was, in the real render, entirely text-derived —
/// reintroducing the false `header_url_mismatch` block H exists to remove.
///
/// The completeness/"missing" check stays scoped to when the profile
/// actually supplied the header — comparing an unrelated profile's links
/// against a text-owned header would otherwise fire a false "missing" for
/// every one of the profile's links, on a document the profile never
/// touched at all.
#[test]
fn an_unrelated_profile_is_never_checked_against_a_text_owned_header() {
    let marker_wrapped = format!(
        "### CANDIDATE RESUME ###\n{RESUME}### JOB ADVERTISEMENT ###\nSome job ad text about a role."
    );
    // RESUME (the default req() text) already has its own pre-section contact
    // line ("jane@example.com"); the profile below shares nothing with it.
    for (text, forbidden, why) in [
        (
            RESUME,
            &["header_url_mismatch", "header_url_missing"][..],
            "a text-derived header must not be checked against an unrelated, unapplied profile",
        ),
        (
            marker_wrapped.as_str(),
            &["header_url_mismatch"][..],
            "marker-wrapped, text-derived header must not be checked against an unrelated profile",
        ),
        (
            RESUME,
            &["header_url_missing"][..],
            "an unrelated, unapplied profile's links must not be reported missing",
        ),
    ] {
        let report = pdf_report(
            text,
            Some(profile_with("https://drive.google.com/unrelated")),
        );
        assert!(report.ok, "must not block: {:?}", report.issues);
        assert!(
            !report
                .issues
                .iter()
                .any(|i| forbidden.contains(&i.code.as_str())),
            "{why}: {:?}",
            report.issues
        );
    }
}

/// When the profile is only a fallback (text already has its own contact
/// line), a job-board/ATS host reaching the header band is still worth a
/// warning — never blocking (the header is user-owned and visible in the
/// editor), but not a silent skip either.
///
/// Security re-review: the job-board warning must not depend on a contact
/// profile being present — the people most likely to export a raw imported
/// header untouched are exactly the ones who never filled one in
/// (`request.contact` absent here, the common shape for them, not `Some(...)`
/// as the sibling test above uses).
#[test]
fn job_board_host_in_a_text_derived_header_is_warned_not_blocked() {
    for (contact, why) in [
        (
            Some(profile_with("https://example.dev/portfolio")),
            "a job-board host in a text-derived header must surface as a warning",
        ),
        (
            None,
            "a job-board host must surface as a warning even with no contact profile at all",
        ),
    ] {
        let report = pdf_report(
            &resume_with_header_link("https://www.indeed.com/cmp/acme"),
            contact,
        );
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
            "{why}: {:?}",
            report.issues
        );
    }
}

/// MEDIUM-3 (security re-review): `xing.com` is also a `JOB_BOARD_HOSTS`
/// entry (Xing hosts job listings too), so without a personal-profile
/// exemption a legitimate DACH candidate's own `/profile/…` Xing link warned
/// every time — the same shape of exemption LinkedIn already has via its
/// `/in/` gate.
#[test]
fn personal_xing_profile_in_the_header_is_exempt_from_the_job_board_warning() {
    let report = pdf_report(
        &resume_with_header_link("https://www.xing.com/profile/Jane_Doe"),
        None,
    );
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
    let report = pdf_report(
        &resume_with_header_link("https://www.xing.com/jobs/12345"),
        None,
    );
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
