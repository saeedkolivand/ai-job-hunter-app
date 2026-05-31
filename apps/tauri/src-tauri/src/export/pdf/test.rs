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
        locale: None,
        contact: None,
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
        locale: None,
        contact: None,
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
        locale: None,
        contact: None,
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
        locale: None,
        contact: None,
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
        locale: None,
        contact: None,
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
        locale: None,
        contact: None,
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
        locale: None,
        contact: None,
    };
    let bytes = generate_pdf(&request).expect("executive resume pdf");
    assert!(
        bytes.len() > 1000,
        "expected a non-trivial PDF, got {} bytes",
        bytes.len()
    );
}

#[test]
fn looks_like_contact_line_detects_contact_lines_only() {
    // Contact lines: an email, or ≥2 "|" separators.
    assert!(looks_like_contact_line("Zaandam | a@b.com | +31 6 1234"));
    assert!(looks_like_contact_line("a@b.com"));
    assert!(looks_like_contact_line("City | Phone | LinkedIn"));
    // Plain recipient / address lines are NOT contact lines.
    assert!(!looks_like_contact_line("JAKALA"));
    assert!(!looks_like_contact_line("Hiring Team"));
    assert!(!looks_like_contact_line("123 Main St | Suite 5")); // single separator
}

#[test]
fn cover_letter_does_not_leak_generated_contact_line() {
    // The generated letter still carries its own contact line (with a markdown
    // link). With a contact profile present, the letterhead renders the profile and
    // the text's contact line must be dropped — never leaked into the body as raw
    // markdown (the `[Dribbble](…` / truncation symptom).
    let text = "Zohreh Nejati\n\
        Zaandam, Netherlands | z@example.com | +31 6 3478 0936 | [LinkedIn](https://linkedin.com/in/z) | [Dribbble](https://dribbble.com/zohreh-nejati)\n\
        31. Mai 2026\n\
        JAKALA\n\
        Hiring Team\n\
        Sehr geehrtes JAKALA-Team,\n\n\
        Mit mehr als vier Jahren Erfahrung bringe ich die Faehigkeit mit.\n\n\
        Mit freundlichen Gruessen,\n\
        Zohreh Nejati";
    let request = ExportRequest {
        text: text.to_string(),
        format: super::super::types::ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: super::super::types::TemplateId::Modern,
        meta: Some(GenerationMeta {
            candidate_name: Some("Zohreh Nejati".to_string()),
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

/// Collect every link annotation's `[x0,y0,x1,y1]` rect (points) + target URL.
/// printpdf writes `/Annots` as **inline** dictionaries nested in the page object,
/// so we recurse through arrays/dicts (not just top-level objects).
fn collect_link_rects(doc: &lopdf::Document) -> Vec<([f32; 4], String)> {
    fn from_dict(d: &lopdf::Dictionary, out: &mut Vec<([f32; 4], String)>) {
        let is_link = d
            .get(b"Subtype")
            .ok()
            .and_then(|v| v.as_name().ok())
            .map(|n| n == b"Link")
            .unwrap_or(false);
        if is_link {
            let rect = d
                .get(b"Rect")
                .ok()
                .and_then(|v| v.as_array().ok())
                .and_then(|a| {
                    let v: Vec<f32> = a.iter().filter_map(|o| o.as_float().ok()).collect();
                    <[f32; 4]>::try_from(v).ok()
                });
            let uri = match d.get(b"A") {
                Ok(lopdf::Object::Dictionary(ad)) => ad
                    .get(b"URI")
                    .ok()
                    .and_then(|u| u.as_str().ok())
                    .map(|b| String::from_utf8_lossy(b).into_owned()),
                _ => None,
            };
            if let (Some(rect), Some(uri)) = (rect, uri) {
                out.push((rect, uri));
            }
        }
        for (_, v) in d.iter() {
            from_obj(v, out);
        }
    }
    fn from_obj(o: &lopdf::Object, out: &mut Vec<([f32; 4], String)>) {
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

/// A long contact profile (many links) used to exercise header wrapping in both
/// the résumé layout engine and the legacy cover-letter letterhead.
fn long_contact_profile() -> crate::contact_profile::ContactProfile {
    use crate::contact_profile::{ContactLink, ContactProfile, LocalizedText};
    let extra = |label: &str, url: &str| ContactLink {
        label: label.to_string(),
        url: url.to_string(),
    };
    ContactProfile {
        location: Some(LocalizedText {
            default: "Zaandam, Netherlands".to_string(),
            ..Default::default()
        }),
        email: Some("zohrehnejati0@gmail.com".to_string()),
        phone: Some("+31 6 3478 0936".to_string()),
        linkedin: Some("https://www.linkedin.com/in/zohreh-nejati/".to_string()),
        website: Some("https://drive.google.com/file/d/abc/view".to_string()),
        extra_links: vec![
            extra("Dribbble", "https://dribbble.com/zohreh"),
            extra("Behance", "https://behance.net/zohreh"),
            extra("Portfolio", "https://zohreh.example/portfolio"),
            extra("YouTube", "https://youtube.com/@zohreh"),
            extra("Instagram", "https://instagram.com/zohreh"),
            extra("Medium", "https://medium.com/@zohreh"),
        ],
        ..Default::default()
    }
}

#[test]
fn resume_long_contact_line_wraps_within_page() {
    let request = ExportRequest {
        text: "Zohreh Nejati\n\nEXPERIENCE\nAcme  2020 - Present\nDesigner\n- Did work".to_string(),
        format: super::super::types::ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: super::super::types::TemplateId::Modern,
        meta: Some(GenerationMeta {
            candidate_name: Some("Zohreh Nejati".to_string()),
            job_title: None,
            company_name: None,
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: Some(long_contact_profile()),
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
        text:
            "Sehr geehrtes Team,\n\nIch bewerbe mich.\n\nMit freundlichen Gruessen,\nZohreh Nejati"
                .to_string(),
        format: super::super::types::ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: super::super::types::TemplateId::Modern,
        meta: Some(GenerationMeta {
            candidate_name: Some("Zohreh Nejati".to_string()),
            job_title: None,
            company_name: None,
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: Some(long_contact_profile()),
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

/// Dev tool (ignored): write legacy-vs-engine sample resume PDFs for every
/// template to `target/sample_pdfs/`, so the canonical layout engine's output
/// can be eyeballed against the legacy renderer before the `layout_pdf` flag is
/// flipped to default. Both backends are compiled regardless of the feature, so
/// no special build is needed. Run with:
///
/// ```text
/// cargo test -p ajh-tauri -- --ignored dump_sample_resume_pdfs --nocapture
/// ```
///
/// Output files: `<template>_legacy.pdf` / `<template>_engine.pdf` (+ an ATS pair
/// for the two-column template) under `apps/tauri/src-tauri/target/sample_pdfs/`.
#[test]
#[ignore = "dev tool: writes sample PDFs to target/sample_pdfs for visual review"]
fn dump_sample_resume_pdfs() {
    use std::fs;
    use std::path::Path;

    use super::super::types::TemplateId;

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
        TemplateId::Modern,
        TemplateId::Executive,
        TemplateId::EditorialSerif,
        TemplateId::SwissMinimal,
        TemplateId::TwoColumn,
        TemplateId::MonoTechnical,
        TemplateId::RefinedExecutive,
        TemplateId::Academic,
    ];

    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/sample_pdfs");
    fs::create_dir_all(&out).expect("create target/sample_pdfs");

    for id in templates {
        let template = Template::get(id);
        let slug = format!("{id:?}").to_lowercase();

        let legacy = generate_resume_pdf(SAMPLE, None, &template, false).expect("legacy pdf");
        fs::write(out.join(format!("{slug}_legacy.pdf")), &legacy).expect("write legacy pdf");

        let engine = crate::export::layout_pdf::generate_resume_pdf(SAMPLE, None, &template, false)
            .expect("engine pdf");
        fs::write(out.join(format!("{slug}_engine.pdf")), &engine).expect("write engine pdf");
    }

    // One ATS pair on the two-column template to show single-column linearization.
    let tc = Template::get(TemplateId::TwoColumn);
    let legacy_ats = generate_resume_pdf(SAMPLE, None, &tc, true).expect("legacy ats pdf");
    fs::write(out.join("twocolumn_legacy_ats.pdf"), &legacy_ats).expect("write legacy ats pdf");
    let engine_ats = crate::export::layout_pdf::generate_resume_pdf(SAMPLE, None, &tc, true)
        .expect("engine ats pdf");
    fs::write(out.join("twocolumn_engine_ats.pdf"), &engine_ats).expect("write engine ats pdf");

    eprintln!("wrote sample PDFs to apps/tauri/src-tauri/target/sample_pdfs/");
}
