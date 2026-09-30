use super::super::prompts::{repair_system, repair_user, SIBLING_CONTEXT_CAP};
use super::super::stages::sections;
use super::super::types::SectionKey;
use super::support::DRAFTED;
use crate::documents::evidence::SectionKind;

/// The anchor carries the Summary and a representative Experience bullet when
/// neither is the section being rewritten, and EXCLUDES whichever of the two
/// IS the target — the property the whole feature exists for: a section must
/// never be handed its own text as "what to match".
///
/// Mutation check: drop the `skip != SectionKind::Summary` (or `::Experience`)
/// guard in `context_anchor` and the matching exclusion assertion below fails.
#[test]
fn context_anchor_carries_siblings_and_excludes_the_target_section() {
    let split = sections::split(DRAFTED);
    let lines: Vec<&str> = DRAFTED.lines().collect();

    // Repairing SKILLS: neither sibling is the target, so both ride along.
    let anchor = sections::context_anchor(&split, &lines, SectionKind::Skills);
    assert!(anchor.contains("A payments engineer."));
    assert!(anchor.contains("Built the ledger"));

    // Repairing SUMMARY: the Summary's own text must be excluded from its own
    // anchor; the Experience bullet still anchors voice/tense.
    let anchor = sections::context_anchor(&split, &lines, SectionKind::Summary);
    assert!(
        !anchor.contains("A payments engineer."),
        "the section being rewritten must not anchor itself"
    );
    assert!(anchor.contains("Built the ledger"));

    // Repairing EXPERIENCE: the bullet is excluded; the Summary still anchors
    // language.
    let anchor = sections::context_anchor(&split, &lines, SectionKind::Experience);
    assert!(anchor.contains("A payments engineer."));
    assert!(
        !anchor.contains("Built the ledger"),
        "the section being rewritten must not anchor itself"
    );
}

/// Neither sibling survives when the document has only the section being
/// rewritten — the caller must get NO block rather than a misleading partial
/// one (`repair_system`'s `has_context` gate depends on this being truly
/// empty, not just short).
#[test]
fn context_anchor_is_empty_when_no_sibling_survives_the_exclusion() {
    let summary_only = "PROFESSIONAL SUMMARY\nA payments engineer.\n";
    let split = sections::split(summary_only);
    let lines: Vec<&str> = summary_only.lines().collect();
    assert_eq!(
        sections::context_anchor(&split, &lines, SectionKind::Summary),
        ""
    );
}

/// **PR #1003 finding 4 (MINOR).** `summary.text(lines)` used to be pushed
/// into the anchor with no bound of its own — the only bound was
/// `SIBLING_CONTEXT_CAP`, applied later by `fenced()` in `repair_user`, which
/// truncates SILENTLY. A pathological (or merely very long) Summary section
/// was therefore built here in full, and a Summary at or past the cap on its
/// own would crowd the Experience bullet pushed after it out of the fenced
/// block entirely — the LATER cap has no way to know a bullet was even meant
/// to survive alongside it.
///
/// Mutation check: drop the `.take(SIBLING_CONTEXT_CAP)` from
/// `context_anchor`'s Summary arm and this goes red — the anchor's Summary
/// half grows past the cap and the Experience bullet is crowded out.
#[test]
fn context_anchor_caps_a_pathological_summary_so_the_sibling_bullet_survives() {
    let huge_summary = "x ".repeat(SIBLING_CONTEXT_CAP); // far past the cap on its own
    let text = format!(
        "PROFESSIONAL SUMMARY\n{huge_summary}\n\nWORK EXPERIENCE\n\nAcme Payments | Senior Engineer | 2021 - Present\n- Built the ledger service\n"
    );
    let split = sections::split(&text);
    let lines: Vec<&str> = text.lines().collect();
    let anchor = sections::context_anchor(&split, &lines, SectionKind::Skills);
    assert!(
        anchor.chars().count() <= SIBLING_CONTEXT_CAP + 200,
        "the Summary half must be bounded at the layer that BUILDS the \
         anchor, not just at the later fence layer that truncates silently; \
         anchor was {} chars",
        anchor.chars().count()
    );
    assert!(
        anchor.contains("Built the ledger service"),
        "a capped Summary must leave room for the Experience bullet rather \
         than crowding it out of the anchor entirely; got {anchor:?}"
    );
}

/// **LOW finding fix.** `representative_bullet`'s fallback used to return the
/// section's LAST non-empty line with no exclusion for the heading line
/// itself — so a heading-only Experience section (no body, no bullets at all)
/// handed `context_anchor` its own heading, "WORK EXPERIENCE", as "the voice
/// to imitate" for the sibling section being rewritten.
///
/// Mutation check: drop the `.skip(1)` from `representative_bullet`'s
/// fallback and this fails — the anchor gains "WORK EXPERIENCE".
#[test]
fn context_anchor_skips_a_heading_only_experience_section_rather_than_anchoring_its_own_heading() {
    let text = "PROFESSIONAL SUMMARY\nA payments engineer.\n\nWORK EXPERIENCE\n";
    let split = sections::split(text);
    let lines: Vec<&str> = text.lines().collect();
    let anchor = sections::context_anchor(&split, &lines, SectionKind::Skills);
    assert_eq!(
        anchor, "PROFESSIONAL SUMMARY\nA payments engineer.\n",
        "a heading-only Experience section must contribute nothing, not its own heading; \
         got {anchor:?}"
    );
}

/// **The built prompt itself** — not just `context_anchor` in isolation —
/// actually carries the sibling context, fenced under `<document_context>`,
/// and that block never contains the target section's own text.
///
/// Mutation check: pass `""` instead of `context` where `repair_user` builds
/// its output and the `<document_context>` presence assertions fail; drop
/// `context_anchor`'s exclusion guard and the last assertion fails.
#[test]
fn repair_user_carries_sibling_context_and_excludes_the_target_section() {
    let split = sections::split(DRAFTED);
    let lines: Vec<&str> = DRAFTED.lines().collect();
    let skills = sections::find(&split, SectionKey::Skills).expect("skills exists");
    let skills_text = skills.text(&lines);
    let context = sections::context_anchor(&split, &lines, SectionKind::Skills);

    let prompt = repair_user(DRAFTED, &skills_text, &[], None, &context);

    assert!(prompt.contains("<document_context>"));
    assert!(
        prompt.contains("A payments engineer."),
        "the sibling Summary must ride along"
    );
    assert!(
        prompt.contains("Built the ledger"),
        "the sibling Experience bullet must ride along"
    );

    let context_block = prompt
        .split("<document_context>")
        .nth(1)
        .and_then(|rest| rest.split("</document_context>").next())
        .expect("the document_context block exists");
    assert!(
        !context_block.contains("Go, Rust"),
        "the section being rewritten (SKILLS) must not anchor itself"
    );
}

/// An empty anchor omits the block entirely — same convention as
/// `note`/`letter_date`/`company_research` — so the model is never pointed at
/// a block that isn't there.
#[test]
fn repair_user_omits_document_context_when_the_anchor_is_empty() {
    let prompt = repair_user("source", "SKILLS\nGo", &[], None, "");
    assert!(!prompt.contains("<document_context>"));
}

/// The `<document_context>` instruction is gated on `has_context`, mirroring
/// `letter_system`'s `has_date`/`has_brief` gates — a caller with nothing to
/// anchor against must not point the model at a block that will not exist.
///
/// Mutation check: default `has_context` to always-true and the first
/// assertion fails.
#[test]
fn repair_system_gates_the_document_context_instruction_on_has_context() {
    let no_context = repair_system("en", false);
    assert!(!no_context.contains("<document_context>"));

    let with_context = repair_system("en", true);
    assert!(with_context.contains("<document_context>"));
    assert!(with_context.contains("voice and tense"));
    // The output language is PINNED over the sibling context, not matched
    // from it — see `the_repair_prompt_pins_the_output_language_over_sibling_context`.
    assert!(with_context.contains("The output language is English"));
}

/// **`"header"` is rejected**, and so is every other spelling outside the closed
/// grammar. The contact header is the editor's at export time (ADR-0021), so a
/// "regenerate the header" request has no representation at all.
///
/// Mutation check: add a `Header` variant with a `"header"` arm and this fails.
#[test]
fn section_key_rejects_header_and_every_non_canonical_spelling() {
    assert_eq!(SectionKey::from_wire("header"), None);
    assert_eq!(SectionKey::from_wire("Header"), None);
    assert_eq!(SectionKey::from_wire("contact"), None);
    // The generated grammar's own rules ride along: no leading zeros, no sign,
    // no over-long value.
    assert_eq!(SectionKey::from_wire("experience:01"), None);
    assert_eq!(SectionKey::from_wire("experience:+1"), None);
    assert_eq!(SectionKey::from_wire("experience:256"), None);
    assert_eq!(SectionKey::from_wire(&"a".repeat(64)), None);

    for wire in [
        "summary",
        "skills",
        "projects",
        "education",
        "experience:0",
        "experience:255",
    ] {
        let key = SectionKey::from_wire(wire).unwrap_or_else(|| panic!("{wire} must parse"));
        assert_eq!(key.to_wire(), wire, "round-trip");
    }
}
