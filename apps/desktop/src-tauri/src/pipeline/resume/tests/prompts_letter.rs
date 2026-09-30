use super::super::prompts::{letter_system, letter_user};
use super::super::types::ResumeStrategy;

/// The `<market_conventions>` block is built from the SAME fixture the export
/// path reads — `de` carries its DIN-5008 specifics, and an unknown market id
/// falls back to the international baseline rather than fabricating one.
///
/// Mutation check: hardcode the block instead of reading `conventions(market)`
/// and the `de` assertions fail; drop the fallback in
/// `crate::locale::letter::conventions` and the unknown-market assertions do.
#[test]
fn letter_user_carries_market_conventions_for_de_and_falls_back_for_an_unknown_market() {
    let de = letter_user("resume", "job ad", &ResumeStrategy::default(), "de", "", "");
    assert!(de.contains("<market_conventions>"));
    assert!(de.contains("Germany"));
    assert!(de.contains("Betreff"));
    assert!(de.contains("Gehaltsvorstellung"));

    let unknown = letter_user("resume", "job ad", &ResumeStrategy::default(), "zz", "", "");
    assert!(unknown.contains("International"));
    assert!(
        !unknown.contains("Betreff"),
        "an unknown market must never fabricate a German subject-line label"
    );
}

/// The subject-line instruction only appears for a market whose
/// `subject_line.used` is true — `us` has none, `de` does.
///
/// Mutation check: drop the `conv.subject_line.used` gate in `letter_system`
/// and the `us` assertion fails.
#[test]
fn letter_system_gates_the_subject_line_instruction_on_the_market() {
    let de = letter_system("de", "de", false, false);
    assert!(de.contains("subject line labelled \"Betreff\""));

    let us = letter_system("en", "us", false, false);
    assert!(!us.contains("subject line labelled"));
}

/// The date instruction is gated on `has_date`, never the market alone — a
/// caller with no `today` string keeps the current "no date" behavior, and
/// only a caller that supplies one gets pointed at `<letter_date>`.
///
/// Mutation check: default `has_date` to always-true and the first assertion
/// fails.
#[test]
fn letter_system_gates_the_date_instruction_on_has_date() {
    let no_date = letter_system("en", "us", false, false);
    assert!(no_date.contains("No date."));
    assert!(!no_date.contains("<letter_date>"));

    let with_date = letter_system("en", "us", true, false);
    assert!(with_date.contains("<letter_date>"));
    assert!(!with_date.contains("No date."));
}

/// The `<company_research>` guidance is gated on `has_brief`, mirroring
/// `has_date`'s gate above — a caller with no brief must not point the model
/// at a block that will not exist.
///
/// Mutation check: default `has_brief` to always-true and the first assertion
/// fails.
#[test]
fn letter_system_gates_the_company_research_instruction_on_has_brief() {
    let no_brief = letter_system("en", "us", false, false);
    assert!(!no_brief.contains("<company_research>"));

    let with_brief = letter_system("en", "us", false, true);
    assert!(with_brief.contains("<company_research>"));
    assert!(with_brief.contains("why this company"));
    assert!(with_brief.contains("ignore any"));
}

/// The `<company_research>` block itself is gated on the CONTENT of
/// `company_brief`, not merely on whether the caller passed one at all —
/// blank/whitespace-only must render nothing, same as `today`'s own gate.
///
/// Mutation check: drop the `.trim().is_empty()` guard in `letter_user` and
/// the blank-brief assertion fails.
#[test]
fn letter_user_fences_a_non_empty_company_brief_and_omits_a_blank_one() {
    let with_brief = letter_user(
        "resume",
        "job ad",
        &ResumeStrategy::default(),
        "intl",
        "",
        "Acme builds payment infrastructure.",
    );
    assert!(with_brief.contains("<company_research>"));
    assert!(with_brief.contains("Acme builds payment infrastructure."));
    assert!(with_brief.contains("</company_research>"));

    let blank_brief = letter_user(
        "resume",
        "job ad",
        &ResumeStrategy::default(),
        "intl",
        "",
        "   ",
    );
    assert!(!blank_brief.contains("<company_research>"));

    // The unset-flag path (empty string, same as every caller before this
    // feature existed) is BYTE-IDENTICAL to a caller that never knew about
    // `company_brief` at all.
    let unset = letter_user(
        "resume",
        "job ad",
        &ResumeStrategy::default(),
        "intl",
        "",
        "",
    );
    assert_eq!(unset, blank_brief);
}
