use super::support::{collect_link_rects, long_contact_profile};
use crate::export::pdf::generate_pdf;
use crate::export::types::{
    DocumentType, ExportFormat, ExportRequest, GenerationMeta, LetterLayout, TemplateId,
};

#[test]
fn cover_letter_does_not_leak_generated_contact_line() {
    // The generated letter still carries its own contact line (with a markdown
    // link). With a contact profile present, the letterhead renders the profile and
    // the text's contact line must be dropped — never leaked into the body as raw
    // markdown (the `[Dribbble](…` / truncation symptom).
    let text = "Lena Vos\n\
        Amsterdam, Netherlands | l@example.com | +31 6 12345678 | [LinkedIn](https://linkedin.com/in/l) | [Dribbble](https://dribbble.com/lenavos)\n\
        31. Mai 2026\n\
        JAKALA\n\
        Hiring Team\n\
        Sehr geehrtes JAKALA-Team,\n\n\
        Mit mehr als vier Jahren Erfahrung bringe ich die Faehigkeit mit.\n\n\
        Mit freundlichen Gruessen,\n\
        Lena Vos";
    let request = ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::SwissMinimal,
        meta: Some(GenerationMeta {
            candidate_name: Some("Lena Vos".to_string()),
            job_title: None,
            company_name: None,
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: Some(crate::contact_profile::ContactProfile {
            website: Some("https://drive.google.com/file/d/abc/view".to_string()),
            ..Default::default()
        }),
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let bytes = generate_pdf(&request).expect("cover letter pdf");
    let rendered = pdf_extract::extract_text_from_mem(&bytes).expect("extract text");

    assert!(
        !rendered.contains("[Dribbble]") && !rendered.to_lowercase().contains("dribbble.com"),
        "the generated contact line leaked into the body: {rendered}"
    );
    assert!(
        rendered.contains("Sehr geehrtes") && rendered.contains("Mit mehr als"),
        "the letter body must still render: {rendered}"
    );
}

#[test]
fn resume_long_contact_line_wraps_within_page() {
    let request = ExportRequest {
        text: "Lena Vos\n\nEXPERIENCE\nAcme  2020 - Present\nDesigner\n- Did work".to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::SwissMinimal,
        meta: Some(GenerationMeta {
            candidate_name: Some("Lena Vos".to_string()),
            job_title: None,
            company_name: None,
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: Some(long_contact_profile()),
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let bytes = generate_pdf(&request).expect("resume pdf");
    let doc = lopdf::Document::load_mem(&bytes).expect("parse pdf");
    let rects = collect_link_rects(&doc);
    assert!(!rects.is_empty(), "expected header link annotations");

    let page_w_pt = request.page_geometry().width_mm * 2.834_645_7;
    for (r, uri) in &rects {
        let right = r[0].max(r[2]);
        assert!(
            right <= page_w_pt + 1.0,
            "link {uri} overflows the page (right={right}, page={page_w_pt})"
        );
    }
    // Wrapping happened → header links sit on ≥2 distinct baselines.
    let mut ys: Vec<i64> = rects
        .iter()
        .map(|(r, _)| r[1].min(r[3]).round() as i64)
        .collect();
    ys.sort_unstable();
    ys.dedup();
    assert!(
        ys.len() >= 2,
        "a long contact line must wrap onto multiple lines, baselines={ys:?}"
    );
}

#[test]
fn cover_letter_long_contact_line_wraps_within_page() {
    let request = ExportRequest {
        text: "Sehr geehrtes Team,\n\nIch bewerbe mich.\n\nMit freundlichen Gruessen,\nLena Vos"
            .to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::SwissMinimal,
        meta: Some(GenerationMeta {
            candidate_name: Some("Lena Vos".to_string()),
            job_title: None,
            company_name: None,
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: Some(long_contact_profile()),
        accent: None,
        letter_layout: LetterLayout::Classic,
    };
    let bytes = generate_pdf(&request).expect("cover letter pdf");
    let doc = lopdf::Document::load_mem(&bytes).expect("parse pdf");
    let rects = collect_link_rects(&doc);
    assert!(!rects.is_empty(), "expected letterhead link annotations");

    let page_w_pt = request.page_geometry().width_mm * 2.834_645_7;
    for (r, uri) in &rects {
        let right = r[0].max(r[2]);
        assert!(
            right <= page_w_pt + 1.0,
            "letterhead link {uri} overflows the page (right={right}, page={page_w_pt})"
        );
    }
    let mut ys: Vec<i64> = rects
        .iter()
        .map(|(r, _)| r[1].min(r[3]).round() as i64)
        .collect();
    ys.sort_unstable();
    ys.dedup();
    assert!(
        ys.len() >= 2,
        "a long letterhead contact line must wrap, baselines={ys:?}"
    );
}
