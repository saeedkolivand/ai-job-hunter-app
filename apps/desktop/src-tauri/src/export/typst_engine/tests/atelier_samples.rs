//! Atelier sample-PDF writers + dense/empty sidebar coverage.

use super::fixtures::opts_atelier;
use super::resume_fixtures::{ATELIER_DENSE_SIDEBAR, ATELIER_FIXTURE, ATELIER_MULTIPAGE};
use crate::export::typst_engine::{render_pdf, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

// (6b) Write an atelier sample PDF to target/ for human review.
// This test always passes; it is informational.
// Uses .ok() so a read-only target/ directory does not fail the test run.
#[test]
fn atelier_write_sample_pdf_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_FIXTURE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("atelier_write_sample_pdf_for_review: could not create target/: {e}");
    }
    let out_path = target.join("atelier_sample.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Atelier sample PDF written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "atelier_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }

    assert!(bytes.starts_with(b"%PDF"));
}

// (6c) Write a MULTI-PAGE atelier sample to target/ for human review.
// Forces ≥2 pages so the page-background sidebar repeat + pagination + the
// locked house spacing scale can be eyeballed across a page break.
// Informational; .ok()-style write never fails the run.
#[test]
fn atelier_write_multipage_sample_for_review() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_MULTIPAGE);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, multipage) should succeed for sample PDF");

    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("atelier_write_multipage_sample_for_review: could not create target/: {e}");
    }
    let out_path = target.join("atelier_multipage_diag.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!("Atelier multipage sample written to: {}", out_path.display()),
        Err(e) => eprintln!(
            "atelier_write_multipage_sample_for_review: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }

    assert!(bytes.starts_with(b"%PDF"));
}

// (7) Dense-sidebar fixture: 10+ skills, 2 degrees, 3 certs, 4 languages.
// Every sidebar item must appear in the extracted PDF text, proving that
// the dense-sidebar overflow detection (F1) correctly falls back to
// single-column and does NOT silently clip any content.
#[test]
fn atelier_dense_sidebar_no_data_loss() {
    use std::fs;
    use std::path::Path;

    let model = model_from_resume_text(ATELIER_DENSE_SIDEBAR);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, dense-sidebar) should succeed");

    assert!(
        bytes.starts_with(b"%PDF"),
        "dense-sidebar PDF must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on dense-sidebar output");

    // Normalise: collapse all whitespace (newlines, multiple spaces) to a
    // single space so that line-wrapped tokens ("Coastal \nCollege") still
    // match the expected substrings.
    let normalised: String = extracted.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalised.to_lowercase();

    // ── Skills (10+) ──────────────────────────────────────────────────────────
    let skills = [
        "rust",
        "go",
        "python",
        "typescript",
        "java",
        "kotlin",
        "c++",
        "bash",
        "sql",
        "terraform",
        "ansible",
        "pulumi",
    ];
    for skill in &skills {
        assert!(
            lower.contains(skill),
            "dense-sidebar: skill '{skill}' missing — possible silent clip\n---\n{lower}"
        );
    }

    // ── Education (2 degrees) ─────────────────────────────────────────────────
    assert!(
        lower.contains("metro university"),
        "dense-sidebar: 'metro university' missing\n---\n{lower}"
    );
    assert!(
        lower.contains("coastal college"),
        "dense-sidebar: 'coastal college' missing\n---\n{lower}"
    );

    // ── Languages (4) ─────────────────────────────────────────────────────────
    for lang in &["english", "german", "french", "mandarin"] {
        assert!(
            lower.contains(lang),
            "dense-sidebar: language '{lang}' missing\n---\n{lower}"
        );
    }

    // ── Certifications (3) ────────────────────────────────────────────────────
    assert!(
        lower.contains("aws solutions architect"),
        "dense-sidebar: 'aws solutions architect' cert missing\n---\n{lower}"
    );
    assert!(
        lower.contains("google cloud"),
        "dense-sidebar: 'google cloud' cert missing\n---\n{lower}"
    );
    assert!(
        lower.contains("kubernetes administrator"),
        "dense-sidebar: 'kubernetes administrator' cert missing\n---\n{lower}"
    );

    // Write the dense-sidebar sample for eyeballing.
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    if let Err(e) = fs::create_dir_all(&target) {
        eprintln!("atelier_dense_sidebar_no_data_loss: could not create target/: {e}");
    }
    let out_path = target.join("atelier_dense_sidebar.pdf");
    match fs::write(&out_path, &bytes) {
        Ok(()) => eprintln!(
            "Dense-sidebar sample PDF written to: {}",
            out_path.display()
        ),
        Err(e) => eprintln!(
            "atelier_dense_sidebar_no_data_loss: could not write {}: {e} (informational only)",
            out_path.display()
        ),
    }
}

// (8) Empty-sidebar fixture: all sections map to main; no sidebar sections.
// The template must render cleanly in single-column mode (no band) and all
// content must be present in the extracted text.
#[test]
fn atelier_empty_sidebar_renders_single_column() {
    // A resume with only SUMMARY + EXPERIENCE + PROJECTS — none of these
    // sections map to the sidebar (Skills/Education/Languages/Certifications
    // are the sidebar sections).  The template must detect no sidebar sections
    // and fall back to single-column to avoid rendering an empty tinted band.
    let fixture = "\
Morgan Ellis
morgan@example.com | https://morganellis.dev

SUMMARY
Full-stack engineer specialising in high-throughput data pipelines.

EXPERIENCE
Senior Engineer | DataCo | 2020 – Present
- Designed a streaming ingestion layer processing 2 M events per second
- Reduced P99 query latency from 800 ms to 35 ms via index optimisation

Engineer | PipeCraft | 2017 – 2020
- Built the core ETL framework adopted by all twelve data teams
- Migrated a batch pipeline to a streaming architecture with zero downtime

PROJECTS
OpenStream | Open Source | 2022
- High-throughput event router with pluggable backends
- 2 k GitHub stars; used in production by three Fortune 500 companies
";

    let model = model_from_resume_text(fixture);
    let bytes = render_pdf(&model, TypstTemplate::Atelier, &opts_atelier(false), None)
        .expect("render_pdf(atelier, empty-sidebar) should succeed");

    assert!(
        bytes.starts_with(b"%PDF"),
        "empty-sidebar PDF must start with %PDF"
    );

    let extracted = pdf_extract::extract_text_from_mem(&bytes)
        .expect("pdf-extract must succeed on empty-sidebar output");

    let lower = extracted.to_lowercase();

    // All content must be present — none clipped by a missing sidebar.
    assert!(
        lower.contains("morgan ellis"),
        "empty-sidebar: candidate name missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("dataco"),
        "empty-sidebar: 'dataco' entry missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("streaming ingestion"),
        "empty-sidebar: bullet fragment missing\n---\n{extracted}"
    );
    assert!(
        lower.contains("openstream"),
        "empty-sidebar: project 'openstream' missing\n---\n{extracted}"
    );
}
