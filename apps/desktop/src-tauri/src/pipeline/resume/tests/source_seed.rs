/// `seed_one_project`'s stack-line filter used to drop an item whenever
/// `urls_in` found ANY match, with no `names_a_resource` guard — unlike its
/// two siblings in the same function (the labeled-span and bare-URL link
/// capture), which both skip a stack-line match that doesn't name a
/// resource. `crates.io` written bare in a stack line matches `URL_RE`'s
/// code-host arm (it is a known registry host) but names no resource (no
/// scheme, `www.`, or path), so it used to be silently deleted from the
/// candidate's own stack line instead of kept as the technology it is.
///
/// Mutation check: reverting the filter to
/// `!item.is_empty() && urls_in(item).is_empty()` drops `crates.io` from
/// `stack` and this assertion goes red — verified — then restoring the
/// `names_a_resource` guard makes it green again.
#[test]
fn a_bare_code_host_stack_token_survives_as_a_technology_not_a_link() {
    let source = "PROJECTS\n\n\
                  **Ledger CLI** · https://ledger.example.dev\n\
                  Rust · crates.io · Tokio\n\
                  A command-line ledger tool.\n";
    let projects = crate::pipeline::resume::source::seed_projects(source);
    assert_eq!(projects.len(), 1, "exactly one project must be seeded");
    assert_eq!(
        projects[0].stack,
        vec![
            "Rust".to_string(),
            "crates.io".to_string(),
            "Tokio".to_string()
        ],
        "a bare code-host token in the stack line is a technology, not a \
         claimed link; got {:?}",
        projects[0].stack
    );
}

/// An IMPORTED résumé has no markdown — PDF/DOCX extraction keeps the words and
/// drops the bold — so before the shape signal existed, `source::entries`
/// collapsed a multi-project section into ONE mega-entry whose description
/// swallowed the next project's title and stack. Measured on the real shape a
/// user reported: three projects seeded as one, with `link_in_description`
/// then correctly refusing to normalize anything at all.
#[test]
fn a_plain_prose_projects_section_seeds_one_entry_per_project() {
    let source = "PROJECTS

AI Job Hunter   aijobhunter.app
Tauri 2 · Rust · React 19 · TypeScript
Local-first Windows and macOS desktop application with local SQLite storage.

CrossKit   crosskit.iamsaeed.dev
TypeScript · React · Vue
Framework-agnostic component library published to npm.
";
    let seeded = super::super::source::seed_projects(source);
    assert_eq!(seeded.len(), 2, "one seed per project, got {seeded:?}");
    assert_eq!(
        seeded[0].stack,
        vec!["Tauri 2", "Rust", "React 19", "TypeScript"]
    );
    assert_eq!(
        seeded[0].description,
        "Local-first Windows and macOS desktop application with local SQLite storage.",
        "the next project's title must not be swallowed into this description"
    );
    assert_eq!(seeded[1].stack, vec!["TypeScript", "React", "Vue"]);
    assert_eq!(
        seeded[1].description,
        "Framework-agnostic component library published to npm."
    );

    // The whole-bail guards must now see plausible seeds rather than refusing
    // to normalize every imported résumé.
    let (seeds, bail) = super::super::projects::seed_projects_for_normalize(source);
    assert_eq!(bail, None, "no bail reason expected, got {bail:?}");
    assert_eq!(seeds.len(), 2);
}

/// The LIMIT of the shape signal, pinned so it is not mistaken for fixed: the
/// signal is "a line sitting directly above a technology stack", so a Projects
/// section with NO stack lines anywhere has nothing to key on and still
/// collapses into one mega-entry.
///
/// Pre-existing and unchanged by the shape signal — this shape had no bold, no
/// bullet and no stack before it either. Recorded because the collapse is NOT
/// caught by the whole-bail guards: the merged description carries no URL, and
/// a single collapsed seed has no sibling for `seeds_are_plausible` to compare
/// against. Normalization would run from this garbled seed.
#[test]
fn a_projects_section_with_no_stack_lines_still_collapses() {
    let source = "PROJECTS

Ledger CLI
A double-entry bookkeeping tool for the terminal that I built and maintain.

Beta Sync
A small file synchroniser used by a handful of people.
";
    let (seeds, bail) = super::super::projects::seed_projects_for_normalize(source);
    assert_eq!(bail, None);
    assert_eq!(seeds.len(), 1, "no stack line anywhere, so no shape signal");
    assert!(
        seeds[0].description.contains("Beta Sync"),
        "the second project is still swallowed: {:?}",
        seeds[0].description
    );
}
