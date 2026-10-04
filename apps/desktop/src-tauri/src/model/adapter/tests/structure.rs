use super::{support::*, *};

/// A realistic synthetic resume exercising header, preamble, entries with
/// subtitles + bullets, a second entry, standalone bullets, and a custom
/// section. No PII (synthetic example domains).
const SAMPLE: &str = "\
Jane Doe
jane@example.com | [LinkedIn](https://linkedin.com/in/jane) | https://janedoe.dev

Experienced engineer with 10 years building web apps.

EXPERIENCE
Acme Corp  2020 - Present
Senior Engineer
- Led a team of five engineers
- Shipped three major features

Beta Inc  2018 - 2020
Engineer
- Built the public API

SKILLS
- Rust, TypeScript, React
- AWS, Docker

SPEAKING ENGAGEMENTS
- Keynote at RustConf
";

fn model() -> DocumentModel {
    model_from_resume_text(SAMPLE)
}

#[test]
fn header_captures_name_and_contact_links() {
    let m = model();
    assert_eq!(m.header.name, "Jane Doe");
    // The contact line keeps every part and surfaces links as link runs.
    let contact = &m.header.contact;
    assert!(flat(contact).contains("LinkedIn"));
    assert!(contact
        .iter()
        .any(|r| r.link.as_deref() == Some("mailto:jane@example.com")));
    assert!(contact
        .iter()
        .any(|r| r.link.as_deref() == Some("https://linkedin.com/in/jane")));
    assert!(contact
        .iter()
        .any(|r| r.link.as_deref() == Some("https://janedoe.dev")));
}

#[test]
fn leading_body_becomes_untitled_summary_section() {
    let m = model();
    let first = &m.sections[0];
    assert_eq!(first.id, SectionId::Summary);
    assert_eq!(first.heading, "", "preamble section has no visible heading");
    assert_eq!(first.blocks.len(), 1);
    match &first.blocks[0] {
        Block::Paragraph(rt) => assert!(flat(rt).contains("Experienced engineer")),
        other => panic!("expected paragraph, got {other:?}"),
    }
}

#[test]
fn sections_are_classified_and_kept_in_order() {
    let m = model();
    let ids = section_ids(&m);
    assert_eq!(
        ids,
        vec![
            &SectionId::Summary, // untitled preamble
            &SectionId::Experience,
            &SectionId::Skills,
            &SectionId::Custom("SPEAKING ENGAGEMENTS".to_string()),
        ]
    );
}

#[test]
fn job_entry_gathers_subtitle_date_and_bullets() {
    let m = model();
    let experience = find_section(&m, SectionId::Experience).expect("experience section");

    // Two entries: Acme then Beta.
    let entries = section_entries(experience);
    assert_eq!(entries.len(), 2);

    let acme = entries[0];
    assert_eq!(flat(&acme.title), "Acme Corp");
    assert_eq!(acme.date.as_deref(), Some("2020 - Present"));
    assert_eq!(
        acme.subtitle.as_ref().map(flat).as_deref(),
        Some("Senior Engineer")
    );
    assert_eq!(acme.bullets.len(), 2);
    assert_eq!(flat(&acme.bullets[0]), "Led a team of five engineers");

    let beta = entries[1];
    assert_eq!(flat(&beta.title), "Beta Inc");
    assert_eq!(beta.date.as_deref(), Some("2018 - 2020"));
    assert_eq!(beta.bullets.len(), 1);
}

#[test]
fn bullets_without_an_entry_are_standalone() {
    let m = model();
    let skills = find_section(&m, SectionId::Skills).expect("skills section");
    assert_eq!(skills.blocks.len(), 2);
    assert!(skills.blocks.iter().all(|b| matches!(b, Block::Bullet(_))));
    match &skills.blocks[0] {
        Block::Bullet(rt) => assert_eq!(flat(rt), "Rust, TypeScript, React"),
        other => panic!("expected bullet, got {other:?}"),
    }
}

#[test]
fn unknown_heading_is_preserved_as_custom() {
    let m = model();
    let speaking = m.sections.last().expect("last section");
    assert_eq!(
        speaking.id,
        SectionId::Custom("SPEAKING ENGAGEMENTS".to_string())
    );
    assert_eq!(speaking.heading, "SPEAKING ENGAGEMENTS");
    assert_eq!(speaking.blocks.len(), 1);
}

#[test]
fn model_is_stamped_as_a_resume() {
    let m = model();
    assert_eq!(m.doc_type, DocumentType::Resume);
    assert_eq!(m.schema_version, crate::model::version::SCHEMA_VERSION);
}

#[test]
fn empty_input_yields_an_empty_resume() {
    let m = model_from_resume_text("");
    assert_eq!(m.header, HeaderBlock::default());
    assert!(m.sections.is_empty());
}

#[test]
fn no_content_is_dropped() {
    // Every non-blank source line must surface somewhere in the model.
    let m = model();
    let mut haystack = String::new();
    haystack.push_str(&m.header.name);
    haystack.push_str(&flat(&m.header.contact));
    for s in &m.sections {
        haystack.push_str(&s.heading);
        for b in &s.blocks {
            match b {
                Block::Paragraph(rt) | Block::Bullet(rt) => haystack.push_str(&flat(rt)),
                Block::Entry(e) => {
                    haystack.push_str(&flat(&e.title));
                    if let Some(st) = &e.subtitle {
                        haystack.push_str(&flat(st));
                    }
                    for bl in &e.bullets {
                        haystack.push_str(&flat(bl));
                    }
                }
            }
        }
    }
    for needle in [
        "Jane Doe",
        "Experienced engineer",
        "Acme Corp",
        "Senior Engineer",
        "Led a team",
        "Beta Inc",
        "Built the public API",
        "Rust, TypeScript, React",
        "AWS, Docker",
        "Keynote at RustConf",
    ] {
        assert!(haystack.contains(needle), "lost content: {needle:?}");
    }
}

/// A short role line directly after the contact is promoted to the header
/// title (not a floating paragraph), and a Markdown thematic break between
/// the title and the first section is dropped — never rendered as "---".
#[test]
fn role_line_becomes_header_title_and_breaks_are_dropped() {
    let resume = "\
Jane Doe
jane@example.com

Front-End Engineer
---

PROFESSIONAL SUMMARY
Senior Front-End Engineer with 6+ years of experience.
";
    let m = model_from_resume_text(resume);
    assert_eq!(m.header.title.as_deref(), Some("Front-End Engineer"));

    // No body paragraph is a literal thematic break or the bare role line.
    for s in &m.sections {
        for b in &s.blocks {
            if let Block::Paragraph(rt) = b {
                let t = flat(rt);
                assert_ne!(t, "---", "literal thematic break leaked into body");
                assert_ne!(
                    t, "Front-End Engineer",
                    "role should be the header title, not a paragraph"
                );
            }
        }
    }

    // The real summary section survives; no invented untitled preamble.
    let summary = find_section(&m, SectionId::Summary).expect("summary section");
    assert_eq!(summary.heading, "PROFESSIONAL SUMMARY");
}

/// A long leading sentence is prose, not a title — it must NOT be promoted.
#[test]
fn long_leading_sentence_is_not_promoted_to_title() {
    // SAMPLE's first preamble line is a full sentence (>6 words, trailing ".").
    let m = model();
    assert_eq!(m.header.title, None);
}

/// The hardened heuristic: real titles promote; known section names, all-caps
/// banners, and prose are rejected so real content never becomes the title.
#[test]
fn is_title_like_distinguishes_titles_from_sections_and_prose() {
    assert!(is_title_like("Front-End Engineer"));
    assert!(is_title_like("Senior Backend Developer"));
    // Known section headings (any case) are content, not titles.
    assert!(!is_title_like("Skills"));
    assert!(!is_title_like("Certifications"));
    assert!(!is_title_like("Education"));
    // An all-caps multi-word banner is a heading, not a title.
    assert!(!is_title_like("KEY HIGHLIGHTS"));
    // Prose (terminal punctuation / too long) is never a title.
    assert!(!is_title_like("A passionate engineer who ships."));
    assert!(!is_title_like("Lots and lots and lots of words here now"));
}

/// Owner-reported: when the source text splits its contact block across
/// TWO physical lines, each ALREADY carrying its own stray leading/
/// trailing separator (a pipe left over from how the line visually
/// continued), the adapter's own " · " join used to double up into a
/// visible "| · |" / "· |" artifact in the rendered header. The fix
/// strips one stray separator off each line's ends before joining, so
/// exactly one clean " · " ever sits between lines while each line's own
/// internal separator style survives untouched.
#[test]
fn contact_lines_with_stray_edge_separators_join_without_doubling() {
    let resume = "\
Jane Doe
Berlin, Germany | jane@example.com |
| +49 30 1234567
";
    let m = model_from_resume_text(resume);
    let joined = flat(&m.header.contact);
    assert_eq!(
        joined, "Berlin, Germany | jane@example.com \u{b7} +49 30 1234567",
        "expected exactly one clean separator between lines, got {joined:?}"
    );
    assert!(
        !joined.contains("\u{b7}  \u{b7}") && !joined.contains("| \u{b7}"),
        "must not contain a doubled separator artifact, got {joined:?}"
    );
}

/// CRITICAL regression: a markdown link in the header contact line must
/// survive as a clickable run, not collapse to plain text once
/// `[label](url)` is (mis)handled upstream.
#[test]
fn contact_markdown_link_survives_as_a_link_run() {
    let resume = "\
Jane Doe
jane@example.com | [linkedin.com/in/jane](https://linkedin.com/in/jane)
";
    let m = model_from_resume_text(resume);
    assert!(
        m.header
            .contact
            .iter()
            .any(|r| r.link.as_deref() == Some("https://linkedin.com/in/jane")),
        "expected a link run, got {:?}",
        m.header.contact
    );
}
