use super::{support::*, *};

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
