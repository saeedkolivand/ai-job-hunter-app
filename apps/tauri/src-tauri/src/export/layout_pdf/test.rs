//! Parity / round-trip tests for the layout-engine PDF backend. These run
//! regardless of the `layout_pdf` feature (they call the backend directly), so
//! the new path is always validated. Text survival is checked with `pdf-extract`
//! and link annotations with `lopdf` — the same tools the eventual Phase 4
//! round-trip gate uses.

use super::*;
use crate::export::types::TemplateId;

const RESUME: &str = "\
Jane Doe
jane@example.com | [LinkedIn](https://linkedin.com/in/jane) | https://janedoe.dev

Professional summary about building reliable software systems end to end.

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Led a team of five engineers
- Shipped three major features

SKILLS
- Rust, TypeScript, React
";

fn extract(bytes: &[u8]) -> String {
    pdf_extract::extract_text_from_mem(bytes).expect("extract text from generated pdf")
}

/// Recursively collect every `/URI` action target in a parsed PDF.
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
fn resume_text_survives_round_trip() {
    let t = Template::get(TemplateId::Modern);
    let bytes = generate_resume_pdf(RESUME, None, &t, false).expect("pdf");
    let text = extract(&bytes);
    for needle in [
        "Jane Doe",
        "EXPERIENCE",
        "Acme Corp",
        "Senior Engineer",
        "Led a team",
        "SKILLS",
        "Rust",
    ] {
        assert!(
            text.contains(needle),
            "missing {needle:?} in extracted text:\n{text}"
        );
    }
}

#[test]
fn resume_embeds_contact_link_annotations() {
    let t = Template::get(TemplateId::Modern);
    let bytes = generate_resume_pdf(RESUME, None, &t, false).expect("pdf");
    let doc = lopdf::Document::load_mem(&bytes).expect("parse generated pdf");
    let uris = collect_uris(&doc);
    assert!(
        uris.iter().any(|u| u == "https://linkedin.com/in/jane"),
        "linkedin link missing: {uris:?}"
    );
    assert!(
        uris.iter().any(|u| u == "mailto:jane@example.com"),
        "mailto missing: {uris:?}"
    );
    assert!(
        uris.iter().any(|u| u == "https://janedoe.dev"),
        "website link missing: {uris:?}"
    );
}

#[test]
fn two_column_resume_renders_both_columns() {
    let t = Template::get(TemplateId::TwoColumn);
    let bytes = generate_resume_pdf(RESUME, None, &t, false).expect("pdf");
    let text = extract(&bytes);
    assert!(text.contains("Jane Doe"), "header");
    assert!(text.contains("Acme Corp"), "main column content");
    assert!(text.contains("Rust"), "sidebar column content");
}

#[test]
fn ats_mode_keeps_all_content_single_column() {
    let t = Template::get(TemplateId::TwoColumn);
    let bytes = generate_resume_pdf(RESUME, None, &t, true).expect("pdf");
    let text = extract(&bytes);
    for needle in ["Jane Doe", "EXPERIENCE", "SKILLS", "Acme Corp", "Rust"] {
        assert!(text.contains(needle), "ats mode lost content: {needle:?}");
    }
}

#[test]
fn generation_meta_name_overrides_header() {
    let t = Template::get(TemplateId::Classic);
    let meta = GenerationMeta {
        candidate_name: Some("Override Name".to_string()),
        job_title: None,
        company_name: None,
        target_language: None,
    };
    let bytes = generate_resume_pdf(RESUME, Some(&meta), &t, false).expect("pdf");
    let text = extract(&bytes);
    assert!(
        text.contains("Override Name"),
        "meta candidate_name should appear in the header:\n{text}"
    );
}

#[test]
fn every_template_renders_a_valid_pdf() {
    for id in [
        TemplateId::Classic,
        TemplateId::Modern,
        TemplateId::Executive,
        TemplateId::EditorialSerif,
        TemplateId::SwissMinimal,
        TemplateId::TwoColumn,
        TemplateId::MonoTechnical,
        TemplateId::RefinedExecutive,
        TemplateId::Academic,
    ] {
        let t = Template::get(id);
        let bytes = generate_resume_pdf(RESUME, None, &t, false).expect("pdf");
        assert!(bytes.len() > 1000, "{id:?} produced a trivial PDF");
        lopdf::Document::load_mem(&bytes).expect("generated pdf must parse");
    }
}
