use super::support::collect_uris;
use crate::export::pdf::generate_pdf;
use crate::export::types::{DocumentType, ExportFormat, ExportRequest, LetterLayout, TemplateId};

#[test]
fn resume_pdf_embeds_contact_link_annotations() {
    // A real resume export must emit clickable annotations for the contact line's
    // links (markdown link → its URL; bare email → mailto:). End-to-end check that
    // the exact-metrics rects are wired through, not just unit-tested in isolation.
    let text = "Jane Doe\njane@example.com | [LinkedIn](https://linkedin.com/in/jane)";
    let request = ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let bytes = generate_pdf(&request).expect("resume pdf");
    let doc = lopdf::Document::load_mem(&bytes).expect("parse generated pdf");
    let uris = collect_uris(&doc);

    assert!(
        uris.iter().any(|u| u == "https://linkedin.com/in/jane"),
        "expected LinkedIn link annotation, found {uris:?}"
    );
    assert!(
        uris.iter().any(|u| u == "mailto:jane@example.com"),
        "expected mailto annotation for the email, found {uris:?}"
    );
}

#[test]
fn single_column_template_resume_pdf_is_generated() {
    // Exercise the parametric single-column generate_pdf path end-to-end.
    let request = ExportRequest {
        text: "Alexander Hamilton\nalex@example.com\n\nEXPERIENCE\nTreasury  2020 - Present\nSecretary"
            .to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let bytes = generate_pdf(&request).expect("modern resume pdf");
    assert!(
        bytes.len() > 1000,
        "expected a non-trivial PDF, got {} bytes",
        bytes.len()
    );
}
