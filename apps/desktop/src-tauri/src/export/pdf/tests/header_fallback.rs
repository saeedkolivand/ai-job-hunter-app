//! candidate_name metadata is a fallback, not an override (H).

use crate::contact_profile::ContactProfile;
use crate::export::pdf::prepare_resume_render;
use crate::export::types::{
    DocumentType, ExportFormat, ExportRequest, GenerationMeta, LetterLayout, TemplateId,
};

#[test]
fn prepare_resume_render_keeps_text_derived_name_over_metadata() {
    let request = ExportRequest {
        text: "Jane Doe\njane@example.com".to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: Some(GenerationMeta {
            candidate_name: Some("Someone Else".to_string()),
            job_title: None,
            company_name: None,
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let inputs = prepare_resume_render(&request);
    assert_eq!(
        inputs.model.header.name, "Jane Doe",
        "text-derived name must win over meta.candidate_name"
    );
}

#[test]
fn prepare_resume_render_fills_name_from_metadata_when_text_has_none() {
    let request = ExportRequest {
        text: "jane@example.com\n\nSUMMARY\nSome text.".to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: Some(GenerationMeta {
            candidate_name: Some("Jane Smith".to_string()),
            job_title: None,
            company_name: None,
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let inputs = prepare_resume_render(&request);
    assert_eq!(
        inputs.model.header.name, "Jane Smith",
        "metadata must fill a header that has no text-derived name"
    );
}

/// Both export-time overrides (`meta.candidate_name` and the contact profile)
/// are fallbacks — a document whose header the user edited must render exactly
/// as written, unchanged, through the shared PDF-preview render-input builder
/// (`prepare_resume_render` backs both `generate_pdf` and `generate_preview_svg`).
#[test]
fn prepare_resume_render_keeps_edited_header_over_both_overrides() {
    let request = ExportRequest {
        text: "Alex Carter\nAlex, Berlin | alex@edited.example.com | +49 30 1111111\n\n\
               SUMMARY\nSome text."
            .to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: Some(GenerationMeta {
            candidate_name: Some("Someone Else".to_string()),
            job_title: None,
            company_name: None,
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: Some(ContactProfile {
            full_name: Some("Profile Name".to_string()),
            email: Some("profile@example.com".to_string()),
            ..Default::default()
        }),
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let inputs = prepare_resume_render(&request);
    assert_eq!(inputs.model.header.name, "Alex Carter");
    let contact_text: String = inputs
        .model
        .header
        .contact
        .iter()
        .map(|r| r.text.clone())
        .collect();
    assert!(
        contact_text.contains("alex@edited.example.com"),
        "the edited header's email must survive: {contact_text}"
    );
    assert!(
        !contact_text.contains("profile@example.com"),
        "the profile email must not override the edited header: {contact_text}"
    );
}
