use super::{support::*, *};

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
    let ids = section_ids(&m);
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
    let publications = find_section(&m, SectionId::Publications)
        .expect("the one-line Publications section must survive");
    assert_eq!(publications.blocks.len(), 1);
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
    let headings = section_headings(&m);
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
    let headings = section_headings(&m);
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
        let headings = section_headings(&m);
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
