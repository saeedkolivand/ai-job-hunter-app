use super::super::prompts::draft_system;

/// The draft prompt is localized off the SAME `resume_conventions` the renderer
/// path uses, so a German run asks for German headings — including the FIVE
/// sections (Projects, Certifications, Languages, Awards, Publications) that
/// used to fall through to their raw English `SectionId` debug word inside
/// the "Sections run in this order" list even though the prompt told the
/// model to write German. A German CV that has real content for all nine
/// sections must never see an English section name anywhere in this prompt.
///
/// Mutation check: swap `resume_conventions(lang)` for `resume_conventions("en")`
/// only inside `section_order_prompt_list` (leaving the four-heading line
/// localized) — RAN, went red (`Projekte`/`Zertifikate` missing, `Projects`/
/// `Certifications` present instead), reverted.
#[test]
fn the_draft_prompt_localizes_its_headings() {
    let german = draft_system("de-DE", "de");
    assert!(german.contains("Berufserfahrung"));
    assert!(german.contains("Kenntnisse"));
    assert!(!german.contains("Work Experience"));

    // The order list localizes every section, not just the four headings.
    assert!(
        german.contains("Projekte"),
        "Projects must be localized in the order list"
    );
    assert!(
        german.contains("Zertifikate"),
        "Certifications must be localized in the order list"
    );
    assert!(
        german.contains("Sprachen"),
        "Languages must be localized in the order list"
    );
    assert!(
        german.contains("Auszeichnungen"),
        "Awards must be localized in the order list"
    );
    assert!(
        german.contains("Publikationen"),
        "Publications must be localized in the order list"
    );
    assert!(!german.contains("Projects"));
    assert!(!german.contains("Certifications"));
    assert!(!german.contains("Publications"));
}

/// The anti-merge clause PR #1003 dropped when it reworded the order
/// instruction into "an ORDER, not a checklist" — without it, a model reading
/// "omit any section you have nothing for" as licence to also MERGE two
/// sections under one joined heading ("Ausbildung & Sprachen") rather than
/// writing each on its own line or omitting one outright.
///
/// Mutation check: delete the "One heading per section" bullet — RAN, went
/// red (no "Never combine two sections" text in the output), reverted.
#[test]
fn the_draft_prompt_forbids_merged_section_headings() {
    for (lang, market) in [("en", "us"), ("de", "de")] {
        let prompt = draft_system(lang, market);
        assert!(
            prompt.contains("Never combine two sections under a joined heading"),
            "{lang}/{market}: the anti-merge clause must survive"
        );
        assert!(
            prompt.contains("Ausbildung & Sprachen"),
            "{lang}/{market}: the worked (bad) example must survive, or the rule reads as abstract"
        );
    }
}

/// The section order is a FIXED instruction resolved from `market`, not left
/// to the model — and the two markets really do disagree, so this can't pass
/// The skills section's SHAPE is the application's decision, not the model's.
///
/// Nothing used to specify it — `resume_conventions.header("Skills")` is only
/// the localized heading ("Kenntnisse"), so the model picked, and the repo's own
/// fixtures disagree (one middot-separated, one comma-separated). One bullet
/// per skill spends a line on a single word; a twenty-skill list becomes twenty
/// lines against a one-to-two-page budget, and an ATS extracts a comma list
/// exactly as well.
///
/// Asserted against the BUILT PROMPT, not against `resume_conventions`
/// returning a label — asserting the ingredient rather than the recipe is the
/// shape that has let fourteen tests on this branch pass while the thing they
/// guarded was broken. And the prohibition is pinned as well as the form: the
/// empty-sections defect this branch fixes came from a prompt that described a
/// shape and was read as licence to do otherwise.
#[test]
fn the_draft_prompt_fixes_the_skills_section_shape() {
    for (lang, market) in [("en", "us"), ("de", "de")] {
        let prompt = draft_system(lang, market);
        assert!(
            prompt.contains("grouped INLINE lists"),
            "{lang}/{market}: the prompt must state the grouped-inline shape"
        );
        assert!(
            prompt.contains("never one bullet per skill"),
            "{lang}/{market}: the prohibition must be explicit — a described shape alone was read as a manifest once already on this branch"
        );
        assert!(
            prompt.contains("Languages: Rust, Go, TypeScript"),
            "{lang}/{market}: the worked example must survive, or the instruction is abstract enough for a small model to ignore"
        );
    }
}

/// by accident with a single hardcoded order string.
///
/// Mutation check: hardcode `draft_system`'s order text instead of reading
/// `locale::resume::section_order_for(market)` and the "de" branch's
/// assertions fail (the string would stay the US order for every market).
#[test]
fn the_draft_prompt_injects_the_market_resolved_section_order() {
    let us = draft_system("en", "us");
    assert!(us.contains("Professional Summary, Work Experience, Skills, Projects, Education"));

    let de = draft_system("de", "de");
    assert!(de.contains("Profil, Berufserfahrung, Ausbildung, Zertifikate, Kenntnisse"));

    // Both markets lead with Experience, so Skills-vs-Education is what
    // actually discriminates them — assert each market lacks the other's
    // signature, or this passes on two identical strings.
    assert!(us.contains("Skills, Projects, Education"));
    assert!(!us.contains("Ausbildung, Zertifikate, Kenntnisse"));
    assert!(!de.contains("Skills, Projects, Education"));
}
