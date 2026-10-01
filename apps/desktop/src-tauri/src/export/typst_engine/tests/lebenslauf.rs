//! Lebenslauf (Phase 3b-i DACH photo template) render tests.

use super::fixtures::{assert_reading_order, fixture_photo_data_url, opts_photo, template_style};
use super::resume_fixtures::{FIXTURE_RESUME, LEBENSLAUF_FIXTURE};
use crate::export::types::TemplateId;
use crate::export::typst_engine::{resolve_photo, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

// (2a) Lebenslauf with fixture photo → valid PDF.
#[test]
fn lebenslauf_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    assert!(photo_png.is_some(), "fixture photo must resolve");

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(lebenslauf) should succeed");

    assert!(!bytes.is_empty(), "Lebenslauf PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Lebenslauf output must start with %PDF"
    );
}

// (2b) Lebenslauf without photo → valid PDF.
#[test]
fn lebenslauf_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(lebenslauf, no-photo) should succeed");

    assert!(!bytes.is_empty());
    assert!(bytes.starts_with(b"%PDF"));
}

// (2c) Lebenslauf ATS harness.
#[test]
fn lebenslauf_ats_harness() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Lebenslauf);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(lebenslauf, ats) should succeed");

    assert!(bytes.starts_with(b"%PDF"));

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on lebenslauf ATS output");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Content present.
    assert!(
        lower.contains("jane doe"),
        "lebenslauf ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "lebenslauf ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "lebenslauf ATS: bullet fragment missing\n---\n{lower}"
    );

    // Word boundaries.
    assert!(
        lower.contains("state university"),
        "lebenslauf ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // Reading order.
    assert_reading_order(
        "lebenslauf",
        &lower,
        &["summary", "experience", "education", "skills"],
    );
}

// (2c-tier) ATS mode drops the Lebenslauf photo.
//
// Verifies `lebenslauf.typ`'s `#if not is-ats and has-photo` branch through the
// render path: with a real photo supplied, the non-ATS render embeds it (Typst
// emits the raster as an SVG `<image>` element) but the ATS render omits it. The
// SVG emit shares the exact world/data as the PDF path, so this is the cheapest
// reliable assertion — no PDF-internals parsing needed.
#[test]
fn lebenslauf_ats_mode_drops_photo() {
    use crate::export::typst_engine::render_resume_svg_pages_with_photo;

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let photo_png = resolve_photo(&fixture_photo_data_url());
    assert!(photo_png.is_some(), "fixture photo must resolve");

    // Non-ATS + photo → the raster photo is embedded as an SVG <image>.
    let pages_shown = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        photo_png.clone(),
    )
    .expect("render_resume_svg_pages_with_photo(lebenslauf, non-ats) should succeed");
    assert!(
        pages_shown.join("").contains("<image"),
        "non-ATS Lebenslauf with a photo must embed it as an <image> element"
    );

    // ATS + the same photo → `#if not is-ats and has-photo` is false → no image.
    let pages_ats = render_resume_svg_pages_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(true),
        Some(&t),
        photo_png,
    )
    .expect("render_resume_svg_pages_with_photo(lebenslauf, ats) should succeed");
    assert!(
        !pages_ats.join("").contains("<image"),
        "ATS-mode Lebenslauf must drop the photo (no <image> element)"
    );
}

// (2d) Write Lebenslauf sample PDFs to target/ for human review.
#[test]
fn lebenslauf_write_sample_pdfs_for_review() {
    use crate::export::typst_engine::render_pdf_with_photo;
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(LEBENSLAUF_FIXTURE);
    let t = template_style(TemplateId::Lebenslauf);
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    let _ = fs::create_dir_all(&target);

    // With photo.
    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    let bytes_with = render_pdf_with_photo(
        &model,
        TypstTemplate::Lebenslauf,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("lebenslauf with photo");
    match fs::write(target.join("lebenslauf_sample.pdf"), &bytes_with) {
        Ok(()) => {
            eprintln!("Lebenslauf (with photo) sample written to target/lebenslauf_sample.pdf")
        }
        Err(e) => eprintln!("lebenslauf_write: could not write lebenslauf_sample.pdf: {e}"),
    }
    assert!(bytes_with.starts_with(b"%PDF"));
}
