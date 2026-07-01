use super::*;
use crate::extraction::types::{ExtractedResume, Link, SourceFormat};
use crate::model::document::Block;

const RESUME: &str = "\
Jane Doe
jane@example.com | +31 6 12345678

Experienced engineer building reliable web applications end to end.

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Led a team of five engineers

SKILLS
- Rust, TypeScript, React

EDUCATION
State University  2013 - 2017
BSc Computer Science
";

fn extracted(text: &str, links: Vec<Link>) -> ExtractedResume {
    ExtractedResume {
        text: text.to_string(),
        links,
        confidence: Confidence::High,
        warnings: vec![],
        source_format: SourceFormat::PlainText,
    }
}

#[test]
fn extracts_name_email_and_phone_with_confidence() {
    let sr = structure(&extracted(RESUME, vec![]));
    assert_eq!(sr.name.value, "Jane Doe");
    assert_eq!(sr.name.confidence, Confidence::High);

    let email = sr.email.expect("email");
    assert_eq!(email.value, "jane@example.com");
    assert_eq!(email.confidence, Confidence::High);
    // The span points back at the email in the source.
    let span = email.source_span.expect("email span");
    assert_eq!(&RESUME[span.start..span.end], "jane@example.com");

    assert!(sr.phone.is_some(), "phone should be detected");
}

#[test]
fn section_inventory_classifies_and_skips_preamble() {
    let sr = structure(&extracted(RESUME, vec![]));
    let kinds: Vec<&str> = sr.sections.iter().map(|s| s.kind.as_str()).collect();
    assert_eq!(kinds, vec!["experience", "skills", "education"]);
    // The untitled summary preamble is not listed as a heading.
    assert!(sr.sections.iter().all(|s| !s.heading.is_empty()));
}

#[test]
fn good_resume_does_not_require_review() {
    let sr = structure(&extracted(RESUME, vec![]));
    assert!(!sr.review_required, "warnings: {:?}", sr.warnings);
    assert!(sr.warnings.is_empty());
}

#[test]
fn missing_email_flags_review_with_a_warning() {
    let no_email = "Jane Doe\n\nEXPERIENCE\nAcme Corp\n- did things\n\nSKILLS\n- Rust";
    let sr = structure(&extracted(no_email, vec![]));
    assert!(sr.email.is_none());
    assert!(sr.review_required);
    assert!(sr.warnings.iter().any(|w| w.contains("email")));
}

#[test]
fn missing_name_scores_low_and_flags_review() {
    // Leading blank lines so the parser finds no name line.
    let sr = structure(&extracted(
        "\n\nEXPERIENCE\nAcme Corp\n- x\n\nSKILLS\n- Rust",
        vec![],
    ));
    assert_eq!(sr.name.confidence, Confidence::Low);
    assert!(sr.name.value.is_empty());
    assert!(sr.review_required);
}

#[test]
fn links_carry_high_confidence_and_spans() {
    let text = "Jane Doe\nhttps://janedoe.dev\n\nEXPERIENCE\nAcme\n- x\n\nSKILLS\n- Rust";
    let links = vec![Link {
        anchor_text: "janedoe.dev".to_string(),
        url: "https://janedoe.dev".to_string(),
    }];
    let sr = structure(&extracted(text, links));
    assert_eq!(sr.links.len(), 1);
    assert_eq!(sr.links[0].confidence, Confidence::High);
    assert!(sr.links[0].source_span.is_some());
}

#[test]
fn build_model_reconciles_header_from_typed_fields() {
    let links = vec![Link {
        anchor_text: "LinkedIn".to_string(),
        url: "https://linkedin.com/in/jane".to_string(),
    }];
    let ex = extracted(RESUME, links);
    let sr = structure(&ex);
    let model = build_model(&ex, &sr);

    assert_eq!(model.header.name, "Jane Doe");
    // Contact runs include the email and the link as a clickable run.
    let contact_text: String = model
        .header
        .contact
        .iter()
        .map(|r| r.text.as_str())
        .collect();
    assert!(
        contact_text.contains("jane@example.com"),
        "got: {contact_text}"
    );
    assert!(
        model
            .header
            .contact
            .iter()
            .any(|r| r.link.as_deref() == Some("https://linkedin.com/in/jane")),
        "link run missing"
    );
    // Body sections survive from the adapter.
    assert!(model.sections.iter().any(|s| {
        s.heading == "EXPERIENCE" && s.blocks.iter().any(|b| matches!(b, Block::Entry(_)))
    }));
}
