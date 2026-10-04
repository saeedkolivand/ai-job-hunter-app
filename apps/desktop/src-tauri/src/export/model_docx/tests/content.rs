//! Hyperlink targets, candidate-name fallback rules, and the project
//! tech-stack-line → italic-subtitle structural guard.

use std::io::Cursor;

use super::support::{build, part, text_of, PROJECTS_RESUME, RESUME};
use crate::export::model_docx::generate_resume_docx;
use crate::export::templates::Template;
use crate::export::types::{GenerationMeta, TemplateId};

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
fn project_tech_stack_lands_in_the_italic_subtitle_run() {
    let template = Template::get(TemplateId::Classic);
    let docx =
        generate_resume_docx(PROJECTS_RESUME, None, &template, false).expect("generate docx");
    let mut buffer = Cursor::new(Vec::new());
    docx.build().pack(&mut buffer).expect("pack docx");
    let xml = part(&buffer.into_inner(), "word/document.xml");

    let idx = xml
        .find("SQLite")
        .expect("the tech-stack text must appear in document.xml");
    let run_start = xml[..idx].rfind("<w:r>").expect("enclosing run start");
    let run_end = idx + xml[idx..].find("</w:r>").expect("enclosing run end");
    let run_xml = &xml[run_start..run_end];
    assert!(
        run_xml.contains("<w:i "),
        "the tech-stack line must render as the italic subtitle run, not flat \
         body prose; run xml: {run_xml:?}"
    );

    // The project NAME stays bold, and the description is still there.
    let name_idx = xml
        .find("Ledger CLI")
        .expect("project name in document.xml");
    let name_start = xml[..name_idx].rfind("<w:r>").expect("name run start");
    let name_end = name_idx + xml[name_idx..].find("</w:r>").expect("name run end");
    assert!(
        xml[name_start..name_end].contains("<w:b "),
        "the project name must stay bold; run xml: {:?}",
        &xml[name_start..name_end]
    );
    assert!(xml.contains("double-entry bookkeeping"));
}
