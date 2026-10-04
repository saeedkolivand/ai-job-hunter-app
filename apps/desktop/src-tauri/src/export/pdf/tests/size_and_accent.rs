use crate::export::pdf::{generate_pdf, generate_preview_svg};
use crate::export::types::{DocumentType, ExportFormat, ExportRequest, LetterLayout, TemplateId};

/// Size guardrail: the Typst-rendered Classic resume must produce a valid,
/// non-trivially-sized PDF. Typst handles its own glyph subsetting internally;
/// the budget here is generous (5 MB) to remain stable across Typst version
/// changes while still catching a catastrophic regression (e.g. engine abort
/// producing an empty file).
#[test]
fn classic_resume_pdf_is_valid_and_within_size_budget() {
    let text = "\
Jane Doe
jane@example.com | +1 555 0100 | [LinkedIn](https://linkedin.com/in/janedoe)

EXPERIENCE
Acme Corp  2020 - Present
Senior Software Engineer
- Led a team of five engineers delivering the core billing platform
- Cut p95 API latency from 800ms to 180ms via caching and query work

SKILLS
- Rust, TypeScript, React, PostgreSQL

EDUCATION
State University  2013 - 2017
BSc Computer Science
";

    let request = ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };

    let bytes = generate_pdf(&request).expect("classic resume pdf");

    // Sanity: a real, parseable PDF.
    assert!(bytes.starts_with(b"%PDF"), "output is not a PDF");
    assert!(
        lopdf::Document::load_mem(&bytes).is_ok(),
        "Typst PDF must still parse with lopdf"
    );
    assert!(
        bytes.len() > 1_000,
        "PDF is suspiciously small ({} bytes)",
        bytes.len()
    );
    assert!(
        bytes.len() < 5_000_000,
        "PDF size budget exceeded ({} bytes > 5 MB)",
        bytes.len()
    );
}

/// Document accent (ADR 0004) threads through the résumé path: the request's
/// `accent` reaches `RenderOpts.accent` → `data.opts.accent`, which the
/// parametric single-column template prefers over its built-in palette when
/// coloring links. A custom accent must therefore change the rendered output,
/// and a malformed accent must fall back to the template palette (no change).
/// Uses the deterministic SVG preview (same world as export) and compares whole
/// documents so the check is robust to Typst's exact color serialisation.
#[test]
fn resume_document_accent_threads_into_render() {
    let base = ExportRequest {
        text: "Jane Doe\njane@example.com | [LinkedIn](https://linkedin.com/in/jane)\n\nEXPERIENCE\nAcme Corp  2020 - Present\nEngineer".to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let default_svg = generate_preview_svg(&base)
        .expect("default preview")
        .concat();

    let accented = ExportRequest {
        accent: Some("#AA0000".to_string()),
        ..base.clone()
    };
    let accented_svg = generate_preview_svg(&accented)
        .expect("accented preview")
        .concat();
    assert_ne!(
        default_svg, accented_svg,
        "a valid document accent must change the rendered résumé (link color)"
    );

    let malformed = ExportRequest {
        accent: Some("not-a-color".to_string()),
        ..base.clone()
    };
    let malformed_svg = generate_preview_svg(&malformed)
        .expect("malformed preview")
        .concat();
    assert_eq!(
        default_svg, malformed_svg,
        "a malformed accent must fall back to the template palette (no change)"
    );
}
