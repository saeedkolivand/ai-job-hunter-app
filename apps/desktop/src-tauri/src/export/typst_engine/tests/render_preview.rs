//! Smoke tests + SVG live-preview emit + header contact-link annotation round-trip.

use super::fixtures::opts_a4;
use super::letter_fixtures::LETTER_FIXTURE_US;
use super::pdf_introspect::link_uris;
use super::resume_fixtures::FIXTURE_RESUME;
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::{
    render_letter_svg_pages, render_pdf, render_pdf_from_source, render_resume_svg_pages,
    TypstTemplate,
};
use crate::model::adapter::model_from_resume_text;

/// Minimal Typst document that exercises font loading and basic layout.
const SMOKE_SOURCE: &str = "= Hello\n\nSome body text rendered with the bundled font.";

#[test]
fn smoke_pdf_is_non_empty_and_starts_with_pdf_header() {
    let bytes = render_pdf_from_source(SMOKE_SOURCE)
        .expect("render_pdf_from_source should succeed for a trivial document");

    assert!(!bytes.is_empty(), "rendered PDF must be non-empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "rendered PDF must begin with %PDF, got: {:?}",
        &bytes[..4.min(bytes.len())]
    );
}

#[test]
fn smoke_pdf_text_extraction_contains_expected_words() {
    let bytes = render_pdf_from_source(SMOKE_SOURCE)
        .expect("render_pdf_from_source should succeed for a trivial document");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract should be able to read our output");

    let lower = extracted.to_lowercase();
    assert!(
        lower.contains("hello"),
        "extracted text should contain 'hello'; got: {extracted:?}"
    );
    assert!(
        lower.contains("body"),
        "extracted text should contain 'body'; got: {extracted:?}"
    );
}

//
// The live preview renders the SAME model + SAME Typst world as the PDF export,
// emitting one SVG string per page instead of a PDF blob. These guard that the
// SVG sibling fns return ≥1 non-empty page whose string is a real SVG document.

#[test]
fn render_resume_svg_pages_returns_svg_page() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let pages = render_resume_svg_pages(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_resume_svg_pages(classic) should succeed");

    assert!(
        !pages.is_empty(),
        "résumé preview must produce at least one page"
    );
    for (i, page) in pages.iter().enumerate() {
        assert!(
            page.contains("<svg"),
            "résumé preview page {i} must contain an <svg root element; got start: {:?}",
            &page[..page.len().min(80)]
        );
    }
}

#[test]
fn document_accent_overrides_letter_accent_color() {
    use super::super::letter::style_from_template as letter_style_from_template;

    // Cover letters inherit the résumé template's accent. A document accent
    // applied via `Template::with_accent_override` must surface as the letter's
    // `c_accent`; a malformed value must leave the template's palette intact.
    let base_accent = letter_style_from_template(&Template::get(TemplateId::Classic)).c_accent;

    let overridden = Template::get(TemplateId::Classic).with_accent_override(Some("#AA0000"));
    assert_eq!(
        letter_style_from_template(&overridden).c_accent,
        "#AA0000",
        "a valid document accent must recolor the letter accent"
    );

    let malformed = Template::get(TemplateId::Classic).with_accent_override(Some("nope"));
    assert_eq!(
        letter_style_from_template(&malformed).c_accent,
        base_accent,
        "a malformed accent must leave the letter palette unchanged"
    );
}

#[test]
fn render_letter_svg_pages_returns_svg_page() {
    let t = Template::get(TemplateId::SwissMinimal);
    let pages = render_letter_svg_pages(
        LETTER_FIXTURE_US,
        &t,
        None,
        Some("Jane Smith"),
        LetterRender {
            market: "us",
            lang: "en",
            layout: LetterLayout::Classic,
            ats: false,
        },
    )
    .expect("render_letter_svg_pages(us) should succeed");

    assert!(
        !pages.is_empty(),
        "cover-letter preview must produce at least one page"
    );
    for (i, page) in pages.iter().enumerate() {
        assert!(
            page.contains("<svg"),
            "cover-letter preview page {i} must contain an <svg root element; got start: {:?}",
            &page[..page.len().min(80)]
        );
    }
}

//
// The header carries the candidate's email/LinkedIn/GitHub as clickable links.
// They must survive into the PDF as real `/Link` annotations with extractable
// `/A /URI` targets — the exact path that regressed before (lopdf inline-annot
// parsing). Render through the live engine, then read the links back. Replaces the
// `resume_embeds_contact_link_annotations` + `every_template_renders_a_valid_pdf`
// coverage that lived in the deleted printpdf `layout_pdf` suite.

#[test]
fn classic_resume_embeds_contact_link_annotations() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed");
    let uris = link_uris(&bytes);
    assert!(
        uris.iter().any(|u| u.contains("linkedin.com/in/janedoe")),
        "LinkedIn link annotation missing from classic resume; found {uris:?}"
    );
    assert!(
        uris.iter().any(|u| u.contains("github.com/janedoe")),
        "GitHub link annotation missing from classic resume; found {uris:?}"
    );
}

#[test]
fn two_column_resume_embeds_contact_link_annotations() {
    // The full-width header in two-column templates is the higher-risk path for
    // dropped annotations, so assert links survive there too (Atelier).
    let model = model_from_resume_text(FIXTURE_RESUME);
    let template = Template::get(TemplateId::Atelier);
    let bytes = render_pdf(
        &model,
        TypstTemplate::from_template(&template),
        &opts_a4(),
        Some(&template),
    )
    .expect("render_pdf(atelier) should succeed");
    let uris = link_uris(&bytes);
    assert!(
        uris.iter().any(|u| u.contains("linkedin.com/in/janedoe")),
        "LinkedIn link annotation missing from two-column resume; found {uris:?}"
    );
    assert!(
        uris.iter().any(|u| u.contains("github.com/janedoe")),
        "GitHub link annotation missing from two-column resume; found {uris:?}"
    );
}
