//! Atelier (Phase 1b) core render tests.

use super::fixtures::{opts_a4, opts_atelier, opts_photo, template_style};
use super::pdf_introspect::count_pdf_pages;
use super::resume_fixtures::{ATELIER_FIXTURE, ATELIER_MULTIPAGE, FIXTURE_RESUME};
use crate::export::templates::Template;
use crate::export::types::TemplateId;
use crate::export::typst_engine::{render_pdf, RenderOpts, TypstTemplate};
use crate::locale::PageGeometry;
use crate::model::adapter::model_from_resume_text;

// (1a) Non-ATS render produces a valid PDF.
#[test]
fn atelier_render_produces_valid_pdf() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier) should succeed");

    assert!(!bytes.is_empty(), "PDF must not be empty");
    assert!(bytes.starts_with(b"%PDF"), "output must start with %PDF");
}

// (1b) ATS render also produces a valid PDF.
#[test]
fn atelier_ats_render_produces_valid_pdf() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(true), None)
        .expect("render_pdf(atelier, ats:true) should succeed");

    assert!(!bytes.is_empty(), "ATS PDF must not be empty");
    assert!(
        bytes.starts_with(b"%PDF"),
        "ATS output must start with %PDF"
    );
}

// (2) 2-page sidebar repeat: the multi-page fixture forces ≥2 pages.
// The FULL set of sidebar items from the multipage fixture must be present
// in the extracted text — this is the regression guard for F1/F4 (dense
// sidebar overflow).  A clipped sidebar would cause these assertions to fail.
#[test]
fn atelier_multipage_sidebar_renders_once() {
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, multipage) should succeed");

    assert!(bytes.starts_with(b"%PDF"));

    // Assert ≥2 pages by counting /Type /Page objects directly in the PDF bytes.
    let page_count = count_pdf_pages(&bytes);
    assert!(
        page_count >= 2,
        "multi-page fixture must produce ≥2 pages; got {page_count}"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on our Typst PDF");

    // Normalise: collapse all whitespace (newlines, multiple spaces) to a single
    // space so line-wrapped tokens ("Eastern \nCollege") still match. Education
    // entries are now rendered as entry blocks (grid layout) which can introduce
    // line breaks inside multi-word names — normalization makes assertions robust.
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // Every sidebar skill from the fixture must be present.
    let sidebar_skills = [
        "rust",
        "go",
        "typescript",
        "kubernetes",
        "aws",
        "gcp",
        "kafka",
        "postgresql",
        "redis",
        "terraform",
        "prometheus",
        "grafana",
    ];
    for skill in &sidebar_skills {
        assert!(
            lower.contains(skill),
            "sidebar skill '{skill}' missing from extracted text — possible sidebar clip\n---\n{lower}"
        );
    }

    // Education entries in the sidebar must also be present.
    assert!(
        lower.contains("western university"),
        "sidebar education 'western university' missing\n---\n{lower}"
    );
    assert!(
        lower.contains("eastern college"),
        "sidebar education 'eastern college' missing\n---\n{lower}"
    );

    // Languages must be present.
    for lang in &["english", "portuguese", "spanish"] {
        assert!(
            lower.contains(lang),
            "sidebar language '{lang}' missing\n---\n{lower}"
        );
    }

    // The sidebar now renders ONCE (page 1 only), no longer repeated per page.
    // A sidebar-only skill ("Grafana" — never appears in a main-column bullet)
    // must therefore appear exactly once across the whole multi-page document.
    let grafana_count = lower.matches("grafana").count();
    assert_eq!(
        grafana_count, 1,
        "sidebar skill 'Grafana' must appear exactly once (sidebar renders once, \
         not repeated per page); found {grafana_count}\n---\n{lower}"
    );
}

#[test]
fn portrait_multipage_sidebar_renders_once() {
    use crate::export::typst_engine::render_pdf_with_photo;

    // Same multi-page fixture through Portrait (no photo). Portrait uses the same
    // page(background:) sidebar technique, so the page-1-only gate must hold here too.
    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let t = template_style(TemplateId::Portrait);
    let bytes = render_pdf_with_photo(
        &model,
        TypstTemplate::Portrait,
        &opts_photo(false),
        Some(&t),
        None,
    )
    .expect("render_pdf_with_photo(portrait, multipage) should succeed");
    assert!(bytes.starts_with(b"%PDF"));

    let page_count = count_pdf_pages(&bytes);
    assert!(
        page_count >= 2,
        "multi-page fixture must produce ≥2 pages; got {page_count}"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract");
    let lower = extracted.to_lowercase();
    // Sidebar content present (on page 1) …
    assert!(
        lower.contains("grafana"),
        "sidebar skill missing\n---\n{lower}"
    );
    // … and rendered exactly once, not repeated per page.
    assert_eq!(
        lower.matches("grafana").count(),
        1,
        "Portrait sidebar must render once across pages\n---\n{lower}"
    );
}

// (3) ATS collapse: ats:true → single column, linear reading order.
// Main headings (SUMMARY, EXPERIENCE) must appear before sidebar headings
// (EDUCATION, SKILLS, LANGUAGES) in the extracted text.
#[test]
fn atelier_ats_linear_reading_order() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(true), None)
        .expect("render_pdf(atelier, ats:true) should succeed");

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed");

    let lower = extracted.to_lowercase();

    // All major section headings must be present.
    for heading in &["summary", "experience", "education", "skills", "languages"] {
        assert!(
            lower.contains(heading),
            "ATS: heading '{heading}' missing from extracted text\n---\n{extracted}"
        );
    }

    // Main-column sections must appear before sidebar-column sections in the
    // extracted text (linear order = no column interleaving).
    let pos_experience = lower
        .find("experience")
        .expect("'experience' must be present");
    let pos_education = lower
        .find("education")
        .expect("'education' must be present");
    let pos_skills = lower.find("skills").expect("'skills' must be present");

    assert!(
        pos_experience < pos_education,
        "ATS: 'experience' ({pos_experience}) should precede 'education' ({pos_education}) \
         in linear order\n---\n{extracted}"
    );
    assert!(
        pos_experience < pos_skills,
        "ATS: 'experience' ({pos_experience}) should precede 'skills' ({pos_skills}) \
         in linear order\n---\n{extracted}"
    );

    // Word boundaries: "Western University" must appear with a space.
    assert!(
        lower.contains("western university"),
        "ATS: 'western university' must appear with preserved word boundary\n---\n{extracted}"
    );
}

// (4) Entry integrity: entry titles and bullet fragments must all be present.
#[test]
fn atelier_entry_integrity() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier) should succeed");

    let extracted = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract must succeed");

    let lower = extracted.to_lowercase();

    // Candidate name.
    assert!(
        lower.contains("alexandra rivera"),
        "entry integrity: candidate name missing\n---\n{extracted}"
    );

    // Entry titles.
    for title in &["meridian systems", "cobalt labs"] {
        assert!(
            lower.contains(title),
            "entry integrity: title '{title}' missing\n---\n{extracted}"
        );
    }

    // Bullet fragments.
    assert!(
        lower.contains("event-sourcing platform"),
        "entry integrity: bullet fragment 'event-sourcing platform' missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("real-time collaboration"),
        "entry integrity: bullet fragment 'real-time collaboration' missing\n---\n{extracted}"
    );
}

// (5) Custom accent override does not cause a compile error.
#[test]
fn atelier_custom_accent_succeeds() {
    let model = model_from_resume_text(ATELIER_FIXTURE);
    let opts = RenderOpts {
        page: PageGeometry {
            width_mm: 210.0,
            height_mm: 297.0,
        },
        accent: Some("#1A6B5A".to_string()), // deep teal override
        lang: "en".to_string(),
        ats: false,
    };
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts, None)
        .expect("render_pdf(atelier, custom accent) should succeed");
    assert!(bytes.starts_with(b"%PDF"));
}

// (6) Write a classic sample PDF to target/ for human review.
// This test always passes; it is informational.
// Uses .ok() so a read-only target/ directory does not fail the test run.
#[test]
fn classic_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(FIXTURE_RESUME);
    let classic = Template::get(TemplateId::Classic);
    let bytes = render_pdf(
        &model,
        TypstTemplate::SingleColumn,
        &opts_a4(),
        Some(&classic),
    )
    .expect("render_pdf(classic) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("classic_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("classic_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Classic sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "classic_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }

    assert!(bytes.starts_with(b"%PDF"));
}
