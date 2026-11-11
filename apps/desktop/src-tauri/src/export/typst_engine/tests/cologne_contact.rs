//! #1499: Cologne Navy contact line must use ONE separator style with linked items.

use super::fixtures::{opts_a4, template_style};
use crate::export::types::TemplateId;
use crate::export::typst_engine::{render_pdf_with_photo, TypstTemplate};
use crate::model::adapter::model_from_resume_text;

const LINKED_CONTACT: &str = "\
Jane Doe
Berlin, Germany | jane@example.com | 0123456789 | [LinkedIn](https://linkedin.com/in/janedoe) | [GitHub](https://github.com/janedoe) | [Website](https://example.com)

EXPERIENCE
Engineer | Acme | 2020 – 2024
- Built things
";

fn contact_text(id: TemplateId, tpl: TypstTemplate) -> String {
    let model = model_from_resume_text(LINKED_CONTACT);
    let t = template_style(id);
    let bytes = render_pdf_with_photo(&model, tpl, &opts_a4(), Some(&t), None).expect("render");
    let text = pdf_extract::extract_text_from_mem(&bytes).expect("pdf-extract");
    text.lines()
        .find(|l| l.contains("jane@example.com"))
        .expect("contact line present")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn cologne_navy_linked_contact_has_one_separator_style() {
    let line = contact_text(TemplateId::CologneNavy, TypstTemplate::CologneNavy);
    assert!(!line.contains('|'), "no pipes in cologne contact: {line}");
    assert!(!line.contains("· ·"), "no doubled separators: {line}");
    assert_eq!(
        line,
        "Berlin, Germany · jane@example.com · 0123456789 · LinkedIn · GitHub · Website"
    );
}

#[test]
fn single_column_linked_contact_keeps_pipes() {
    let line = contact_text(TemplateId::Classic, TypstTemplate::SingleColumn);
    assert!(line.contains('|'), "pipe-joined line kept: {line}");
    assert!(!line.contains("· ·"), "{line}");
}
