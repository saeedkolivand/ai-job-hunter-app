//! Aria (PR4 design two-column photo template) render tests.

use super::fixtures::{fixture_photo_data_url, opts_photo, placement_of, template_style};
use super::pdf_introspect::count_pdf_pages;
use super::resume_fixtures::{ATELIER_FIXTURE, ATELIER_MULTIPAGE, PLACEMENT_FIXTURE};
use crate::export::types::TemplateId;
use crate::export::typst_engine::{resolve_photo, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

#[test]
fn aria_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(aria) should succeed");
    assert!(!bytes.is_empty(), "Aria PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Aria output must start with %PDF"
    );
}

#[test]
fn aria_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(aria, no-photo) should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Aria no-photo must start with %PDF"
    );
}

#[test]
fn aria_ats_mode_drops_photo() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");

    // Non-ATS + photo → embedded as an SVG <image>.
    let shown = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        photo_png.clone(),
    )
    .expect("aria non-ats svg");
    assert!(
        shown.join("").contains("<image"),
        "non-ATS Aria with a photo must embed it as an <image> element"
    );

    // ATS + same photo → linear, no image.
    let ats = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(true),
        Some(&t),
        photo_png,
    )
    .expect("aria ats svg");
    assert!(
        !ats.join("").contains("<image"),
        "ATS-mode Aria must drop the photo (no <image> element)"
    );
}

#[test]
fn aria_ats_mode_linearizes_reading_order() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let mut model = model_from_resume_text(PLACEMENT_FIXTURE);
    // Export path linearizes for ATS; replicate it here for the reading-order
    // check. "us" resolves to the default (reverse-chronological) order.
    crate::model::transform::linearize(&mut model, "us");
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("aria ats pdf");
    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract");
    let lower: String = extracted
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    // ATS linearization (`model::transform::linearize`) reorders `data.sections`
    // to the market's canonical reading order — for the default (skills-driven)
    // market: Summary, Experience, Skills, Projects, Education, Certifications,
    // Languages, Awards, Publications — it ignores column `placement` entirely,
    // which only shapes the two-column VISUAL layout and never applies in ATS
    // mode. So the Aria placement override (Education → main column) has no
    // bearing here: the expected order is the canonical market order, not the
    // placement-projected one.
    let exp = lower.find("experience").expect("experience present");
    let skl = lower.find("skills").expect("skills present");
    let edu = lower.find("education").expect("education present");
    let cert = lower
        .find("certifications")
        .expect("certifications present");
    assert!(
        exp < skl && skl < edu && edu < cert,
        "aria ATS reading order wrong (expected default-market order: \
         experience < skills < education < certifications): {lower}"
    );
}

#[test]
fn aria_accent_override_changes_output() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Aria);

    let base = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("aria base svg")
    .join("");

    let mut accented_opts = opts_photo(false);
    accented_opts.accent = Some("#FF00AA".to_string());
    let accented = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Aria,
        &accented_opts,
        Some(&t),
        None,
    )
    .expect("aria accent svg")
    .join("");

    assert_ne!(
        base, accented,
        "a document-accent override must change Aria's rendered output"
    );
    assert!(
        accented.to_lowercase().contains("ff00aa"),
        "the accent hex should appear in Aria's SVG fills"
    );
}

#[test]
fn aria_is_two_column() {
    assert!(crate::theme::is_two_column(TemplateId::Aria));
}

#[test]
fn aria_multipage_sidebar_renders_once() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let t = template_style(TemplateId::Aria);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Aria,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(aria, multipage) should succeed");
    assert!(bytes.starts_with(b"%PDF"));
    assert!(
        count_pdf_pages(&bytes) >= 2,
        "multi-page fixture must produce ≥2 pages"
    );
    let lower = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract")
        .to_lowercase();
    assert!(
        lower.contains("grafana"),
        "sidebar skill missing\n---\n{lower}"
    );
    assert_eq!(
        lower.matches("grafana").count(),
        1,
        "Aria sidebar must render once across pages\n---\n{lower}"
    );
}

#[test]
fn aria_moves_education_to_main_column() {
    assert_eq!(
        placement_of(TemplateId::Aria, "education"),
        "main",
        "Aria: Education must be placed in the main column"
    );
    // The rest of the sidebar set is unchanged for Aria.
    assert_eq!(placement_of(TemplateId::Aria, "skills"), "sidebar");
    assert_eq!(placement_of(TemplateId::Aria, "certifications"), "sidebar");
}
