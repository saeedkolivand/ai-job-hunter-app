//! Shared fixtures + OOXML-part helpers reused across the model_docx test topics.

use std::io::{Cursor, Read};

use crate::export::model_docx::generate_resume_docx;
use crate::export::templates::Template;
use crate::export::types::TemplateId;

pub(super) const RESUME: &str = "\
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

/// A project's `·`-separated tech-stack line must reach DOCX as the entry
/// SUBTITLE — the italic run `RunOpts::subtitle` styles — and not as an ordinary
/// body paragraph. This is the structural guard for the adapter regrouping
/// (`model::adapter::absorb_project_line`) surviving all the way to the second
/// export format: the PDF matrix test can only see that the WORDS rendered,
/// while run properties here can tell a styled meta line from flat prose.
pub(super) const PROJECTS_RESUME: &str = "\
Jane Doe
jane@example.com

PROJECTS

**Ledger CLI** · https://github.com/janedoe/ledger
Rust · SQLite · Clap
A double-entry bookkeeping tool for the terminal.
";

pub(super) fn build(template_id: TemplateId, ats_mode: bool) -> Vec<u8> {
    let template = Template::get(template_id);
    let docx = generate_resume_docx(RESUME, None, &template, ats_mode).expect("generate docx");
    let mut buffer = Cursor::new(Vec::new());
    docx.build().pack(&mut buffer).expect("pack docx");
    buffer.into_inner()
}

/// Read a named part out of the DOCX zip.
pub(super) fn part(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("docx zip");
    let mut s = String::new();
    zip.by_name(name)
        .expect(name)
        .read_to_string(&mut s)
        .expect("read part");
    s
}

/// Strip XML tags so body text can be checked for content survival.
pub(super) fn text_of(xml: &str) -> String {
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

/// Every `w:sz w:val="N"` (half-points) found in a DOCX body, in document
/// order. Deliberately does not match `w:szCs` (the companion complex-script
/// size, same value) — the literal `w:sz w:val="` substring requires a space
/// right after `sz`, which `szCs` never has.
pub(super) fn all_font_sizes(xml: &str) -> Vec<u32> {
    let needle = "w:sz w:val=\"";
    let mut sizes = Vec::new();
    let mut rest = xml;
    while let Some(idx) = rest.find(needle) {
        let after = &rest[idx + needle.len()..];
        let end = after
            .find('"')
            .expect("w:sz w:val opening quote must close");
        sizes.push(
            after[..end]
                .parse::<u32>()
                .expect("w:sz w:val must be numeric"),
        );
        rest = &after[end..];
    }
    sizes
}
