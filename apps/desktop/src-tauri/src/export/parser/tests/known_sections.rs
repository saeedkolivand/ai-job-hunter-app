//! The known-section-name list: exact-match recognition, multilingual
//! coverage, the "X & Y" combined-heading join, and the TS-fixture parity
//! guards that keep the Rust list and the generated-producer headings in sync.

use crate::export::parser::headings::SECTION_NAMES;
use crate::export::parser::{is_known_section_name, parse_line, strip_md};
use crate::export::types::LineKind;

#[test]
fn test_company_name_not_section() {
    let line = parse_line("NASA ENGINEER", 1, &["Name", "NASA ENGINEER"]);
    assert!(!matches!(line.kind, LineKind::SectionHeader));
}

#[test]
fn test_section_header_detection() {
    let line = parse_line("work experience", 5, &[]);
    assert!(matches!(line.kind, LineKind::SectionHeader));
}

#[test]
fn test_multilingual_sections() {
    let line = parse_line("berufserfahrung", 5, &[]);
    assert!(matches!(line.kind, LineKind::SectionHeader));
}

#[test]
fn section_header_not_job_entry() {
    let line = parse_line("EXPERIENCE", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
}

// ── German/Italian heading recogniser gap (Projekte / Progetti) ────────────

/// The exact reported bug: a Title-Case German "Projekte" heading (not
/// ALL-CAPS, so `is_all_caps_section_heading` cannot save it) must render as
/// a SectionHeader, not fall through to body Text.
#[test]
fn german_title_case_projekte_is_section_header() {
    let line = parse_line("Projekte", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "Title-Case 'Projekte' must be a SectionHeader, got {:?}",
        line.kind
    );
}

/// The other three German headings the producer can now emit and the
/// recogniser previously lacked.
#[test]
fn german_title_case_new_headings_are_section_headers() {
    for heading in ["Zertifikate", "Auszeichnungen", "Publikationen"] {
        let line = parse_line(heading, 5, &[]);
        assert!(
            matches!(line.kind, LineKind::SectionHeader),
            "{heading:?} must be a SectionHeader, got {:?}",
            line.kind
        );
    }
}

/// Italian is the other priority locale named in the bug report (a real user
/// works in Italy) — its five new headings must all recognise too.
#[test]
fn italian_title_case_new_headings_are_section_headers() {
    for heading in [
        "Progetti",
        "Certificazioni",
        "Lingue",
        "Riconoscimenti",
        "Pubblicazioni",
    ] {
        let line = parse_line(heading, 5, &[]);
        assert!(
            matches!(line.kind, LineKind::SectionHeader),
            "{heading:?} must be a SectionHeader, got {:?}",
            line.kind
        );
    }
}

/// pt-PT "Prémios" is what the producer actually emits; pt-BR "Prêmios" is
/// accepted too even though the producer never generates it — nothing in the
/// producer's header table discriminates the two spellings.
#[test]
fn portuguese_awards_both_spellings_recognised() {
    for heading in ["Prémios", "Prêmios"] {
        let line = parse_line(heading, 5, &[]);
        assert!(
            matches!(line.kind, LineKind::SectionHeader),
            "{heading:?} must be a SectionHeader, got {:?}",
            line.kind
        );
    }
}

// ── Combined "X & Y" headings (the "Ausbildung & Sprachen" half of the bug) ─

/// The other reported symptom: a merged heading where BOTH halves are
/// individually known must still render as a heading rather than as an
/// unstyled paragraph — the producer forbids emitting this shape going
/// forward, but this covers already-generated documents and any
/// non-compliant generation.
#[test]
fn combined_ampersand_heading_both_known_is_section_header() {
    let line = parse_line("Ausbildung & Sprachen", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader for a combined heading with both known halves, got {:?}",
        line.kind
    );
}

/// English combined heading — the join logic is not German-specific.
#[test]
fn combined_ampersand_heading_english_is_section_header() {
    let line = parse_line("Skills & Certifications", 5, &[]);
    assert!(
        matches!(line.kind, LineKind::SectionHeader),
        "expected SectionHeader, got {:?}",
        line.kind
    );
}

/// The join must NOT recognise a heading when only one half is known, or
/// neither is — an arbitrary "X & Y" prose line stays Text.
#[test]
fn combined_ampersand_heading_requires_both_halves_known() {
    let one_known = parse_line("Projekte & Craft Beer", 5, &[]);
    assert!(
        !matches!(one_known.kind, LineKind::SectionHeader),
        "one known half must NOT be enough, got {:?}",
        one_known.kind
    );
    let neither_known = parse_line("Beer & Wine", 5, &[]);
    assert!(
        !matches!(neither_known.kind, LineKind::SectionHeader),
        "neither half known must stay non-heading, got {:?}",
        neither_known.kind
    );
}

/// Parity guard for `is_known_section_name`'s `" & "` arm — the half of that
/// predicate the section-names fixture structurally cannot see. That fixture
/// compares two name LISTS; the join arm is a rule that combines two entries,
/// so Rust could (and did) gain it while the TS mirror
/// (`isKnownSectionName`) kept answering `false` for every merged heading,
/// with every existing test still green.
///
/// The three-part cases pin `split_once`'s cut-at-the-FIRST-separator
/// semantics, which is load bearing because one entry contains a separator of
/// its own ("certifications & training").
#[test]
fn section_name_joins_match_the_ts_predicate_fixture() {
    #[derive(serde::Deserialize)]
    struct Case {
        line: String,
        known: bool,
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/prompts/src/fixtures/section-name-joins.json");
    let raw = std::fs::read_to_string(&path).expect(
        "read section-name-joins parity fixture \
         (packages/prompts/src/fixtures/section-name-joins.json)",
    );
    let cases: Vec<Case> =
        serde_json::from_str(&raw).expect("parse section-name-joins parity fixture");

    assert!(
        !cases.is_empty(),
        "section-name-joins parity fixture must not be empty"
    );
    assert!(
        cases.iter().any(|c| c.known) && cases.iter().any(|c| !c.known),
        "the fixture must carry BOTH accepted and rejected joins — one-sided, \
         it passes for a predicate hardwired to that answer"
    );
    for c in &cases {
        // Same reasoning as the sibling fixtures: `parse_line` only ever runs
        // this predicate on `strip_md(trimmed)`.
        let clean = strip_md(c.line.trim());
        assert_eq!(
            is_known_section_name(&clean),
            c.known,
            "is_known_section_name drift for {:?} (clean: {:?})",
            c.line,
            clean
        );
    }
}

#[test]
fn section_names_exactly_matches_ts_known_section_names_fixture() {
    // Cross-language parity guard, same shape as the contact-line fixture
    // above, but for one of the TWO predicates that gate the renderer's
    // header-seeding scan boundary (`isKnownSectionName` in
    // packages/prompts/src/generate/text/header-contact-line.ts — the other
    // is `isAllCapsSectionHeading`, tested below). Asserted both ways
    // (fixture ⊆ SECTION_NAMES and SECTION_NAMES ⊆ fixture) so extending
    // either list without the other fails immediately. Covers all 7 locales
    // `packages/prompts/src/locale/index.ts`'s `CONVENTIONS` ships résumé
    // headers for (en/de/fr/es/it/nl/pt) — a résumé generated for any of them
    // whose model wrote a Title-Case (not ALL-CAPS) heading still stops the
    // scan here. TS doesn't hold a second copy of this list at all — it
    // imports the fixture directly as its runtime data — so only this
    // direction can ever drift.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/prompts/src/fixtures/section-names.json");
    let raw = std::fs::read_to_string(&path).expect(
        "read section-names parity fixture (packages/prompts/src/fixtures/section-names.json)",
    );
    let fixture: Vec<String> =
        serde_json::from_str(&raw).expect("parse section-names parity fixture");

    assert!(
        !fixture.is_empty(),
        "section-names parity fixture must not be empty"
    );
    let fixture_set: std::collections::BTreeSet<&str> =
        fixture.iter().map(String::as_str).collect();
    let rust_set: std::collections::BTreeSet<&str> = SECTION_NAMES.iter().copied().collect();
    // Set equality alone silently absorbs a duplicate entry on either side
    // (a name authored twice collapses to one element and the comparison
    // below would still pass) — catch that separately so a duplicate is a
    // real, loud failure rather than a no-op.
    assert_eq!(
        fixture.len(),
        fixture_set.len(),
        "section-names fixture must not contain a duplicate entry"
    );
    assert_eq!(
        SECTION_NAMES.len(),
        rust_set.len(),
        "SECTION_NAMES must not contain a duplicate entry"
    );
    assert_eq!(
        fixture_set, rust_set,
        "SECTION_NAMES and the shared fixture must contain exactly the same names"
    );
}

// ── Recurrence guard: producer/recogniser contract, mechanically enforced ──

/// Every heading `pipeline::resume::prompt_blocks::resume_conventions` can
/// emit, for every one of the nine ordered `SectionId`s and all seven curated
/// locales, must be recognised by `export::parser` as a SectionHeader —
/// exactly as the producer emits it (Title-Case, not ALL-CAPS, since
/// ALL-CAPS already has its own shape-based recognition path and Title-Case
/// is the shape that broke in the reported bug). This closes the loop
/// mechanically: a future SectionId or locale added to the producer's total
/// `headers` record without a matching recogniser entry fails HERE, not in a
/// screenshot. Both axes are enumerated from the generated data itself
/// (`RESUME_CONVENTION_LOCALES` and `ResumeConventions::ids`) rather than
/// restated as literal lists — a hardcoded list is exactly how a guard ends up
/// never visiting the new thing it was written to catch.
///
/// Mutation check on the locale axis: added an 8th locale (`sv`) to the TS
/// `CONVENTIONS` with headings absent from `SECTION_NAMES` and re-ran
/// `pnpm gen:prompts` — RAN, went red naming `sv`, reverted. With the old
/// hardcoded `LOCALES` list it would have stayed green.
///
/// Whether this guard would have caught the ORIGINALLY reported bug, if it
/// had existed beforehand: see the report — the honest answer is nuanced,
/// not a flat yes.
#[test]
fn every_producer_heading_is_recognised_by_the_parser() {
    use crate::pipeline::resume::prompt_blocks::{resume_conventions, RESUME_CONVENTION_LOCALES};

    for &lang in RESUME_CONVENTION_LOCALES {
        let conventions = resume_conventions(lang);
        for id in conventions.ids() {
            let heading = conventions.header(id);
            let line = parse_line(heading, 5, &[]);
            assert!(
                matches!(line.kind, LineKind::SectionHeader),
                "locale {lang:?} heading {heading:?} (for SectionId::{id}) was not \
                 recognised as a SectionHeader by export::parser, got {:?}",
                line.kind
            );
        }
    }
}
