//! Shape-based heading recognition (no name list involved): the generic
//! ALL-CAPS rule, letter-spaced-heading collapsing, and the "blast radius"
//! guards proving the new section vocabulary doesn't misfire on ordinary
//! company names, bullets, or prose.

use crate::export::parser::headings::despace_letterspaced;
use crate::export::parser::{is_all_caps_section_heading, parse_line, parse_resume, strip_md};
use crate::export::types::LineKind;

#[test]
fn test_all_caps_section() {
    let line = parse_line("EXPERIENCE", 5, &[]);
    assert!(matches!(line.kind, LineKind::SectionHeader));
}

#[test]
fn is_all_caps_section_heading_matches_ts_fixture() {
    // Cross-language parity guard for the shape-based (not list-based)
    // heading predicate — this is what recognizes a locale's own ALL-CAPS
    // heading (the résumé prompt mandates ALL-CAPS section titles) without a
    // per-locale word list, and what an unfixtured/unrecognised locale falls
    // back to when it isn't literally in SECTION_NAMES. A previous version of
    // this predicate was deleted from the TS mirror without a fixture gate,
    // which silently broke header-seeding for es/it/nl/pt résumés (and any
    // en résumé whose first heading — "PROFESSIONAL EXPERIENCE", "KEY
    // ACHIEVEMENTS" — isn't literally in SECTION_NAMES either); restoring it
    // WITHOUT this gate would be the same mistake again.
    #[derive(serde::Deserialize)]
    struct Case {
        line: String,
        heading: bool,
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/prompts/src/fixtures/all-caps-headings.json");
    let raw = std::fs::read_to_string(&path).expect(
        "read all-caps-headings parity fixture \
         (packages/prompts/src/fixtures/all-caps-headings.json)",
    );
    let cases: Vec<Case> =
        serde_json::from_str(&raw).expect("parse all-caps-headings parity fixture");

    assert!(
        !cases.is_empty(),
        "all-caps-headings parity fixture must not be empty"
    );
    for c in &cases {
        // Same reasoning as the contact-line fixture above: `parse_line`
        // always runs this predicate on `strip_md(trimmed)`, never the raw
        // line, so the parity check must too.
        let clean = strip_md(c.line.trim());
        assert_eq!(
            is_all_caps_section_heading(&clean),
            c.heading,
            "is_all_caps_section_heading drift for {:?} (clean: {:?})",
            c.line,
            clean
        );
    }
}

// ── Blast radius: the new vocabulary must not fire on ordinary content ─────

/// A company literally named after a new section word, in JobEntry shape,
/// must stay a JobEntry — the exact-match test requires the WHOLE line to
/// equal the section word, so trailing text (here the date column) already
/// prevents a false positive; this pins that down for the newly added words.
#[test]
fn company_named_after_new_section_word_stays_job_entry() {
    let line = parse_line("Projekte GmbH  2020 - Present", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::JobEntry),
        "expected JobEntry, got {:?}",
        line.kind
    );
}

/// A prose bullet that STARTS with a new section word must stay a Bullet —
/// the whole line, not a prefix, has to match a known name.
#[test]
fn bullet_starting_with_new_section_word_stays_bullet() {
    let line = parse_line(
        "- Projekte für interne Kunden geleitet und Teams koordiniert",
        5,
        &[],
    );
    assert!(
        matches!(line.kind, LineKind::Bullet),
        "expected Bullet, got {:?}",
        line.kind
    );
}

/// A candidate's own prose sentence that merely CONTAINS a new section word
/// must stay Text.
#[test]
fn prose_line_containing_new_section_word_stays_text() {
    let line = parse_line("Mehrere Projekte erfolgreich abgeschlossen.", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Text),
        "expected Text, got {:?}",
        line.kind
    );
}

/// A grouped skills line labelled with a new section word ("Sprachen:") must
/// stay Text, not become a heading — the exact-match test requires the whole
/// line to equal the bare word, and a trailing colon + list is longer than that.
#[test]
fn skills_group_labelled_with_new_section_word_stays_text() {
    let line = parse_line("Sprachen: Deutsch, Englisch, Französisch", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::Text),
        "expected Text, got {:?}",
        line.kind
    );
}

/// Designers set headings with wide tracking, and some PDF producers bake it
/// into the text layer. A real CV extracted its headings as
/// `S E L E C T E D   P R O J E C T S`, which matched no heading test anywhere —
/// the section was invisible, so its projects never seeded and its links were
/// never collected.
#[test]
fn letter_spaced_headings_collapse_back_into_words() {
    assert_eq!(
        despace_letterspaced("S E L E C T E D   P R O J E C T S").as_deref(),
        Some("SELECTED PROJECTS"),
        "a double-space is a word gap, not a letter gap"
    );
    assert_eq!(
        despace_letterspaced("P R O J E K T E").as_deref(),
        Some("PROJEKTE")
    );
    assert_eq!(
        despace_letterspaced("E D U C A T I O N   &   L A N G U A G E S").as_deref(),
        Some("EDUCATION & LANGUAGES")
    );

    // Never rewrite an ordinary line.
    for plain in [
        "Projects",
        "SELECTED PROJECTS",
        "A B C", // under the four-token floor
        "Rust · SQLite · Clap",
        "Built a CLI tool for teams",
    ] {
        assert_eq!(
            despace_letterspaced(plain),
            None,
            "must not rewrite {plain:?}"
        );
    }
}

#[test]
fn a_letter_spaced_section_heading_is_recognized_and_readable() {
    let doc = "S E L E C T E D   P R O J E C T S\n\
               Ledger CLI   example.dev\n";
    let parsed = parse_resume(doc);
    let head = &parsed.lines[0];
    assert!(
        matches!(head.kind, LineKind::SectionHeader),
        "expected a section heading, got {:?}",
        head.kind
    );
    assert_eq!(
        head.text, "SELECTED PROJECTS",
        "downstream heading matching reads `text`, so it must be the collapsed form"
    );
}

/// The rewrite is gated on the collapsed form being a KNOWN heading, so a
/// letter-spaced line that is not one is left exactly as the candidate wrote it.
#[test]
fn a_letter_spaced_line_that_is_not_a_heading_is_left_alone() {
    let parsed = parse_resume("N O T   A   H E A D I N G   A T   A L L\n");
    assert_eq!(
        parsed.lines[0].text,
        "N O T   A   H E A D I N G   A T   A L L"
    );
    assert!(!matches!(parsed.lines[0].kind, LineKind::SectionHeader));
}
