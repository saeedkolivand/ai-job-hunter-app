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

/// Bug 2 (PR#998 regression): the source résumé has no projects, and the
/// generated text — realistically, from a model that still wrote the
/// heading despite the prompt fix — carries a "PROJECTS" heading with
/// nothing under it before the next real section. The produced
/// [`DocumentModel`] (what actually gets rendered) must not carry a
/// Projects section at all. Anchored on the model the PDF/DOCX backends
/// consume, not on the prompt string.
#[test]
fn a_heading_with_nothing_under_it_never_reaches_the_document_model() {
    let generated = "Jane Doe\njane@example.com\n\n\
                      EXPERIENCE\nAcme Corp  2020 - Present\n- Shipped things\n\n\
                      PROJECTS\n\n\
                      SKILLS\n- Rust, TypeScript\n";
    let m = model_from_resume_text(generated);
    let ids: Vec<&SectionId> = m.sections.iter().map(|s| &s.id).collect();
    assert!(
        !ids.contains(&&SectionId::Projects),
        "an empty Projects heading must not survive into the rendered model; got {ids:?}"
    );
    assert_eq!(
        ids,
        vec![&SectionId::Experience, &SectionId::Skills],
        "Experience and Skills, the two sections with real content, are untouched"
    );
}

/// The other half of the same guard: a section that is merely TERSE — one
/// short line, not zero — must survive. Otherwise the empty-section drop
/// would destroy a legitimate one-entry Publications/Awards section along
/// with the genuinely empty ones.
#[test]
fn a_terse_one_line_section_is_not_mistaken_for_an_empty_one() {
    let generated = "Jane Doe\njane@example.com\n\n\
                      PUBLICATIONS\nDoe, J. (2022). A short paper.\n\n\
                      SKILLS\n- Rust\n";
    let m = model_from_resume_text(generated);
    let publications = m
        .sections
        .iter()
        .find(|s| s.id == SectionId::Publications)
        .expect("the one-line Publications section must survive");
    assert_eq!(publications.blocks.len(), 1);
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

/// Comma + parenthesized date format (AI documented output) yields
/// Block::Entry (bold title) not Block::Paragraph (non-bold text).
#[test]
fn comma_paren_date_yields_entry_block() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE
Senior Engineer, Acme Corp (January 2021 \u{2013} March 2023)
- Led a team of five engineers
- Shipped three major features
";
    let m = model_from_resume_text(resume);
    let experience = m
        .sections
        .iter()
        .find(|s| s.id == SectionId::Experience)
        .expect("experience section must be present");

    let entries: Vec<&EntryBlock> = experience
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Entry(e) => Some(e),
            _ => None,
        })
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "expected one Entry block for the job header"
    );

    let title_text = flat(&entries[0].title);
    assert!(
        title_text.contains("Senior Engineer"),
        "entry title must contain the role; got: {title_text:?}"
    );
    assert!(
        title_text.contains("Acme Corp"),
        "entry title must contain the company; got: {title_text:?}"
    );
    assert!(
        title_text.contains("January 2021"),
        "entry title must contain the date (whole line is bold); got: {title_text:?}"
    );
    assert!(
        entries[0].date.is_none(),
        "date must be None for comma+paren format (date is embedded in title); got: {:?}",
        entries[0].date
    );
    assert_eq!(
        entries[0].bullets.len(),
        2,
        "both bullets must attach to the entry"
    );
}

/// Pipe-separated with a date segment yields Block::Entry (bold title).
#[test]
fn pipe_date_segment_yields_entry_block() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE
Principal Engineer | Meridian Systems | 2019 \u{2013} Present
- Scaled the platform to 500 k events per second
";
    let m = model_from_resume_text(resume);
    let experience = m
        .sections
        .iter()
        .find(|s| s.id == SectionId::Experience)
        .expect("experience section must be present");

    let entries: Vec<&EntryBlock> = experience
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Entry(e) => Some(e),
            _ => None,
        })
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "expected one Entry block for the pipe-date line"
    );

    let title_text = flat(&entries[0].title);
    assert!(
        title_text.contains("Principal Engineer"),
        "entry title must contain role; got: {title_text:?}"
    );
    assert!(
        title_text.contains("Meridian Systems"),
        "entry title must contain company; got: {title_text:?}"
    );
    assert!(
        entries[0].date.is_none(),
        "date must be None for pipe-date format (date is embedded in title); got: {:?}",
        entries[0].date
    );
    assert_eq!(
        entries[0].bullets.len(),
        1,
        "bullet must attach to the entry"
    );
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
    let summary = m
        .sections
        .iter()
        .find(|s| s.id == SectionId::Summary)
        .expect("summary section");
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

/// CRITICAL regression: `**bold**` inside a bullet must survive as a real
/// bold [`TextRun`](super::super::rich::TextRun), not just have its `**`
/// markers stripped. `BULLET_RE` used to capture the bullet's text from
/// the already-markdown-stripped `clean` string, so `raw`/`segments`
/// never saw the `**` in the first place — every bullet lost bold, in
/// every template, PDF and DOCX alike.
#[test]
fn bullet_with_bold_marker_produces_a_bold_text_run() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE
Acme Corp  2020 - Present
- Migrated to **Rust** and cut latency
";
    let m = model_from_resume_text(resume);
    let experience = m
        .sections
        .iter()
        .find(|s| s.id == SectionId::Experience)
        .expect("experience section");
    let entry = experience
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Entry(e) => Some(e),
            _ => None,
        })
        .expect("job entry");
    let bullet = &entry.bullets[0];
    assert!(
        bullet.iter().any(|r| r.bold && r.text == "Rust"),
        "expected a bold \"Rust\" run, got {bullet:?}"
    );
}

/// CRITICAL regression: a job-entry TITLE carrying `**bold**` must also
/// survive — the two-space-gap `JobEntry` arm computed its title from the
/// stripped `clean` string too, and the adapter tokenized `line.text`
/// (also stripped) rather than a markdown-preserving field.
#[test]
fn job_entry_title_with_bold_marker_produces_a_bold_text_run() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE
**Acme Corp**  2020 - Present
- Led the platform team
";
    let m = model_from_resume_text(resume);
    let experience = m
        .sections
        .iter()
        .find(|s| s.id == SectionId::Experience)
        .expect("experience section");
    let entry = experience
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Entry(e) => Some(e),
            _ => None,
        })
        .expect("job entry");
    assert!(
        entry.title.iter().any(|r| r.bold && r.text == "Acme Corp"),
        "expected a bold \"Acme Corp\" title run, got {:?}",
        entry.title
    );
    assert_eq!(entry.date.as_deref(), Some("2020 - Present"));
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

/// Owner-reported regression: the title/company line carries no date at
/// all (separated from the role by a middot, not a comma/pipe/paren), and
/// the date range + location sit on their OWN following line. Before the
/// next-line-date `JobEntry` branches, NEITHER line matched any recognized
/// job-entry shape, so the whole entry silently rendered as two unrelated
/// plain (non-bold) paragraphs instead of a structured, bold entry with a
/// distinguishable date and location subtitle.
#[test]
fn title_middot_company_then_bare_date_line_yields_structured_entry() {
    let resume = "\
Jane Doe
jane@example.com

EXPERIENCE
Senior Frontend Developer \u{b7} ACTINEO GmbH
December 2022 \u{2013} November 2025, K\u{f6}ln, Deutschland
- Built scalable applications
";
    let m = model_from_resume_text(resume);
    let experience = m
        .sections
        .iter()
        .find(|s| s.id == SectionId::Experience)
        .expect("experience section");

    let entries: Vec<&EntryBlock> = experience
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Entry(e) => Some(e),
            _ => None,
        })
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "expected exactly one structured entry, got blocks: {:?}",
        experience.blocks
    );
    let entry = entries[0];
    assert_eq!(
        flat(&entry.title),
        "Senior Frontend Developer \u{b7} ACTINEO GmbH"
    );
    assert_eq!(
        entry.date.as_deref(),
        Some("December 2022 \u{2013} November 2025")
    );
    assert_eq!(
        entry.subtitle.as_ref().map(flat).as_deref(),
        Some("K\u{f6}ln, Deutschland")
    );
    assert_eq!(entry.bullets.len(), 1);
    assert_eq!(flat(&entry.bullets[0]), "Built scalable applications");
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

// ── Projects: bold title / tech-stack subtitle / description ──────────────

/// The locked project signature `pipeline::resume::project_render` emits:
/// bold name + links, a `·`-separated stack line, then prose. Two projects,
/// so entry GROUPING is under test and not just a single lucky line.
const PROJECTS: &str = "\
PROJECTS

**Ledger CLI** · https://github.com/janedoe/ledger
Rust · SQLite · Clap
A double-entry bookkeeping tool for the terminal.

**Atlas** · https://atlas.example.dev
TypeScript · React
Framework-agnostic component library published to npm.
";

fn entries(model: &DocumentModel) -> Vec<&EntryBlock> {
    model
        .sections
        .iter()
        .flat_map(|s| &s.blocks)
        .filter_map(|b| match b {
            Block::Entry(e) => Some(e),
            _ => None,
        })
        .collect()
}

#[test]
fn project_lines_regroup_into_entries_with_a_stack_subtitle() {
    let m = model_from_resume_text(PROJECTS);
    let found = entries(&m);
    assert_eq!(found.len(), 2, "one entry per project, got {found:?}");

    // Absolute expected strings — not a comparison against another value
    // derived from the same parse, which would stay green if BOTH drifted.
    // `tokenize_rich` shows a bare URL without its scheme; the href itself
    // stays intact (asserted in `project_title_keeps_its_bold_run_and_link`).
    assert_eq!(
        flat(&found[0].title),
        "Ledger CLI · github.com/janedoe/ledger"
    );
    assert_eq!(
        found[0].subtitle.as_ref().map(flat).as_deref(),
        Some("Rust · SQLite · Clap"),
        "the tech line must land in the subtitle slot every template styles"
    );
    assert_eq!(
        found[0].bullets.iter().map(flat).collect::<Vec<_>>(),
        vec!["A double-entry bookkeeping tool for the terminal."]
    );
    assert_eq!(found[0].date, None, "projects carry no date column");

    // The second project proves the first entry was CLOSED, not extended.
    assert_eq!(flat(&found[1].title), "Atlas · atlas.example.dev");
    assert_eq!(
        found[1].subtitle.as_ref().map(flat).as_deref(),
        Some("TypeScript · React"),
        "a two-item stack has only ONE separator — it must still be a stack"
    );
    assert_eq!(
        found[1].bullets.iter().map(flat).collect::<Vec<_>>(),
        vec!["Framework-agnostic component library published to npm."]
    );
}

#[test]
fn project_title_keeps_its_bold_run_and_link() {
    let m = model_from_resume_text(PROJECTS);
    let title = &entries(&m)[0].title;
    assert!(
        title
            .iter()
            .any(|r| r.bold && r.text.contains("Ledger CLI")),
        "the project NAME must stay bold: {title:?}"
    );
    assert!(
        title
            .iter()
            .any(|r| r.link.as_deref() == Some("https://github.com/janedoe/ledger")),
        "the project link must stay clickable: {title:?}"
    );
}

/// The regression guard that matters: the identical shapes under any OTHER
/// heading must keep rendering exactly as they did before this change.
#[test]
fn the_same_shapes_under_experience_are_untouched() {
    let text = PROJECTS.replacen("PROJECTS", "EXPERIENCE", 1);
    let m = model_from_resume_text(&text);
    assert!(
        entries(&m).is_empty(),
        "no Projects section, so no project regrouping may happen"
    );
    let paragraphs = m
        .sections
        .iter()
        .flat_map(|s| &s.blocks)
        .filter(|b| matches!(b, Block::Paragraph(_)))
        .count();
    assert_eq!(paragraphs, 6, "all six lines stay paragraphs");
}

#[test]
fn a_prose_only_projects_section_stays_paragraphs() {
    let m = model_from_resume_text(
        "PROJECTS\n\nBuilt an internal deploy tool used by the whole team.\n",
    );
    assert!(entries(&m).is_empty(), "nothing bold-led opens an entry");
    assert_eq!(m.sections[0].blocks.len(), 1);
    assert!(matches!(m.sections[0].blocks[0], Block::Paragraph(_)));
}

/// A `·`-bearing sentence AFTER the description must not be mistaken for a
/// second stack line — only the line directly under the title can be one.
#[test]
fn only_the_line_under_the_title_can_be_the_stack() {
    let m = model_from_resume_text(
        "PROJECTS\n\n\
         **Ledger CLI** · https://github.com/janedoe/ledger\n\
         Rust · SQLite\n\
         Ships on Windows · macOS · Linux\n",
    );
    let found = entries(&m);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].subtitle.as_ref().map(flat).as_deref(),
        Some("Rust · SQLite")
    );
    assert_eq!(
        found[0].bullets.iter().map(flat).collect::<Vec<_>>(),
        vec!["Ships on Windows · macOS · Linux"],
        "the second separator line is body content, not a second subtitle"
    );
}

/// The `bullets.is_empty()` half of the stack guard, which the "only the
/// line under the title" case above does NOT reach (there the subtitle slot
/// is already taken). A project with NO stack line has an empty subtitle for
/// its whole run, so without this half a later `·`-bearing sentence would be
/// hoisted into the subtitle slot and RENDER ABOVE the description it
/// followed — reordering the candidate's own prose.
#[test]
fn a_separator_line_after_the_description_is_never_hoisted() {
    let m = model_from_resume_text(
        "PROJECTS\n\n\
         **Ledger CLI** · https://github.com/janedoe/ledger\n\
         A double-entry bookkeeping tool for the terminal.\n\
         Ships on Windows · macOS · Linux\n",
    );
    let found = entries(&m);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].subtitle, None, "this project has no stack line");
    assert_eq!(
        found[0].bullets.iter().map(flat).collect::<Vec<_>>(),
        vec![
            "A double-entry bookkeeping tool for the terminal.",
            "Ships on Windows · macOS · Linux",
        ],
        "body order must survive verbatim"
    );
}

/// A résumé that puts its project links on their OWN line, rather than on the
/// title line the locked signature uses, must not have that link line styled
/// as the technology list. It stays body content; the entry simply has no
/// subtitle, which is what it rendered as before this feature existed.
#[test]
fn a_link_line_under_the_title_is_never_styled_as_the_tech_list() {
    let m = model_from_resume_text(
        "PROJECTS\n\n\
         **Ledger CLI**\n\
         Demo · https://example.dev\n\
         A double-entry bookkeeping tool for the terminal.\n",
    );
    let found = entries(&m);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].subtitle, None,
        "a link line must not be mistaken for a technology stack"
    );
    assert_eq!(
        found[0].bullets.iter().map(flat).collect::<Vec<_>>(),
        vec![
            "Demo · example.dev",
            "A double-entry bookkeeping tool for the terminal.",
        ],
        "the link stays body content, in source order"
    );
}

/// An IMPORTED résumé carries no markdown: PDF and DOCX extraction keeps the
/// words and drops the bold. A candidate's own CV with a perfectly-formed
/// project block therefore has no `**` anywhere, and a bold-only opener
/// rendered the whole section as loose paragraphs — the exact flat output
/// this feature exists to remove. Real shape, taken from an imported CV.
#[test]
fn a_project_title_is_recognized_without_markdown_bold() {
    let m = model_from_resume_text(
        "PROJECTS

         AI Job Hunter   aijobhunter.app
         Tauri 2 · Rust · React 19 · TypeScript
         Local-first Windows and macOS desktop application with local SQLite storage.

         CrossKit   crosskit.iamsaeed.dev
         TypeScript · React · Vue
         Framework-agnostic component library published to npm.
",
    );
    let found = entries(&m);
    assert_eq!(found.len(), 2, "one entry per project, got {found:?}");
    assert_eq!(flat(&found[0].title), "AI Job Hunter   aijobhunter.app");
    assert_eq!(
        found[0].subtitle.as_ref().map(flat).as_deref(),
        Some("Tauri 2 · Rust · React 19 · TypeScript")
    );
    assert_eq!(
        found[0].bullets.iter().map(flat).collect::<Vec<_>>(),
        vec!["Local-first Windows and macOS desktop application with local SQLite storage."]
    );
    assert_eq!(flat(&found[1].title), "CrossKit   crosskit.iamsaeed.dev");
    assert_eq!(
        found[1].subtitle.as_ref().map(flat).as_deref(),
        Some("TypeScript · React · Vue")
    );
}

/// The shape fallback must not fire on ordinary prose. A description line is
/// only ever followed by more prose or the next title, never by a stack.
#[test]
fn prose_followed_by_prose_never_opens_an_entry() {
    let m = model_from_resume_text(
        "PROJECTS

         Built an internal deploy tool used by the whole team
         and documented it for the on-call rotation.
",
    );
    assert!(entries(&m).is_empty(), "no stack line, so no entry opens");
}

/// The shape signal must not read across a section boundary. A
/// separator-bearing HEADING (`SKILLS · TOOLS`, and German/French headings
/// like `KENNTNISSE · SPRACHEN` are the same shape) sits right after the last
/// line of Projects, and looking past the heading made that line a title.
#[test]
fn the_shape_signal_never_looks_past_a_section_heading() {
    let m = model_from_resume_text(
        "PROJECTS

         Ledger CLI   example.dev
         Rust · SQLite
         A bookkeeping tool I maintain

         SKILLS · TOOLS
         Rust, Python
",
    );
    let found = entries(&m);
    assert_eq!(found.len(), 1, "one project, got {found:?}");
    assert_eq!(
        found[0].bullets.iter().map(flat).collect::<Vec<_>>(),
        vec!["A bookkeeping tool I maintain"],
        "the last line stays this project's body"
    );
}

/// A line INSIDE an entry must not hijack a following stack line. Entries are
/// blank-separated and a description line never is, which is the signal that
/// separates the two: `Used by 200 teams` above a SECOND stack is prose.
#[test]
fn an_unpunctuated_body_line_above_a_stack_is_not_a_title() {
    let m = model_from_resume_text(
        "PROJECTS

         Ledger CLI   example.dev
         Rust · SQLite
         Used by 200 teams
         Go · gRPC · Redis
",
    );
    let found = entries(&m);
    assert_eq!(found.len(), 1, "one project, got {found:?}");
    assert_eq!(
        found[0].subtitle.as_ref().map(flat).as_deref(),
        Some("Rust · SQLite"),
        "the FIRST stack stays the technology line"
    );
    assert_eq!(
        found[0].bullets.iter().map(flat).collect::<Vec<_>>(),
        vec!["Used by 200 teams", "Go · gRPC · Redis"],
        "both later lines stay body content, in order"
    );
}

/// A generator told to omit a section it has no content for sometimes writes
/// the heading anyway plus a note explaining the absence. Reported from a
/// real export: two headings advertising that the candidate has no awards
/// and no publications, which is worse than printing neither.
#[test]
fn a_section_that_is_only_a_parenthetical_note_is_dropped() {
    let m = model_from_resume_text(
        "PROJEKTE\n\n\
         Ledger CLI   example.dev\n\
         Rust · SQLite\n\
         Ein Buchhaltungswerkzeug.\n\n\
         AUSZEICHNUNGEN\n\
         (Keine Auszeichnungen im vorliegenden Lebenslauf)\n\n\
         PUBLIKATIONEN\n\
         (Keine Publikationen im vorliegenden Lebenslauf)\n",
    );
    let headings: Vec<&str> = m.sections.iter().map(|s| s.heading.as_str()).collect();
    assert_eq!(
        headings,
        vec!["PROJEKTE"],
        "both placeholder sections must be gone"
    );
}

/// Shape, not wording, so it holds in any language. A section with REAL
/// content that merely CONTAINS a parenthetical must survive.
#[test]
fn a_section_with_real_content_survives_a_parenthetical() {
    let m = model_from_resume_text(
        "AWARDS\n\n\
         Employee of the Year (2024)\n\n\
         PUBLICATIONS\n\
         (None)\n",
    );
    let headings: Vec<&str> = m.sections.iter().map(|s| s.heading.as_str()).collect();
    assert_eq!(headings, vec!["AWARDS"]);
}

/// The placeholder test must be ONE parenthetical spanning the paragraph, not
/// merely "starts with ( and ends with )". `(B.Sc.) Computer Science (2020)`
/// satisfies the naive test and is ordinary Education content \u2014 dropping its
/// section would silently delete a real qualification.
#[test]
fn a_paragraph_that_merely_starts_and_ends_with_parens_is_not_a_placeholder() {
    for body in [
        "(B.Sc.) Computer Science (2020)",
        "(Remote) Senior Engineer, Berlin (2021)",
        ") stray close and an open (",
    ] {
        let m = model_from_resume_text(&format!("EDUCATION\n\n{body}\n"));
        let headings: Vec<&str> = m.sections.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(
            headings,
            vec!["EDUCATION"],
            "must keep the section for {body:?}"
        );
    }

    // The real placeholder shape still goes.
    let m = model_from_resume_text("AWARDS\n\n(None in the present résumé)\n");
    assert!(
        m.sections.is_empty(),
        "a single parenthetical is still dropped"
    );
}

/// A2: the same text under a German heading takes the same path. Before this
/// change `Projekte` classified as `Custom` and the whole feature was
/// silently English-only.
#[test]
fn a_localized_projects_heading_takes_the_same_path() {
    for heading in ["PROJEKTE", "PROJETS", "PROYECTOS", "PROGETTI", "PROJECTEN"] {
        let text = PROJECTS.replacen("PROJECTS", heading, 1);
        let m = model_from_resume_text(&text);
        assert_eq!(
            m.sections[0].id,
            SectionId::Projects,
            "{heading} must classify as Projects"
        );
        assert_eq!(entries(&m).len(), 2, "{heading} must regroup its entries");
    }
}
