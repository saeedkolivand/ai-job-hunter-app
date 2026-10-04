//! Fixtures and helpers shared by every topic file under this module.

use super::*;

pub(super) const RESUME: &str = "\
Jane Doe
jane@example.com

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Led a team of five engineers delivering the core platform

SKILLS
- Rust, TypeScript, React

EDUCATION
State University  2013 - 2017
BSc Computer Science
";

/// Same as [`RESUME`] but with no pre-section contact line, so a contact
/// profile applied via [`req`] is the header's source of truth (H) — needed by
/// checks that specifically exercise a profile-driven header link.
pub(super) const RESUME_NO_CONTACT_LINE: &str = "\
Jane Doe

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Led a team of five engineers delivering the core platform

SKILLS
- Rust, TypeScript, React

EDUCATION
State University  2013 - 2017
BSc Computer Science
";

pub(super) fn req(format: ExportFormat, template_id: TemplateId, ats_mode: bool) -> ExportRequest {
    ExportRequest {
        text: RESUME.to_string(),
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

pub(super) fn profile_with(website: &str) -> crate::contact_profile::ContactProfile {
    crate::contact_profile::ContactProfile {
        website: Some(website.to_string()),
        ..Default::default()
    }
}

/// A single-column SwissMinimal PDF export of `text`, carrying `contact`.
pub(super) fn pdf_request(
    text: &str,
    contact: Option<crate::contact_profile::ContactProfile>,
) -> ExportRequest {
    ExportRequest {
        text: text.to_string(),
        contact,
        ..req(ExportFormat::Pdf, TemplateId::SwissMinimal, false)
    }
}

/// Run `validate_and_fix` over `request` rendered as a PDF.
pub(super) fn export_pdf(request: ExportRequest) -> (Vec<u8>, ExportReport) {
    validate_and_fix(request, crate::export::pdf::generate_pdf).expect("pdf export")
}

/// The report for `text` exported as a PDF with `contact`.
pub(super) fn pdf_report(
    text: &str,
    contact: Option<crate::contact_profile::ContactProfile>,
) -> ExportReport {
    export_pdf(pdf_request(text, contact)).1
}
