use super::super::types::{ExportFormat, GenerationMeta, TemplateId};
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
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_cover_letter_basic() {
    let request = ExportRequest {
        text: "Dear Hiring Manager,\n\nI am writing to apply...".to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_resume_with_meta() {
    let request = ExportRequest {
        text: "John Doe\njohn@example.com".to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::SwissMinimal,
        meta: Some(GenerationMeta {
            candidate_name: Some("Jane Smith".to_string()),
            job_title: Some("Software Engineer".to_string()),
            company_name: Some("Test Corp".to_string()),
            target_language: None,
        }),
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_resume_with_section_markers() {
    let text = "### CANDIDATE RESUME ###\nJohn Doe\njohn@example.com\n### JOB ADVERTISEMENT ###\nJob description here";
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
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
    };
    let result = generate_pdf(&request);
    assert!(result.is_ok());
}

#[test]
fn test_generate_pdf_cover_letter_german_market_with_betreff() {
    // German market: a bold Betreff line + a German salutation + formal sign-off.
    // Exercises the subject-line render, market date placement, and the
    // locale-aware salutation/sign-off detection (previously English/German-only).
    let text = "Max Mustermann\n\nBetreff: Bewerbung als Frontend Engineer\n\nSehr geehrte Damen und Herren,\n\nmit großem Interesse bewerbe ich mich.\n\nMit freundlichen Grüßen\nMax Mustermann";
    let request = ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::Classic,
        meta: None,
        ats_mode: false,
        locale: Some("de".to_string()),
        contact: None,
        accent: None,
    };
    let bytes = generate_pdf(&request).expect("German cover letter renders");
    assert!(!bytes.is_empty());
}

#[test]
fn test_generate_pdf_cover_letter_french_salutation() {
    // French salutation/sign-off must be recognized (not dumped into the
    // recipient block) now that detection is locale-aware.
    let text = "Marie Dupont\n\nMadame, Monsieur,\n\nje vous écris pour le poste.\n\nCordialement,\nMarie Dupont";
    let request = ExportRequest {
        text: text.to_string(),
        format: ExportFormat::Pdf,
        document_type: DocumentType::CoverLetter,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: Some("fr".to_string()),
        contact: None,
        accent: None,
    };
    let bytes = generate_pdf(&request).expect("French cover letter renders");
    assert!(!bytes.is_empty());
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
        format: ExportFormat::Pdf,
        document_type: DocumentType::Resume,
        template_id: TemplateId::SwissMinimal,
        meta: None,
        ats_mode: false,
        locale: None,
        contact: None,
        accent: None,
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
    };
    let bytes = generate_pdf(&request).expect("modern resume pdf");
    assert!(
        bytes.len() > 1000,
        "expected a non-trivial PDF, got {} bytes",
        bytes.len()
    );
}

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
/// Typst writes `/Annots` as **inline** dictionaries nested in the page object,
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
            default: "Amsterdam, Netherlands".to_string(),
            ..Default::default()
        }),
        email: Some("lena.vos@example.com".to_string()),
        phone: Some("+31 6 12345678".to_string()),
        linkedin: Some("https://www.linkedin.com/in/lena-vos/".to_string()),
        website: Some("https://drive.google.com/file/d/abc/view".to_string()),
        extra_links: vec![
            extra("Dribbble", "https://dribbble.com/lenavos"),
            extra("Behance", "https://behance.net/lenavos"),
            extra("Portfolio", "https://lena.example/portfolio"),
            extra("YouTube", "https://youtube.com/@lenavos"),
            extra("Instagram", "https://instagram.com/lenavos"),
            extra("Medium", "https://medium.com/@lenavos"),
        ],
        ..Default::default()
    }
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
        };
        let slug = format!("{id:?}").to_lowercase();
        let bytes = generate_pdf(&request).expect("typst pdf");
        fs::write(out.join(format!("{slug}_typst.pdf")), &bytes).expect("write pdf");
    }

    eprintln!("wrote sample PDFs to apps/desktop/src-tauri/target/sample_pdfs/");
}

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
