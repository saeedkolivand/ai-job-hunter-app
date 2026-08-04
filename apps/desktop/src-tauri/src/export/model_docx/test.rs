//! Golden invariants for the model-based DOCX backend. Each test builds a real
//! DOCX and inspects the unzipped OOXML parts (`document.xml` and the hyperlink
//! relationships), since DOCX is flow-based and has no pixel geometry to compare.

use std::io::{Cursor, Read};

use super::*;
use crate::export::types::TemplateId;

const RESUME: &str = "\
Jane Doe
jane@example.com | [LinkedIn](https://linkedin.com/in/jane)

Experienced engineer building reliable web applications end to end.

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

fn build(template_id: TemplateId, ats_mode: bool) -> Vec<u8> {
    let template = Template::get(template_id);
    let docx = generate_resume_docx(RESUME, None, &template, ats_mode).expect("generate docx");
    let mut buffer = Cursor::new(Vec::new());
    docx.build().pack(&mut buffer).expect("pack docx");
    buffer.into_inner()
}

/// Read a named part out of the DOCX zip.
fn part(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("docx zip");
    let mut s = String::new();
    zip.by_name(name)
        .expect(name)
        .read_to_string(&mut s)
        .expect("read part");
    s
}

/// Strip XML tags so body text can be checked for content survival.
fn text_of(xml: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in xml.chars() {
        match c {
            '<' => {
                in_tag = true;
                out.push(' ');
            }
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

#[test]
fn two_column_renders_a_borderless_shaded_table() {
    let xml = part(&build(TemplateId::Atelier, false), "word/document.xml");
    assert!(xml.contains("<w:tbl"), "two-column DOCX must use a table");
    // Atelier sidebar tint (240,239,248) → F0EFF8 fill on the sidebar cell.
    assert!(
        xml.contains(r#"w:fill="F0EFF8""#),
        "sidebar cell should carry the Atelier tint"
    );
}

#[test]
fn two_column_splits_sections_between_columns() {
    // SKILLS + EDUCATION are sidebar sections; EXPERIENCE is a main section.
    // All three must survive somewhere in the document.
    let text = text_of(&part(
        &build(TemplateId::Atelier, false),
        "word/document.xml",
    ));
    for needle in ["EXPERIENCE", "SKILLS", "EDUCATION", "Acme Corp", "Rust"] {
        assert!(text.contains(needle), "two-column lost content: {needle:?}");
    }
}

#[test]
fn ats_mode_emits_no_table() {
    // ATS mode linearizes to a single column — no two-column table.
    let xml = part(&build(TemplateId::Atelier, true), "word/document.xml");
    assert!(
        !xml.contains("<w:tbl"),
        "ATS mode must not emit a two-column table"
    );
    let text = text_of(&xml);
    for needle in ["EXPERIENCE", "SKILLS", "EDUCATION"] {
        assert!(
            text.contains(needle),
            "ATS linearization dropped {needle:?}"
        );
    }
}

#[test]
fn single_column_template_has_no_table() {
    let xml = part(&build(TemplateId::SwissMinimal, false), "word/document.xml");
    assert!(
        !xml.contains("<w:tbl"),
        "single-column template must not use a table"
    );
    let text = text_of(&xml);
    for needle in [
        "Jane Doe",
        "EXPERIENCE",
        "Rust, TypeScript, React",
        "BSc Computer Science",
    ] {
        assert!(
            text.contains(needle),
            "single-column lost content: {needle:?}"
        );
    }
}

#[test]
fn contact_links_become_hyperlinks_with_correct_targets() {
    let bytes = build(TemplateId::SwissMinimal, false);
    let doc = part(&bytes, "word/document.xml");
    assert!(
        doc.contains("<w:hyperlink"),
        "contact links must render as hyperlinks"
    );

    // External hyperlink targets live in the relationships part.
    let rels = part(&bytes, "word/_rels/document.xml.rels");
    assert!(
        rels.contains("https://linkedin.com/in/jane"),
        "LinkedIn URL must be a hyperlink target"
    );
    assert!(
        rels.contains("mailto:jane@example.com"),
        "email must be a mailto hyperlink target"
    );

    // The visible label, not the raw URL, is shown.
    let text = text_of(&doc);
    assert!(text.contains("LinkedIn"), "link label should display");
    assert!(
        !text.contains("https://linkedin.com/in/jane"),
        "raw URL must not be visible text"
    );
}

// ── candidate_name metadata is a fallback, not an override (H) ───────────────

#[test]
fn candidate_name_metadata_is_fallback_when_text_has_a_name() {
    let template = Template::get(TemplateId::SwissMinimal);
    let meta = GenerationMeta {
        candidate_name: Some("Someone Else".to_string()),
        job_title: None,
        company_name: None,
        target_language: None,
    };
    let docx = generate_resume_docx(RESUME, Some(&meta), &template, false).expect("generate docx");
    let mut buffer = Cursor::new(Vec::new());
    docx.build().pack(&mut buffer).expect("pack docx");
    let text = text_of(&part(&buffer.into_inner(), "word/document.xml"));
    assert!(
        text.contains("Jane Doe"),
        "text-derived name must win over meta.candidate_name"
    );
    assert!(
        !text.contains("Someone Else"),
        "metadata name must not override a text-derived name"
    );
}

#[test]
fn candidate_name_metadata_fills_header_when_text_has_none() {
    let template = Template::get(TemplateId::SwissMinimal);
    let text = "jane@example.com\n\nSUMMARY\nSome text.";
    // Padded on purpose: the emptiness check (`!name.trim().is_empty()`) used
    // to trim while the assignment (`model.header.name = name.to_string()`)
    // didn't, so a padded metadata name rendered with stray leading/trailing
    // whitespace baked into the header run.
    let meta = GenerationMeta {
        candidate_name: Some("  Jane Smith  ".to_string()),
        job_title: None,
        company_name: None,
        target_language: None,
    };
    let docx = generate_resume_docx(text, Some(&meta), &template, false).expect("generate docx");
    let mut buffer = Cursor::new(Vec::new());
    docx.build().pack(&mut buffer).expect("pack docx");
    let bytes = buffer.into_inner();
    let xml = part(&bytes, "word/document.xml");
    // The run text itself must be exactly the trimmed name — bounded
    // immediately by tags, no leaked interior whitespace from the untrimmed
    // metadata field.
    assert!(
        xml.contains(">Jane Smith<"),
        "the header run must contain the trimmed name with no stray \
         whitespace: {xml}"
    );
    // Not just "appears somewhere in the body" — it must land as the header,
    // first in the document, not e.g. folded into a later section by a
    // fallback that reached the wrong branch.
    let doc_text = text_of(&xml);
    assert!(
        doc_text.trim_start().starts_with("Jane Smith"),
        "metadata name must fill the header and land first in the document, \
         not merely appear somewhere in the body: {doc_text:?}"
    );
}

#[test]
fn declares_a4_page_size_and_fallback_fonts() {
    // Academic: name/heading/body all SourceSerif4 → Georgia.
    let xml = part(&build(TemplateId::Academic, false), "word/document.xml");
    assert!(
        xml.contains(r#"w:w="11906""#) && xml.contains(r#"w:h="16838""#),
        "A4 page size"
    );
    assert!(
        xml.contains(r#"w:ascii="Georgia""#),
        "SourceSerif4 → Georgia"
    );
    let bundled = "Source Serif 4";
    assert!(
        !xml.contains(&format!(r#""{bundled}""#)),
        "bundled font {bundled:?} must not leak"
    );
}
