//! Adapter: build a canonical [`DocumentModel`] from already-parsed resume text.
//!
//! Temporary strangler-fig bridge. It reuses the existing line-based
//! [`parser::parse_resume`](crate::export::parser::parse_resume) so the new model
//! (and the layout engine that will consume it in Phase 2b) can be populated
//! WITHOUT yet touching extraction. The flat `Vec<ParsedLine>` is regrouped into
//! the structured shape the model expects: a header plus titled sections whose
//! content is paragraphs, standalone bullets, and entries (a title line with an
//! optional subtitle / date and its own bullets).
//!
//! Faithfulness: this mirrors what `parse_resume` already recognizes — it does
//! not re-classify or enrich. Inline formatting is recovered with
//! [`tokenize_rich`], so links survive as first-class runs (and bold survives
//! wherever the parser preserved it). Content is never dropped: unrecognized or
//! out-of-place lines fall back to paragraphs. The structured extractor that
//! builds the model directly (skipping this text round-trip) arrives in Phase 6.
//!
//! Resumes only. Cover letters have a fundamentally different shape (letterhead,
//! date, recipient, salutation, body, closing, signature) and stay on the legacy
//! `export::pdf` path until a later phase models them explicitly.

use crate::export::parser::parse_resume;
use crate::export::types::{DocumentType, LineKind};

use super::document::{Block, DocumentModel, EntryBlock, HeaderBlock, Section, SectionId};
use super::rich::tokenize_rich;

/// Push a finished block into the active section, or into the preamble when no
/// section has started yet (leading content under no heading).
fn push_block(block: Block, current: &mut Option<Section>, preamble: &mut Vec<Block>) {
    match current {
        Some(section) => section.blocks.push(block),
        None => preamble.push(block),
    }
}

/// Close the in-progress entry (if any), emitting it as a [`Block::Entry`].
fn flush_entry(
    entry: &mut Option<EntryBlock>,
    current: &mut Option<Section>,
    preamble: &mut Vec<Block>,
) {
    if let Some(e) = entry.take() {
        push_block(Block::Entry(e), current, preamble);
    }
}

/// Build a resume [`DocumentModel`] from raw resume text.
///
/// The first non-section line that looks like a name becomes the header name;
/// contact lines before the first section become the header contact runs.
/// Everything after is grouped under its section heading. Content appearing
/// before any heading is kept in a leading [`SectionId::Summary`] section with an
/// empty heading (so the layout engine renders the body with no visible title).
pub fn model_from_resume_text(text: &str) -> DocumentModel {
    let parsed = parse_resume(text);

    let mut name: Option<String> = None;
    let mut contact_parts: Vec<String> = Vec::new();
    let mut seen_section = false;

    let mut sections: Vec<Section> = Vec::new();
    let mut preamble: Vec<Block> = Vec::new();
    let mut current: Option<Section> = None;
    let mut entry: Option<EntryBlock> = None;

    for line in &parsed.lines {
        match line.kind {
            LineKind::Blank => {}

            LineKind::Name => {
                if !seen_section && name.is_none() {
                    name = Some(line.text.clone());
                } else {
                    // A stray name-like line inside the body: keep it, don't drop it.
                    flush_entry(&mut entry, &mut current, &mut preamble);
                    push_block(
                        Block::Paragraph(tokenize_rich(&line.raw)),
                        &mut current,
                        &mut preamble,
                    );
                }
            }

            LineKind::Contact => {
                if !seen_section {
                    // Header contact: keep the raw (un-stripped) line so markdown
                    // links `[label](url)` tokenize into clickable runs.
                    contact_parts.push(line.raw.clone());
                } else {
                    flush_entry(&mut entry, &mut current, &mut preamble);
                    push_block(
                        Block::Paragraph(tokenize_rich(&line.raw)),
                        &mut current,
                        &mut preamble,
                    );
                }
            }

            LineKind::SectionHeader => {
                flush_entry(&mut entry, &mut current, &mut preamble);
                if let Some(section) = current.take() {
                    sections.push(section);
                }
                seen_section = true;
                current = Some(Section {
                    id: SectionId::from_header(&line.text),
                    heading: line.text.clone(),
                    blocks: Vec::new(),
                });
            }

            LineKind::JobEntry => {
                flush_entry(&mut entry, &mut current, &mut preamble);
                entry = Some(EntryBlock {
                    // `line.text` is the left-hand part (the date is `right_text`).
                    title: tokenize_rich(&line.text),
                    subtitle: None,
                    date: line.right_text.clone(),
                    bullets: Vec::new(),
                });
            }

            LineKind::JobTitle => {
                // Attach as the subtitle of the open entry if it doesn't have one.
                let attached = match entry.as_mut() {
                    Some(e) if e.subtitle.is_none() => {
                        e.subtitle = Some(tokenize_rich(&line.raw));
                        true
                    }
                    _ => false,
                };
                if !attached {
                    flush_entry(&mut entry, &mut current, &mut preamble);
                    push_block(
                        Block::Paragraph(tokenize_rich(&line.raw)),
                        &mut current,
                        &mut preamble,
                    );
                }
            }

            LineKind::Bullet => match entry.as_mut() {
                Some(e) => e.bullets.push(tokenize_rich(&line.raw)),
                None => push_block(
                    Block::Bullet(tokenize_rich(&line.raw)),
                    &mut current,
                    &mut preamble,
                ),
            },

            LineKind::Text => {
                flush_entry(&mut entry, &mut current, &mut preamble);
                push_block(
                    Block::Paragraph(tokenize_rich(&line.raw)),
                    &mut current,
                    &mut preamble,
                );
            }
        }
    }

    // Final flush of any open entry / section.
    flush_entry(&mut entry, &mut current, &mut preamble);
    if let Some(section) = current.take() {
        sections.push(section);
    }

    // Leading content under no heading becomes an untitled Summary section so it
    // renders first without an invented heading.
    if !preamble.is_empty() {
        sections.insert(
            0,
            Section {
                id: SectionId::Summary,
                heading: String::new(),
                blocks: preamble,
            },
        );
    }

    let contact = if contact_parts.is_empty() {
        Vec::new()
    } else {
        tokenize_rich(&contact_parts.join(" · "))
    };

    let mut model = DocumentModel::new(DocumentType::Resume);
    model.header = HeaderBlock {
        name: name.unwrap_or_default(),
        title: None,
        contact,
    };
    model.sections = sections;
    model
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Flatten a RichText into its visible string for concise assertions.
    fn flat(rt: &super::super::rich::RichText) -> String {
        rt.iter().map(|r| r.text.as_str()).collect()
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
        let ids: Vec<&SectionId> = m.sections.iter().map(|s| &s.id).collect();
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
        let experience = m
            .sections
            .iter()
            .find(|s| s.id == SectionId::Experience)
            .expect("experience section");

        // Two entries: Acme then Beta.
        let entries: Vec<&EntryBlock> = experience
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::Entry(e) => Some(e),
                _ => None,
            })
            .collect();
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
        let skills = m
            .sections
            .iter()
            .find(|s| s.id == SectionId::Skills)
            .expect("skills section");
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
        assert_eq!(m.schema_version, super::super::version::SCHEMA_VERSION);
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
}
