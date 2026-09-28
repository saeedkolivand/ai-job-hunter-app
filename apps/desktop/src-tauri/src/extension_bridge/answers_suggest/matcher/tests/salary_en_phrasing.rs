use super::super::*;

use super::candidate;

/// Broadened salary keywords (critic finding): "how much ... paid" flags
/// Copy-only without a literal "salary"/"compensation" token.
#[test]
fn match_questions_flags_how_much_paid_as_salary() {
    let candidates = vec![candidate(
        "How much do you expect to be paid?",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["How much do you expect to be paid?".to_string()],
        &candidates,
    );
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// "income" alone is enough to flag Copy-only.
#[test]
fn match_questions_flags_income_as_salary() {
    let candidates = vec![candidate(
        "Expected income",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Expected income".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// "day rate" (a salary-shaped multi-token phrase) is flagged.
#[test]
fn match_questions_flags_day_rate_as_salary() {
    let candidates = vec![candidate(
        "Day rate",
        "£500",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Day rate".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// Bare "rate" (no salary-shaped multi-token phrase) must NEVER trip the
/// denylist — a skills self-rating question is not a salary question.
#[test]
fn match_questions_does_not_flag_rate_your_skills_as_salary() {
    let candidates = vec![candidate(
        "Rate your TypeScript skills",
        "9/10",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Rate your TypeScript skills".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(!out[0].salary);
}

/// Hyphen-proof salary check (critic finding): `normalize_question` only
/// collapses whitespace, so "Day-rate" still carries the literal hyphen and
/// would silently miss the "day rate" phrase without the matcher's
/// non-alphanumeric re-tokenization.
#[test]
fn match_questions_flags_hyphenated_day_rate_as_salary() {
    let candidates = vec![candidate(
        "Day-rate",
        "£500",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Day-rate".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// Same as above with a slash instead of a hyphen.
#[test]
fn match_questions_flags_slash_day_rate_as_salary() {
    let candidates = vec![candidate(
        "day/rate",
        "£500",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["day/rate".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// Hyphenated "How-much" must still flag Copy-only, same as the
/// space-separated "How much" case above.
#[test]
fn match_questions_flags_hyphenated_how_much_as_salary() {
    let candidates = vec![candidate(
        "How-much do you expect?",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["How-much do you expect?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}

/// CodeRabbit finding: a single-word keyword ("paid") must match a WHOLE
/// token, never a substring inside an unrelated word — "unpaid" must never
/// trip the salary denylist.
#[test]
fn match_questions_does_not_flag_unpaid_leave_as_salary() {
    let candidates = vec![candidate(
        "Unpaid leave policy acknowledgment",
        "Acknowledged.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["Unpaid leave policy acknowledgment".to_string()],
        &candidates,
    );
    assert_eq!(out.len(), 1);
    assert!(
        !out[0].salary,
        "\"paid\" must not substring-match inside \"unpaid\""
    );
}

/// The single-word exact-token fix must not regress the genuine "paid"
/// salary question it was narrowed from.
#[test]
fn match_questions_still_flags_how_much_will_i_be_paid_as_salary() {
    let candidates = vec![candidate(
        "How much will I be paid?",
        "$120,000",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["How much will I be paid?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(out[0].salary);
}
