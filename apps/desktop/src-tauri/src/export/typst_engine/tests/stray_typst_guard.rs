//! Stray-Typst-code guard: every template's extracted text must carry no un-evaluated Typst markup.

use super::fixtures::{
    fixture_photo_data_url, opts_a4, opts_atelier, opts_p3a, opts_photo, opts_sc, template_style,
};
use super::letter_fixtures::LETTER_FIXTURE_US;
use super::pdf_introspect::assert_no_stray_tokens;
use super::resume_fixtures::{ATELIER_FIXTURE, FIXTURE_RESUME, LEBENSLAUF_FIXTURE};
use crate::error::AppResult;
use crate::export::templates::Template;
use crate::export::types::{LetterLayout, LetterRender, TemplateId};
use crate::export::typst_engine::{
    render_letter_pdf, render_pdf, render_pdf_with_photo, resolve_photo, TypstTemplate,
};
use crate::model::adapter::model_from_resume_text;

/// Table-driven: renders every résumé template family through its live path
/// (plain or photo-aware) and asserts none of them leak un-evaluated Typst
/// markup into the extracted text. One `#[test]` covering the 10 résumé cases
/// that were previously 10 near-identical `stray_typst_code_guard_*` fns
/// (differing only in model/template/opts/photo — see issue #1280 batch 5a);
/// every case, label, and render call is unchanged. The cover-letter case
/// (`stray_typst_code_guard_letter`, below) keeps its own fn — `render_letter_pdf`
/// takes a different shape (raw text, not a `DocumentModel`).
#[test]
fn stray_typst_code_guard_every_resume_template() {
    let resume_model = model_from_resume_text(FIXTURE_RESUME);
    let atelier_model = model_from_resume_text(ATELIER_FIXTURE);
    let lebenslauf_model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let photo_png = resolve_photo(&fixture_photo_data_url());

    let cases: Vec<(&str, AppResult<Vec<u8>>)> = vec![
        (
            "classic",
            render_pdf(
                &resume_model,
                TypstTemplate::SingleColumn,
                &opts_a4(),
                Some(&Template::get(TemplateId::Classic)),
            ),
        ),
        (
            "swiss-minimal",
            render_pdf(
                &resume_model,
                TypstTemplate::SingleColumn,
                &opts_sc(),
                Some(&template_style(TemplateId::SwissMinimal)),
            ),
        ),
        (
            "academic",
            render_pdf(
                &resume_model,
                TypstTemplate::SingleColumn,
                &opts_sc(),
                Some(&template_style(TemplateId::Academic)),
            ),
        ),
        (
            "atelier",
            render_pdf(
                &atelier_model,
                TypstTemplate::Atelier,
                &opts_atelier(false),
                None,
            ),
        ),
        (
            "meridian",
            render_pdf(
                &resume_model,
                TypstTemplate::Meridian,
                &opts_p3a(),
                Some(&template_style(TemplateId::Meridian)),
            ),
        ),
        (
            "throughline",
            render_pdf(
                &resume_model,
                TypstTemplate::Throughline,
                &opts_p3a(),
                Some(&template_style(TemplateId::Throughline)),
            ),
        ),
        (
            "portrait-with-photo",
            render_pdf_with_photo(
                &atelier_model,
                TypstTemplate::Portrait,
                &opts_photo(false),
                Some(&template_style(TemplateId::Portrait)),
                photo_png.clone(),
            ),
        ),
        (
            "portrait-no-photo",
            render_pdf_with_photo(
                &atelier_model,
                TypstTemplate::Portrait,
                &opts_photo(false),
                Some(&template_style(TemplateId::Portrait)),
                None,
            ),
        ),
        (
            "lebenslauf-with-photo",
            render_pdf_with_photo(
                &lebenslauf_model,
                TypstTemplate::Lebenslauf,
                &opts_photo(false),
                Some(&template_style(TemplateId::Lebenslauf)),
                photo_png.clone(),
            ),
        ),
        (
            "lebenslauf-no-photo",
            render_pdf_with_photo(
                &lebenslauf_model,
                TypstTemplate::Lebenslauf,
                &opts_photo(false),
                Some(&template_style(TemplateId::Lebenslauf)),
                None,
            ),
        ),
    ];

    for (label, result) in cases {
        let bytes =
            result.unwrap_or_else(|e| panic!("stray-token guard: {label} render failed: {e:?}"));
        assert_no_stray_tokens(label, &bytes);
    }
}

#[test]
fn stray_typst_code_guard_letter() {
    let t = Template::get(TemplateId::SwissMinimal);
    let bytes = render_letter_pdf(
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
    .expect("stray-token guard: letter render failed");
    assert_no_stray_tokens("letter", &bytes);
}
