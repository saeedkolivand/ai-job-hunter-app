//! Portrait (Phase 3b-i photo template) render tests.

use super::fixtures::{assert_reading_order, fixture_photo_data_url, opts_photo, template_style};
use super::resume_fixtures::{ATELIER_FIXTURE, FIXTURE_RESUME};
use crate::export::types::TemplateId;
use crate::export::typst_engine::{resolve_photo, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

// (1a) Portrait with fixture photo → valid PDF.
#[test]
fn portrait_render_with_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    assert!(photo_png.is_some(), "fixture photo must resolve");

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("render_pdf_with_photo(portrait) should succeed");

    assert!(!bytes.is_empty(), "Portrait PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Portrait output must start with %PDF"
    );
}

// (1b) Portrait without photo (no-photo fallback) → valid PDF.
#[test]
fn portrait_render_no_photo_produces_valid_pdf() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(portrait, no-photo) should succeed");

    assert!(!bytes.is_empty(), "Portrait no-photo PDF must not be empty");
    assert!(bytes.starts_with(b"%PDF"));
}

// (1b-multibyte) Portrait's no-photo monogram fallback slices the candidate's
// first name to build initials. A byte-offset `.slice(0, 1)` panics Typst
// whenever the first character is multi-byte in UTF-8 (plausible DACH/EU
// names) — this pins the grapheme-safe fix (`.clusters().first()`).
#[test]
fn portrait_no_photo_monogram_is_grapheme_safe_for_multibyte_names() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let text = "Über Ödegaard\nuber@example.com\n\nSUMMARY\nEngineer.\n";
    let model = model_from_resume_text(text);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("portrait no-photo render with a multi-byte first character must not panic");
    assert!(bytes.starts_with(b"%PDF"));
}

// (1c) Portrait ATS mode → valid PDF with linear reading order.
#[test]
fn portrait_ats_harness() {
    use crate::export::typst_engine::render_pdf_with_photo;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(true),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(portrait, ats) should succeed");

    assert!(bytes.starts_with(b"%PDF"));

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on portrait ATS output");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Content present.
    assert!(
        lower.contains("jane doe"),
        "portrait ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "portrait ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "portrait ATS: bullet fragment missing\n---\n{lower}"
    );

    // Word boundaries.
    assert!(
        lower.contains("state university"),
        "portrait ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // Reading order.
    assert_reading_order(
        "portrait",
        &lower,
        &["summary", "experience", "education", "skills"],
    );
}

// (1d) Write Portrait sample PDFs to target/ for human review (with and without photo).
#[test]
fn portrait_write_sample_pdfs_for_review() {
    use crate::export::typst_engine::render_pdf_with_photo;
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let t = template_style(TemplateId::Portrait);
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    let _ = fs::create_dir_all(&target);

    // With photo.
    let data_url = fixture_photo_data_url();
    let photo_png = resolve_photo(&data_url);
    let bytes_with = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        photo_png,
    )
    .expect("portrait with photo");
    match fs::write(target.join("portrait_sample.pdf"), &bytes_with) {
        Ok(()) => eprintln!("Portrait (with photo) sample written to target/portrait_sample.pdf"),
        Err(e) => eprintln!("portrait_write: could not write portrait_sample.pdf: {e}"),
    }
    assert!(bytes_with.starts_with(b"%PDF"));

    // Without photo (no-photo fallback).
    let bytes_nophoto = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("portrait no-photo");
    match fs::write(target.join("portrait_nophoto_sample.pdf"), &bytes_nophoto) {
        Ok(()) => {
            eprintln!("Portrait (no-photo) sample written to target/portrait_nophoto_sample.pdf")
        }
        Err(e) => eprintln!("portrait_write: could not write portrait_nophoto_sample.pdf: {e}"),
    }
    assert!(bytes_nophoto.starts_with(b"%PDF"));
}
