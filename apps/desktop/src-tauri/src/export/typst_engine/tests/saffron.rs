//! Saffron (PR4 design two-column photo template) render tests.

use super::fixtures::{fixture_photo_data_url, opts_photo, placement_of, template_style};
use super::pdf_introspect::count_pdf_pages;
use super::resume_fixtures::{ATELIER_FIXTURE, ATELIER_MULTIPAGE, PLACEMENT_FIXTURE};
use crate::export::types::TemplateId;
use crate::export::typst_engine::{resolve_photo, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

#[test]
fn saffron_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(saffron) should succeed");
    assert!(!bytes.is_empty(), "Saffron PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Saffron output must start with %PDF"
    );
}

#[test]
fn saffron_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(saffron, no-photo) should succeed");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Saffron no-photo must start with %PDF"
    );
}

// Saffron's no-photo monogram fallback shares Portrait's slicing logic (copied
// pattern) — same grapheme-safety pin as
// `portrait_no_photo_monogram_is_grapheme_safe_for_multibyte_names`.
#[test]
fn saffron_no_photo_monogram_is_grapheme_safe_for_multibyte_names() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let text = "Über Ödegaard\nuber@example.com\n\nSUMMARY\nEngineer.\n";
    let model = model_from_resume_text(text);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("saffron no-photo render with a multi-byte first character must not panic");
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn saffron_ats_mode_drops_photo() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");

    let shown = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        photo_png.clone(),
    )
    .expect("saffron non-ats svg");
    assert!(
        shown.join("").contains("<image"),
        "non-ATS Saffron with a photo must embed it as an <image> element"
    );

    let ats = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(true),
        Some(&t),
        photo_png,
    )
    .expect("saffron ats svg");
    assert!(
        !ats.join("").contains("<image"),
        "ATS-mode Saffron must drop the photo (no <image> element)"
    );
}

#[test]
fn saffron_ats_mode_linearizes_reading_order() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let mut model = model_from_resume_text(PLACEMENT_FIXTURE);
    // "us" resolves to the default (skills-driven) market order.
    crate::model::transform::linearize(&mut model, "us");
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("saffron ats pdf");
    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract");
    let lower: String = extracted
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    // Same semantics as `aria_ats_mode_linearizes_reading_order`: ATS mode uses
    // the default-market canonical order (Summary, Experience, Skills, Projects,
    // Education, Certifications, …) regardless of Saffron's placement override
    // (Certifications → main column), which is visual-only and never applies in
    // ATS mode. Include `education` so the expected order isn't coincidentally
    // satisfied by only checking two of the four sections.
    let exp = lower.find("experience").expect("experience present");
    let skl = lower.find("skills").expect("skills present");
    let edu = lower.find("education").expect("education present");
    let cert = lower
        .find("certifications")
        .expect("certifications present");
    assert!(
        exp < skl && skl < edu && edu < cert,
        "saffron ATS reading order wrong (expected default-market order: \
         experience < skills < education < certifications): {lower}"
    );
}

#[test]
fn saffron_accent_override_changes_output() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Saffron);

    let base = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("saffron base svg")
    .join("");

    let mut accented_opts = opts_photo(false);
    accented_opts.accent = Some("#FF00AA".to_string());
    let accented = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Saffron,
        &accented_opts,
        Some(&t),
        None,
    )
    .expect("saffron accent svg")
    .join("");

    assert_ne!(
        base, accented,
        "a document-accent override must change Saffron's rendered output"
    );
    assert!(
        accented.to_lowercase().contains("ff00aa"),
        "the accent hex should appear in Saffron's SVG fills"
    );
}

#[test]
fn saffron_is_two_column() {
    assert!(crate::theme::is_two_column(TemplateId::Saffron));
}

#[test]
fn saffron_multipage_sidebar_renders_once() {
    use crate::export::typst_engine::render_pdf_with_photo;
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let t = template_style(TemplateId::Saffron);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Saffron,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(saffron, multipage) should succeed");
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
        "Saffron sidebar must render once across pages\n---\n{lower}"
    );
}

#[test]
fn saffron_moves_certifications_to_main_column() {
    assert_eq!(
        placement_of(TemplateId::Saffron, "certifications"),
        "main",
        "Saffron: Certifications must be placed in the main column"
    );
    // Education stays in the sidebar for Saffron (unlike Aria).
    assert_eq!(placement_of(TemplateId::Saffron, "education"), "sidebar");
    assert_eq!(placement_of(TemplateId::Saffron, "skills"), "sidebar");
}

#[test]
fn portrait_placement_is_unchanged_by_the_refactor() {
    // Control: the default table (Portrait) keeps Education + Certifications in
    // the sidebar — the per-template id parameter must not shift it.
    assert_eq!(placement_of(TemplateId::Portrait, "education"), "sidebar");
    assert_eq!(
        placement_of(TemplateId::Portrait, "certifications"),
        "sidebar"
    );
}
