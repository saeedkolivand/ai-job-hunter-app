use crate::export::pdf::generate_pdf;
use crate::export::types::{DocumentType, ExportFormat, ExportRequest, LetterLayout, TemplateId};

/// Dev tool (ignored): write sample resume PDFs for every template to
/// `target/sample_pdfs/` for visual inspection. Run with:
///
/// ```text
/// cargo test -p ajh-tauri -- --ignored dump_sample_resume_pdfs --nocapture
/// ```
///
/// Output files: `<template>_legacy.pdf` / `<template>_engine.pdf` (+ an ATS pair
/// for the two-column template) under `apps/desktop/src-tauri/target/sample_pdfs/`.
#[test]
#[ignore = "dev tool: writes sample PDFs to target/sample_pdfs for visual review"]
fn dump_sample_resume_pdfs() {
    use std::fs;
    use std::path::Path;

    const SAMPLE: &str = "\
Jane Doe
jane@example.com | +1 555 0100 | [LinkedIn](https://linkedin.com/in/janedoe) | https://janedoe.dev

Senior software engineer with a decade building reliable, user-facing web
applications end to end across startups and scale-ups.

EXPERIENCE
Acme Corp  2020 - Present
Senior Software Engineer
- Led a team of five engineers delivering the core billing platform
- Shipped three major features that grew activation by 24%
- Cut p95 API latency from 800ms to 180ms via caching and query work

Globex Inc  2017 - 2020
Software Engineer
- Built the public REST API now serving two million requests per day
- Mentored four junior engineers through onboarding

SKILLS
- Rust, TypeScript, React, PostgreSQL
- AWS, Docker, Kubernetes, CI/CD

EDUCATION
State University  2013 - 2017
BSc Computer Science

LANGUAGES
- English (native), Spanish (professional)
";

    let templates = [
        TemplateId::Classic,
        TemplateId::SwissMinimal,
        TemplateId::Academic,
        TemplateId::Atelier,
        TemplateId::Meridian,
        TemplateId::Throughline,
        TemplateId::Portrait,
        TemplateId::Lebenslauf,
        TemplateId::Cadence,
        TemplateId::Regent,
        TemplateId::Aria,
        TemplateId::Saffron,
    ];

    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/sample_pdfs");
    fs::create_dir_all(&out).expect("create target/sample_pdfs");

    for id in templates {
        let request = ExportRequest {
            text: SAMPLE.to_string(),
            format: ExportFormat::Pdf,
            document_type: DocumentType::Resume,
            template_id: id,
            meta: None,
            ats_mode: false,
            locale: None,
            contact: None,
            accent: None,
            letter_layout: LetterLayout::Classic,
        };
        let slug = format!("{id:?}").to_lowercase();
        let bytes = generate_pdf(&request).expect("typst pdf");
        fs::write(out.join(format!("{slug}_typst.pdf")), &bytes).expect("write pdf");
    }

    eprintln!("wrote sample PDFs to apps/desktop/src-tauri/target/sample_pdfs/");
}
