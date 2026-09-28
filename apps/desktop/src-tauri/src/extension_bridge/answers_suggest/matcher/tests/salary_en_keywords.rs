use super::super::*;

use super::candidate;

/// A question matching a salary keyword is flagged `salary: true` — the
/// popup's Copy-only rule reads this field directly.
#[test]
fn match_questions_flags_salary_keyword_questions() {
    let candidates = vec![candidate(
        "What is your expected salary?",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["What is your expected salary range?".to_string()],
        &candidates,
    );
    assert_eq!(out.len(), 1);
    assert!(
        out[0].salary,
        "a salary-keyword question must be flagged for Copy-only"
    );
}

/// A question with no salary keyword is never flagged.
#[test]
fn match_questions_does_not_flag_non_salary_questions() {
    let candidates = vec![candidate(
        "Why do you want to work here?",
        "Because the mission excites me.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Why do you want to work here?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(!out[0].salary);
}

/// Cross-question footgun (review fix): "What is your current location?" and
/// "What is your current salary?" share {what, is, your, current} = 4 of a
/// 6-token union = 0.67, comfortably above MIN_SCORE, even though the two
/// questions are about completely different things. The scanned INPUT
/// question carries no salary keyword, but the matched candidate's own
/// (stored) question does — the salary Copy-only guard must catch this via
/// the candidate side, not just the input side.
#[test]
fn match_questions_flags_salary_when_matched_candidates_source_question_is_salary() {
    let candidates = vec![candidate(
        "What is your current salary?",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["What is your current location?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(
        out[0].salary,
        "a stored salary answer must stay Copy-only no matter which label it matched"
    );
    assert_eq!(out[0].source_question, "What is your current salary?");
}

/// The flip side of the above: when NEITHER the scanned input nor the
/// matched candidate's own question is salary-shaped, the suggestion stays
/// fillable — the OR'd guard must never over-flag an unrelated match.
#[test]
fn match_questions_stays_fillable_when_neither_side_is_salary() {
    let candidates = vec![candidate(
        "What is your notice period?",
        "Two weeks.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["What is your notice period?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(!out[0].salary);
}

/// Punctuation must never fracture a token from its bare form elsewhere: a
/// short stored question ("Notice period") against the verbose scanned label
/// ("What is your notice period?") shares 2 tokens {notice, period} over a
/// 5-token union = 0.4 — exactly at the (lowered) threshold. Before the
/// matcher-local tokenizer, the trailing "?" made "period?" a distinct token
/// from "period" and this pair scored only 0.2, well under the old 0.5.
#[test]
fn match_questions_matches_short_paraphrase_despite_trailing_punctuation() {
    let candidates = vec![candidate(
        "Notice period",
        "Two weeks.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["What is your notice period?".to_string()], &candidates);
    assert_eq!(
        out.len(),
        1,
        "punctuation must not block a genuine short-vs-verbose paraphrase"
    );
    assert_eq!(out[0].answer, "Two weeks.");
}

/// The other PR-6 regression pair: "want to work here" vs "want this role"
/// share {why, do, you, want} (4) over a 9-token union = 0.44, just above the
/// lowered 0.4 threshold — the pair the 0.5 threshold used to reject outright.
#[test]
fn match_questions_matches_why_this_role_paraphrase() {
    let candidates = vec![candidate(
        "Why do you want to work here?",
        "Because I love building things.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Why do you want this role?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].answer, "Because I love building things.");
}

/// Negative regression pair for the lowered 0.4 threshold: two genuinely
/// unrelated questions share zero tokens and must never match, however low
/// the threshold goes.
#[test]
fn match_questions_does_not_match_unrelated_salary_and_license_questions() {
    let candidates = vec![candidate(
        "What is your salary expectation?",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["Do you have a driver's license?".to_string()],
        &candidates,
    );
    assert!(
        out.is_empty(),
        "unrelated questions must never match, even at a lowered threshold"
    );
}

/// Near-miss negative regression: a partial-overlap pair that shares 3 of 8
/// tokens ("what","is","your" out of {what,is,your,desired,start,date,
/// favorite,color}) — 3/8 = 0.375, just below `MIN_SCORE` (0.4) — must never
/// match despite sharing a common question-stem.
#[test]
fn match_questions_does_not_match_near_miss_partial_overlap() {
    let candidates = vec![candidate(
        "What is your desired start date?",
        "Immediately",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["What is your favorite color?".to_string()], &candidates);
    assert!(
        out.is_empty(),
        "3/8 = 0.375 overlap must fall below MIN_SCORE and never match"
    );
}
