//! SwissMinimal and Academic (Phase 2 SingleColumn parametric) render tests.
//!
//! SwissMinimal and Academic differ only by `TemplateId` and their message labels — every
//! assertion and panic message below is byte-identical to the two templates' former separate
//! `swiss_minimal_*`/`academic_*` tests (issue #1280 batch 5a merge; see each fn's doc comment
//! for the old names).

use super::fixtures::{assert_reading_order, opts_sc, template_style};
use super::resume_fixtures::FIXTURE_RESUME;
use crate::export::types::TemplateId;
use crate::export::typst_engine::{render_pdf, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

/// `(TemplateId, kebab-case ATS-message label, Title Case sample-PDF label)`.
fn cases() -> [(TemplateId, &'static str, &'static str); 2] {
    [
        (TemplateId::SwissMinimal, "swiss-minimal", "Swiss Minimal"),
        (TemplateId::Academic, "academic", "Academic"),
    ]
}

/// Was `swiss_minimal_render_produces_valid_pdf` + `academic_render_produces_valid_pdf`.
#[test]
fn swiss_minimal_and_academic_render_produce_valid_pdf() {
    for (id, label, title) in cases() {
        let model = model_from_resume_text(FIXTURE_RESUME);
        let t = template_style(id);
        let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
            .unwrap_or_else(|e| panic!("render_pdf({label}) should succeed: {e:?}"));
        assert!(!bytes.is_empty(), "{title} PDF must not be empty");
        assert!(
            bytes.starts_with(b"%PDF"),
            "{title} output must start with %PDF"
        );
    }
}

/// Was `swiss_minimal_ats_harness` + `academic_ats_harness`.
#[test]
fn swiss_minimal_and_academic_ats_harness() {
    for (id, label, _title) in cases() {
        let model = model_from_resume_text(FIXTURE_RESUME);
        let t = template_style(id);
        let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
            .unwrap_or_else(|e| {
                panic!("render_pdf({label}) for ATS harness should succeed: {e:?}")
            });

        let extracted = pdf_extract::extract_text_from_mem(&bytes)
            .unwrap_or_else(|e| panic!("pdf-extract must succeed on {label} output: {e:?}"));
        let lower = extracted.to_lowercase();

        assert!(
            lower.contains("jane doe"),
            "{label} ATS: 'jane doe' missing\n---\n{extracted}"
        );
        for heading in &["summary", "experience", "education", "skills"] {
            assert!(
                lower.contains(heading),
                "{label} ATS: heading '{heading}' missing\n---\n{extracted}"
            );
        }
        assert!(
            lower.contains("distributed task scheduler"),
            "{label} ATS: bullet fragment missing\n---\n{extracted}"
        );
        assert!(
            lower.contains("state university"),
            "{label} ATS: 'state university' word boundary broken\n---\n{extracted}"
        );

        assert_reading_order(
            label,
            &lower,
            &["summary", "experience", "education", "skills"],
        );
    }
}

/// Was `swiss_minimal_write_sample_pdf_for_review` + `academic_write_sample_pdf_for_review`.
/// Always passes; informational sample-PDF writer.
#[test]
fn swiss_minimal_and_academic_write_sample_pdfs_for_review() {
    use std::fs;
    use std::path::Path;

    for (id, slug, title) in [
        (TemplateId::SwissMinimal, "swiss_minimal", "Swiss Minimal"),
        (TemplateId::Academic, "academic", "Academic"),
    ] {
        let model = model_from_resume_text(FIXTURE_RESUME);
        let t = template_style(id);
        let bytes = render_pdf(&model, TypstTemplate::SingleColumn, &opts_sc(), Some(&t))
            .unwrap_or_else(|e| panic!("render_pdf({slug}) should succeed for sample PDF: {e:?}"));

        let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        if let Err(e) = fs::create_dir_all(&target) {
            eprintln!("{slug}_write_sample_pdf_for_review: could not create target/: {e}");
        }
        let out_path = target.join(format!("{slug}_sample.pdf"));
        match fs::write(&out_path, &bytes) {
            Ok(()) => eprintln!("{title} sample PDF written to: {}", out_path.display()),
            Err(e) => eprintln!(
                "{slug}_write_sample_pdf_for_review: could not write {}: {e} (informational only)",
                out_path.display()
            ),
        }
        assert!(bytes.starts_with(b"%PDF"));
    }
}
