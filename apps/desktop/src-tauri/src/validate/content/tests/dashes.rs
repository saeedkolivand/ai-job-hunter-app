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

/// Each of these was corrupted by the first version of the strip.
#[test]
fn ranges_compound_names_and_list_markers_are_never_damaged() {
    for kept in [
        "March – June 2021",
        "Jan–Mar 2020",
        "Mon – Fri",
        "Mon–Fri",
        "Rolls–Royce",
        "– Shipped the cache",
        "  — Shipped the cache",
        "Shipped the cache —",
        "Shipped the cache — ",
        "Januar – März 2021",
        "10:00 – 18:00",
    ] {
        assert_eq!(strip_long_dashes(kept), kept, "{kept:?}");
    }
}

#[test]
fn an_unspaced_em_dash_between_words_becomes_a_comma() {
    assert_eq!(
        strip_long_dashes("Fast—and reliable, it ran—well."),
        "Fast, and reliable, it ran, well."
    );
}

#[test]
fn an_unspaced_en_dash_or_numeric_em_dash_is_left_alone() {
    for kept in [
        "Rolls–Royce",
        "Jan–Mar 2020",
        "2019—2022",
        "2019–2022",
        "Mon–Fri",
    ] {
        assert_eq!(strip_long_dashes(kept), kept, "{kept:?}");
    }
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

/// The check counts exactly what the strip would remove: kept dashes never raise the warning.
#[test]
fn kept_dashes_are_not_counted_as_overuse() {
    use super::super::dashes::clause_dash_count;
    assert_eq!(
        clause_dash_count("Rolls–Royce, Jan–Mar, Mon – Fri\nhttps://example.dev/a – b"),
        0
    );
    assert_eq!(clause_dash_count("I built it — and it ran – well."), 2);
}
