use super::super::*;

use super::candidate;

// ── German (DACH) salary keywords (Task #30) ──────────────────────────────────

/// Each DACH salary-question shape is individually flagged Copy-only — one
/// assertion per compound noun, mirroring the English keyword coverage above.
#[test]
fn match_questions_flags_german_salary_shapes() {
    for question in [
        "Wie hoch ist Ihr Gehalt?",
        "Bitte geben Sie Ihre Gehaltsvorstellung an",
        "Nennen Sie uns Ihre Gehaltsvorstellungen",
        "Was ist Ihr Gehaltswunsch?",
        "Ihr Bruttojahresgehalt?",
        "Erwartetes Jahresgehalt",
        "Gewünschte Vergütung",
        "Ihre Salärvorstellung",
    ] {
        let candidates = vec![candidate(question, "80.000 EUR", "Acme", "Backend", 1_000)];
        let out = match_questions(&[question.to_string()], &candidates);
        assert_eq!(out.len(), 1, "expected a match for {question:?}");
        assert!(out[0].salary, "{question:?} must be flagged salary");
    }
}

/// Near-miss (decided + pinned, Task #30): "Gehaltsabrechnung hochladen"
/// ("upload payslip") is a DIFFERENT compound word from every listed salary
/// keyword — it must NEVER be flagged. Correct behavior: this question wants
/// a file upload, not a stated expectation, so the synthetic salary-
/// expectation fill (`resolve_answers_suggest`) must never target it either.
#[test]
fn match_questions_does_not_flag_gehaltsabrechnung_hochladen_as_salary() {
    let candidates = vec![candidate(
        "Gehaltsabrechnung hochladen",
        "Erledigt.",
        "Acme",
        "Backend",
        1_000,
    )];
    let out = match_questions(&["Gehaltsabrechnung hochladen".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert!(
        !out[0].salary,
        "\"Gehaltsabrechnung\" (payslip) must not match \"gehalt\"-family keywords"
    );
}

/// Pure property: the SAME inputs always produce the SAME output — no AI, no
/// egress, no randomness (the PR-6 handoff's binding determinism property).
#[test]
fn match_questions_is_deterministic() {
    let candidates = vec![
        candidate(
            "Why this role?",
            "Answer A.",
            "Acme",
            "Backend Engineer",
            1_000,
        ),
        candidate(
            "Why this role?",
            "Answer B.",
            "Globex",
            "QA Engineer",
            2_000,
        ),
    ];
    let questions = vec!["Why this role?".to_string()];
    let first = match_questions(&questions, &candidates);
    let second = match_questions(&questions, &candidates);
    assert_eq!(first, second);
}
