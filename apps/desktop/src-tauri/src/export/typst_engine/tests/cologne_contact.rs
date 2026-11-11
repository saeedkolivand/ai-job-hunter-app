//! #1499: Cologne Navy contact line must use ONE separator style with linked items.

use super::fixtures::{opts_a4, template_style};
use super::svg_geometry::{svg_page1, text_lines};
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
    contact_text_of(LINKED_CONTACT, id, tpl)
}

fn contact_text_of(resume: &str, id: TemplateId, tpl: TypstTemplate) -> String {
    let model = model_from_resume_text(resume);
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

fn cologne_line(contact: &str) -> String {
    let resume = format!(
        "Jane Doe
{contact}

EXPERIENCE
Engineer | Acme | 2020 – 2024
- Built things
"
    );
    contact_text_of(&resume, TemplateId::CologneNavy, TypstTemplate::CologneNavy)
}

#[test]
fn cologne_navy_contact_without_links_uses_one_separator_style() {
    assert_eq!(
        cologne_line("Berlin, Germany | jane@example.com | 0123456789"),
        "Berlin, Germany · jane@example.com · 0123456789"
    );
}

#[test]
fn cologne_navy_contact_starting_with_a_link() {
    assert_eq!(
        cologne_line("[Website](https://example.com) | jane@example.com | Berlin, Germany"),
        "Website · jane@example.com · Berlin, Germany"
    );
}

#[test]
fn cologne_navy_contact_ending_with_a_link() {
    assert_eq!(
        cologne_line(
            "jane@example.com | Berlin, Germany | [LinkedIn](https://linkedin.com/in/janedoe)"
        ),
        "jane@example.com · Berlin, Germany · LinkedIn"
    );
}

/// A bold plain run must stay bold after the split-and-rejoin. Glyphs carry no
/// weight in SVG, so measure it like `education_style`: the bold line is
/// wider than the identical text rendered regular.
#[test]
fn cologne_navy_bold_plain_run_keeps_its_styling() {
    let width = |contact: &str| {
        let resume = format!(
            "Jane Doe
{contact}

SUMMARY
Wxwxwxwxwxw
"
        );
        let svg = svg_page1(
            &model_from_resume_text(&resume),
            &template_style(TemplateId::CologneNavy),
            false,
        );
        // Name is the first line, contact the second.
        let l = text_lines(&svg)[1];
        l.2 - l.1
    };
    let bold = width("**Wxwxwxwxwxw** | jane@example.com");
    let plain = width("Wxwxwxwxwxw | jane@example.com");
    assert!(
        bold > plain + 1.0,
        "bold contact run ({bold:.2}pt) must be wider than regular ({plain:.2}pt)"
    );
}
