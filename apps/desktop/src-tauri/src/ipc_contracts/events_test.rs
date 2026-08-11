//! Pins for the generated `pipeline:stage` vocabulary consts.
//!
//! `events.rs` is @generated/DO-NOT-EDIT, so these are hand-written. `pnpm
//! gen:ipc:check` proves the file MATCHES the TS source of truth; it cannot
//! prove the source of truth is self-consistent. That is what these check —
//! on the RUST consts, because they are what the Phase-3 emitter validates
//! against (the TS twins are covered by `packages/shared/src/events/events.test.ts`).

use super::events::{
    is_pipeline_section_key, PIPELINE_SECTION_EXPERIENCE_PREFIX, PIPELINE_SECTION_KEYS_FIXED,
    PIPELINE_STAGE_PHASES, SECTION_KEY_MAX_LENGTH,
};

/// The longest key the grammar can produce is `experience:255`. If the bound
/// ever drops below it, the emitter would reject a value its own grammar calls
/// legal — the guard contradicting the contract it enforces.
#[test]
fn the_bound_admits_the_longest_legal_section_key() {
    let longest = format!("{PIPELINE_SECTION_EXPERIENCE_PREFIX}255");
    assert!(
        longest.chars().count() <= SECTION_KEY_MAX_LENGTH,
        "'{longest}' is the longest legal key but exceeds SECTION_KEY_MAX_LENGTH \
         ({SECTION_KEY_MAX_LENGTH})"
    );
    for key in PIPELINE_SECTION_KEYS_FIXED {
        assert!(
            key.chars().count() <= SECTION_KEY_MAX_LENGTH,
            "fixed key '{key}' exceeds SECTION_KEY_MAX_LENGTH ({SECTION_KEY_MAX_LENGTH})"
        );
    }
}

/// Value pin: the consts must carry the grammar, not some other vocabulary that
/// happens to be the right shape (both halves are short lower-case strings, so a
/// codegen mix-up with `PIPELINE_STAGE_PHASES` would satisfy every length check
/// above). Widening the grammar is a deliberate contract change — update the TS
/// source of truth, regenerate, and move this pin with it.
#[test]
fn the_section_key_grammar_is_pinned_to_its_literals() {
    assert_eq!(
        PIPELINE_SECTION_KEYS_FIXED,
        &["summary", "skills", "projects", "education"]
    );
    assert_eq!(PIPELINE_SECTION_EXPERIENCE_PREFIX, "experience:");
    assert_eq!(SECTION_KEY_MAX_LENGTH, 24);
}

/// The two halves must not overlap: a fixed key that also parses as the indexed
/// form (or an empty prefix, which would make EVERY key indexed) turns a closed
/// grammar into an ambiguous one.
#[test]
fn the_two_halves_of_the_grammar_are_disjoint_and_non_empty() {
    assert!(!PIPELINE_SECTION_EXPERIENCE_PREFIX.is_empty());
    assert!(!PIPELINE_SECTION_KEYS_FIXED.is_empty());
    assert!(!PIPELINE_STAGE_PHASES.is_empty());
    for key in PIPELINE_SECTION_KEYS_FIXED {
        assert!(
            !key.starts_with(PIPELINE_SECTION_EXPERIENCE_PREFIX),
            "fixed key '{key}' also matches the indexed form"
        );
    }
}

/// The generated guard must accept exactly what the TS `isPipelineSectionKey`
/// accepts — the cases mirror `packages/shared/src/events/events.test.ts`.
#[test]
fn the_guard_admits_the_grammar_and_nothing_else() {
    for key in [
        "summary",
        "skills",
        "projects",
        "education",
        "experience:0",
        "experience:7",
        "experience:255",
    ] {
        assert!(
            is_pipeline_section_key(key),
            "'{key}' is legal grammar but the guard rejected it"
        );
    }
    for key in [
        "Work History",
        "experience",     // the index is not optional
        "experience:",    // ...nor empty
        "experience:256", // ...nor wider than a u8
        "experience:1.5",
        "experience:-1",
        "experience:0; DROP TABLE runs",
        "SUMMARY", // the grammar is lower-case
        "",
    ] {
        assert!(
            !is_pipeline_section_key(key),
            "'{key}' is not the grammar but the guard accepted it"
        );
    }
    // Length is checked FIRST, so a value far past the bound is rejected without
    // the guard doing any further work on it.
    let hostile = format!("{PIPELINE_SECTION_EXPERIENCE_PREFIX}{}", "9".repeat(10_000));
    assert!(!is_pipeline_section_key(&hostile));
}

/// THE canonical-decimal pin. `str::parse::<u8>` is LOOSER than the TS regex
/// `^(0|[1-9][0-9]{0,2})$`: it accepts a `+` sign and leading zeros, so an
/// emitter that only parsed would put `experience:01` — a second spelling of
/// `experience:1` — on a wire whose vocabulary is supposed to be closed. Each
/// case asserts BOTH halves: that a bare parse would have let it through, and
/// that the guard does not. Loosening the generated guard back to a plain parse
/// fails here.
#[test]
fn the_indexed_half_admits_only_canonical_decimals() {
    for index in ["01", "007", "+1", "0255"] {
        assert!(
            index.parse::<u8>().is_ok(),
            "'{index}' no longer parses as a u8 — this pin has lost its point"
        );
        let key = format!("{PIPELINE_SECTION_EXPERIENCE_PREFIX}{index}");
        assert!(
            !is_pipeline_section_key(&key),
            "'{key}' is not canonical decimal but the guard accepted it — the \
             guard is parsing before validating"
        );
    }
    // Whitespace and the other non-canonical spellings a parse also rejects,
    // pinned so the guard can never start trimming its input.
    for index in [" 1", "1 ", "1_0", "٣"] {
        let key = format!("{PIPELINE_SECTION_EXPERIENCE_PREFIX}{index}");
        assert!(
            !is_pipeline_section_key(&key),
            "'{key}' is not canonical decimal but the guard accepted it"
        );
    }
}
