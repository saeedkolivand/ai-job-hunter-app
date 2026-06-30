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
    let xml = part(&build(TemplateId::Modern, false), "word/document.xml");
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
    let bytes = build(TemplateId::Modern, false);
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
