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

/// #1451 / #1561: the header shape of a designer-made PDF — leading blank lines,
/// an ALL-CAPS Unicode name, a tagline, `City, Country · email · phone`, and
/// letter-spaced headings baked into the text layer (one with a hyphenated word).
const DESIGNED_PDF: &str = "\n\nMARÍA GARCÍA\n\nData Engineer  ·  Platform Focus\n\n\
Lyon, France  ·  maria@example.com  ·  0612 345678\n\
example.dev  ·  linkedin.com/in/example\n\n\
P R O F I L E\n\nEngineer building data platforms end to end.\n\n\
E X P E R I E N C E\n\nAcme Corp  2020 - Present\nData Engineer\n- Built pipelines\n\n\
S E L E C T E D   O P E N - S O U R C E   P R O J E C T S\n\nflowkit\n- A streaming toolkit\n";

#[test]
fn designed_pdf_header_yields_name_location_and_a_projects_section() {
    let sr = structure(&extracted(DESIGNED_PDF, vec![]));
    assert_eq!(sr.name.value, "MARÍA GARCÍA");
    assert_eq!(sr.name.confidence, Confidence::High);
    assert_eq!(
        sr.location.map(|l| l.value).as_deref(),
        Some("Lyon, France")
    );
    let projects = sr.sections.iter().find(|s| s.kind == "projects");
    assert_eq!(
        projects.map(|s| s.heading.as_str()),
        Some("SELECTED OPEN-SOURCE PROJECTS"),
        "sections: {:?}",
        sr.sections
            .iter()
            .map(|s| (&s.kind, &s.heading))
            .collect::<Vec<_>>()
    );
    assert!(!sr.review_required, "warnings: {:?}", sr.warnings);
}

#[test]
fn a_kerned_email_and_digit_spaced_phone_are_rejoined() {
    let text = "Jane Doe\n\njane0 @example.com  +3 1 6 1 2 3 4 5 6 7 8\n\nEXPERIENCE\nAcme\n- x";
    let sr = structure(&extracted(text, vec![]));
    assert_eq!(
        sr.email.map(|e| e.value).as_deref(),
        Some("jane0@example.com")
    );
    assert_eq!(sr.phone.map(|p| p.value).as_deref(), Some("+31612345678"));
    assert!(
        !sr.warnings.iter().any(|w| w.contains("email")),
        "{:?}",
        sr.warnings
    );
    // A normally grouped number is left as written.
    let grouped = structure(&extracted("Jane Doe\n+31 6 1234 5678\n", vec![]));
    assert_eq!(
        grouped.phone.map(|p| p.value).as_deref(),
        Some("+31 6 1234 5678")
    );
}

#[test]
fn a_comma_bearing_skills_segment_is_never_a_location() {
    let text = "Jane Doe\njane@example.com\n\nSKILLS\nRust, TypeScript | Python, SQL\n";
    assert!(structure(&extracted(text, vec![])).location.is_none());
}

#[test]
fn a_letter_spaced_compound_heading_ends_the_location_scan() {
    let text = "Jane Doe\njane@example.com\n\nS E L E C T E D   O P E N - S O U R C E   P R O J E C T S\nProject Alpha, Rust | Project Beta, SQL\n";
    assert!(structure(&extracted(text, vec![])).location.is_none());
}
