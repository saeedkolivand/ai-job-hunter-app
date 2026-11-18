//! Shared fixtures: a rich résumé and real-renderer exports of it.

use crate::export::types::{DocumentType, ExportFormat, ExportRequest, LetterLayout, TemplateId};
use crate::validate::{extract_docx_text, extract_pdf_text};

pub(super) const RICH: &str = "\
Jane Doe
Berlin, Germany | +49 151 2345 6789 | jane@example.com | linkedin.com/in/janedoe

PROFESSIONAL SUMMARY
Backend engineer with eight years of experience.

WORK EXPERIENCE
Acme Corp  Mar 2021 - Present
Senior Engineer
- Led a team of five engineers delivering the core platform

Beta GmbH  03/2018 - 02/2021
Software Engineer
- Built billing

EDUCATION
State University  2013 - 2017
BSc Computer Science

SKILLS
- Rust, TypeScript, React
";

pub(super) fn request(
    text: &str,
    format: ExportFormat,
    template_id: TemplateId,
    ats_mode: bool,
) -> ExportRequest {
    ExportRequest {
        text: text.to_string(),
        format,
        document_type: DocumentType::Resume,
        template_id,
        meta: None,
        ats_mode,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    }
}

/// Render through the real PDF / DOCX backend and return the bytes.
pub(super) fn render(r: &ExportRequest) -> Vec<u8> {
    match r.format {
        ExportFormat::Pdf => crate::export::pdf::generate_pdf(r).expect("pdf"),
        _ => crate::export::docx::generate_docx(r).expect("docx"),
    }
}

/// The text the readback re-extracts from a real render.
pub(super) fn extracted(r: &ExportRequest) -> String {
    let bytes = render(r);
    match r.format {
        ExportFormat::Pdf => extract_pdf_text(&bytes).expect("extract pdf"),
        _ => extract_docx_text(&bytes).expect("extract docx"),
    }
}
