//! Meridian and Throughline (Phase 3a premium single-column) render tests.

use super::fixtures::{opts_p3a, template_style};
use super::resume_fixtures::FIXTURE_RESUME;
use crate::export::types::TemplateId;
use crate::export::typst_engine::{render_pdf, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

#[test]
fn meridian_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Meridian);
    let bytes = render_pdf(&model, TypstTemplate::Meridian, &opts_p3a(), Some(&t))
        .expect("render_pdf(meridian) should succeed");
    assert!(!bytes.is_empty(), "Meridian PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Meridian output must start with %PDF"
    );
}

#[test]
fn meridian_ats_harness() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Meridian);
    let bytes = render_pdf(&model, TypstTemplate::Meridian, &opts_p3a(), Some(&t))
        .expect("render_pdf(meridian) for ATS harness");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on meridian output");

    // Normalise whitespace — band layout can introduce line breaks inside
    // the header content (name, contact line placed in page background).
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // (c) Content present.
    assert!(
        lower.contains("jane doe"),
        "meridian ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "meridian ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "meridian ATS: bullet fragment 'distributed task scheduler' missing\n---\n{lower}"
    );

    // (b) Word boundaries.
    assert!(
        lower.contains("state university"),
        "meridian ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // (a) Reading order: summary → experience → education → skills.
    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("meridian ATS: '{h}' not found in extracted text"));
        assert!(
            pos >= last,
            "meridian ATS: '{h}' ({pos}) appeared before previous heading ({last})\n---\n{lower}"
        );
        last = pos;
    }
}

#[test]
fn meridian_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Meridian);
    let bytes = render_pdf(&model, TypstTemplate::Meridian, &opts_p3a(), Some(&t))
        .expect("render_pdf(meridian) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("meridian_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("meridian_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Meridian sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "meridian_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn throughline_render_produces_valid_pdf() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) should succeed");
    assert!(!bytes.is_empty(), "Throughline PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "Throughline output must start with %PDF"
    );
}

#[test]
fn throughline_ats_harness() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) for ATS harness");

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on throughline output");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // (c) Content present.
    assert!(
        lower.contains("jane doe"),
        "throughline ATS: 'jane doe' missing\n---\n{lower}"
    );
    for heading in &["summary", "experience", "education", "skills"] {
        assert!(
            lower.contains(heading),
            "throughline ATS: heading '{heading}' missing\n---\n{lower}"
        );
    }
    assert!(
        lower.contains("distributed task scheduler"),
        "throughline ATS: bullet fragment missing\n---\n{lower}"
    );

    // (b) Word boundaries.
    assert!(
        lower.contains("state university"),
        "throughline ATS: 'state university' word boundary broken\n---\n{lower}"
    );

    // (a) Reading order.
    let order = ["summary", "experience", "education", "skills"];
    let mut last = 0usize;
    for h in &order {
        let pos = lower
            .find(h)
            .unwrap_or_else(|| panic!("throughline ATS: '{h}' not found"));
        assert!(
            pos >= last,
            "throughline ATS: '{h}' ({pos}) appeared before previous ({last})\n---\n{lower}"
        );
        last = pos;
    }
}

// (d) Throughline-specific: EXPERIENCE entries + bullets must all survive
// text extraction — the timeline decoration (nodes/spine) must not drop content.
#[test]
fn throughline_timeline_entries_and_bullets_survive_extraction() {
    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) for timeline integrity");

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed");

    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Entry titles from the fixture's EXPERIENCE section.
    for title in &["acme corp", "beta inc"] {
        assert!(
            lower.contains(title),
            "throughline timeline: entry title '{title}' missing — timeline may have dropped text\n---\n{lower}"
        );
    }

    // Bullet fragments from EXPERIENCE entries.
    assert!(
        lower.contains("distributed task scheduler"),
        "throughline timeline: bullet 'distributed task scheduler' missing\n---\n{lower}"
    );
    assert!(
        lower.contains("real-time data pipeline"),
        "throughline timeline: bullet 'real-time data pipeline' missing\n---\n{lower}"
    );
}

#[test]
fn throughline_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let t = template_style(TemplateId::Throughline);
    let bytes = render_pdf(&model, TypstTemplate::Throughline, &opts_p3a(), Some(&t))
        .expect("render_pdf(throughline) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("throughline_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("throughline_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Throughline sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "throughline_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
    assert!(bytes.starts_with(b"%PDF"));
}
