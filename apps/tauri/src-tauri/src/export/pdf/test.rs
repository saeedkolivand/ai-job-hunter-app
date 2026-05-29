use super::*;

#[test]
fn test_extract_section_with_both_markers() {
    let text = "Header\n### START ###\nContent here\n### END ###\nFooter";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    assert_eq!(result, "Content here");
}

#[test]
fn test_extract_section_no_start_marker() {
    let text = "Content here\n### END ###\nFooter";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    assert_eq!(result, "Content here\n### END ###\nFooter");
}

#[test]
fn test_extract_section_no_end_marker() {
    let text = "Header\n### START ###\nContent here\nMore content";
    let result = extract_section(text, "### START ###", None);
    assert_eq!(result, "Content here\nMore content");
}

#[test]
fn test_extract_section_empty_markers() {
    let text = "Just some text";
    let result = extract_section(text, "NONEXISTENT", None);
    assert_eq!(result, "Just some text");
}

#[test]
fn test_extract_section_with_newline_after_marker() {
    let text = "Header\n### START ###\n\nContent here\n### END ###";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    assert_eq!(result, "Content here");
}

#[test]
fn test_extract_section_multiple_occurrences() {
    let text = "Header\n### START ###\nFirst\n### START ###\nSecond\n### END ###";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    // Should extract from first occurrence to end marker
    assert!(result.contains("First"));
}

#[test]
fn test_extract_section_end_marker_before_start() {
    let text = "### END ###\nHeader\n### START ###\nContent";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    // Should return everything after start since end is before it
    assert_eq!(result, "Content");
}

#[test]
fn test_extract_section_same_marker() {
    let text = "Header\n### MARKER ###\nContent\n### MARKER ###\nFooter";
    let result = extract_section(text, "### MARKER ###", Some("### MARKER ###"));
    assert_eq!(result, "Content");
}

#[test]
fn test_extract_section_whitespace_handling() {
    let text = "Header\n### START ###\n   Content with spaces   \n### END ###";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    assert_eq!(result, "Content with spaces");
}

#[test]
fn test_extract_section_multiline_content() {
    let text = "Header\n### START ###\nLine 1\nLine 2\nLine 3\n### END ###\nFooter";
    let result = extract_section(text, "### START ###", Some("### END ###"));
    assert_eq!(result, "Line 1\nLine 2\nLine 3");
}

#[test]
fn test_generate_pdf_resume_basic() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com".to_string(),
        format: super::super::types::ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: super::super::types::TemplateId::Classic,
        meta: None,
        ats_mode: false,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_cover_letter_basic() {
    let request = ExportRequest {
        text: "Dear Hiring Manager,\n\nI am writing to apply...".to_string(),
        format: super::super::types::ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: super::super::types::TemplateId::Modern,
        meta: None,
        ats_mode: false,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_resume_with_meta() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com".to_string(),
        format: super::super::types::ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: super::super::types::TemplateId::Executive,
        meta: Some(GenerationMeta {
            candidate_name: Some("Jane Smith".to_string()),
            job_title: Some("Software Engineer".to_string()),
            company_name: Some("Test Corp".to_string()),
            target_language: None,
        }),
        ats_mode: false,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_resume_with_section_markers() {
    let text = "### CANDIDATE RESUME ###\nJohn Doe\njohn@example.com\n### JOB ADVERTISEMENT ###\nJob description here";
    let request = ExportRequest {
        text: text.to_string(),
        format: super::super::types::ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: super::super::types::TemplateId::Classic,
        meta: None,
        ats_mode: false,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_cover_letter_with_section_markers() {
    let text =
        "Some header\n### COMPLETE COVER LETTER ###\nDear Hiring Manager,\n\nI am writing...";
    let request = ExportRequest {
        text: text.to_string(),
        format: super::super::types::ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: super::super::types::TemplateId::Modern,
        meta: None,
        ats_mode: false,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

/// Recursively collect every `/URI` action target in a parsed PDF (link
/// annotations store the URL nested under the annotation's `/A` action dict).
fn collect_uris(doc: &lopdf::Document) -> Vec<String> {
    fn from_dict(d: &lopdf::Dictionary, out: &mut Vec<String>) {
        if let Ok(u) = d.get(b"URI") {
            if let Ok(bytes) = u.as_str() {
                out.push(String::from_utf8_lossy(bytes).into_owned());
            }
        }
        for (_, v) in d.iter() {
            from_obj(v, out);
        }
    }
    fn from_obj(o: &lopdf::Object, out: &mut Vec<String>) {
        match o {
            lopdf::Object::Dictionary(d) => from_dict(d, out),
            lopdf::Object::Stream(s) => from_dict(&s.dict, out),
            lopdf::Object::Array(a) => a.iter().for_each(|v| from_obj(v, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for obj in doc.objects.values() {
        from_obj(obj, &mut out);
    }
    out
}

#[test]
fn resume_pdf_embeds_contact_link_annotations() {
    // A real resume export must emit clickable annotations for the contact line's
    // links (markdown link → its URL; bare email → mailto:). End-to-end check that
    // the exact-metrics rects are wired through, not just unit-tested in isolation.
    let text = "Jane Doe\njane@example.com | [LinkedIn](https://linkedin.com/in/jane)";
    let request = ExportRequest {
        text: text.to_string(),
        format: super::super::types::ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: super::super::types::TemplateId::Modern,
        meta: None,
        ats_mode: false,
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
fn centered_template_resume_pdf_is_generated() {
    // Executive centers the name (name_centered = true) — exercise the exact-advance
    // centering path end-to-end and confirm a non-empty PDF is produced.
    let request = ExportRequest {
        text: "Alexander Hamilton\nalex@example.com\n\nEXPERIENCE\nTreasury  2020 - Present\nSecretary"
            .to_string(),
        format: super::super::types::ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: super::super::types::TemplateId::Executive,
        meta: None,
        ats_mode: false,
    };
    let bytes = generate_pdf(&request).expect("executive resume pdf");
    assert!(
        bytes.len() > 1000,
        "expected a non-trivial PDF, got {} bytes",
        bytes.len()
    );
}
