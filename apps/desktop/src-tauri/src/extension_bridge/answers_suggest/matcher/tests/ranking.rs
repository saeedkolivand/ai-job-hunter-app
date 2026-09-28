use super::super::*;

use super::candidate;

/// A close paraphrase above the threshold is matched.
#[test]
fn match_questions_returns_best_match_above_threshold() {
    let candidates = vec![candidate(
        "Why do you want to work at our company?",
        "Because the mission excites me.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(&["Why do you want to work here?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].answer, "Because the mission excites me.");
    assert!(out[0].score >= 0.5);
}

/// An unrelated question stays below the threshold and is skipped entirely.
#[test]
fn match_questions_skips_below_threshold() {
    let candidates = vec![candidate(
        "Why do you want to work here?",
        "Because the mission excites me.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["What's your salary expectation?".to_string()],
        &candidates,
    );
    assert!(
        out.is_empty(),
        "an unrelated question must never be suggested"
    );
}

/// Two equally-scored candidates (identical question text) tie-break on the
/// most recently updated application — never the first-seen one.
#[test]
fn match_questions_tie_breaks_by_score_then_most_recent() {
    let candidates = vec![
        candidate(
            "Why this role?",
            "Older answer.",
            "Acme",
            "Backend Engineer",
            1_000,
        ),
        candidate(
            "Why this role?",
            "Newer answer.",
            "Globex",
            "QA Engineer",
            5_000,
        ),
    ];
    let out = match_questions(&["Why this role?".to_string()], &candidates);
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].answer, "Newer answer.",
        "the most recently updated application must win a tie"
    );
}

/// At most one suggestion per question, even with multiple candidates that
/// could match — never a fan-out.
#[test]
fn match_questions_caps_one_suggestion_per_question() {
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
        candidate("Why this role?", "Answer C.", "Initech", "SRE", 3_000),
    ];
    let out = match_questions(&["Why this role?".to_string()], &candidates);
    assert_eq!(out.len(), 1, "never more than one suggestion per question");
}

/// Two INPUT questions that normalize to the same text (case/whitespace
/// variants — e.g. two form fields sharing a label) collapse to one output
/// entry, not two.
#[test]
fn match_questions_dedupes_effectively_identical_input_questions() {
    let candidates = vec![candidate(
        "Why this role?",
        "Because I love it.",
        "Acme",
        "Backend Engineer",
        1_000,
    )];
    let out = match_questions(
        &["Why this role?".to_string(), "why   THIS role?".to_string()],
        &candidates,
    );
    assert_eq!(out.len(), 1);
}

/// The overall reply is capped at 20 suggestions even when every question has
/// a qualifying match.
#[test]
fn match_questions_caps_overall_at_max_suggestions() {
    let questions: Vec<String> = (0..25).map(|i| format!("Question {i}?")).collect();
    let candidates: Vec<AnswerCandidate> = questions
        .iter()
        .map(|q| candidate(q, "An answer.", "Acme", "Backend Engineer", 1_000))
        .collect();
    let out = match_questions(&questions, &candidates);
    assert_eq!(
        out.len(),
        20,
        "the overall reply must be capped at MAX_SUGGESTIONS"
    );
}
