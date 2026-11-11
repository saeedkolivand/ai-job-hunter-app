//! The long-dash ban, enforced after humanize: `strip_long_dashes` and the
//! shared definition of a clause dash it and `voice.em_dash_overuse` rest on.

use super::{support::*, *};

#[test]
fn a_clause_dash_becomes_a_comma() {
    assert_eq!(
        strip_long_dashes("I led the migration — and the rollout – alone."),
        "I led the migration, and the rollout, alone."
    );
}

#[test]
fn date_ranges_and_link_lines_are_left_whole() {
    let kept =
        "Backend Developer | Globex | 2018 – 2021\nBackend Developer | Globex | 2021 – Present\n\
                **Ledger** · https://example.dev/a—b · 2020–2023";
    assert_eq!(strip_long_dashes(kept), kept);
}

#[test]
fn a_dash_at_a_line_edge_is_dropped_without_a_stray_comma() {
    assert_eq!(
        strip_long_dashes("— Shipped the cache"),
        "Shipped the cache"
    );
    assert_eq!(
        strip_long_dashes("Shipped the cache —"),
        "Shipped the cache"
    );
}

#[test]
fn stripping_is_idempotent_and_leaves_clean_text_alone() {
    let once = strip_long_dashes("One — two — three.");
    assert_eq!(strip_long_dashes(&once), once);
    assert_eq!(
        strip_long_dashes("Plain text, no dashes."),
        "Plain text, no dashes."
    );
}

/// A document that the validator flags for dash overuse is clean once stripped:
/// the ban and the check are one definition.
#[test]
fn a_stripped_letter_no_longer_trips_the_dash_check() {
    let letter = "Dear Hiring Manager,\n\nI built the ledger — it ran well — and the cache — it was fast — shipped.\n";
    let before = letter_report_for(letter, EN_SOURCE, EN_JOB_AD);
    assert!(
        codes(&before).contains(&VOICE_EM_DASH_OVERUSE),
        "{:?}",
        codes(&before)
    );
    let after = letter_report_for(&strip_long_dashes(letter), EN_SOURCE, EN_JOB_AD);
    silent(&after, VOICE_EM_DASH_OVERUSE);
}
