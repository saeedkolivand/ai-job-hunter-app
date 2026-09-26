//! Unit tests for `answer_assist_topic`.

use super::*;

#[test]
fn parse_topic_recognizes_both_literals_and_rejects_anything_else() {
    assert_eq!(
        parse_topic(&serde_json::json!({ "topic": "company-brief" })).unwrap(),
        Some(AssistTopic::CompanyBrief)
    );
    assert_eq!(
        parse_topic(&serde_json::json!({ "topic": "salary-answer" })).unwrap(),
        Some(AssistTopic::SalaryAnswer)
    );
    assert_eq!(parse_topic(&serde_json::json!({})).unwrap(), None);
    assert!(parse_topic(&serde_json::json!({ "topic": "bogus" })).is_err());
}

#[test]
fn topic_question_for_salary_is_recognized_as_a_salary_question() {
    // The whole reason this can reuse the existing salary-shaped grounding with zero new
    // code: the synthesized question must itself be a whole-token match for
    // `answers_suggest::is_salary_question`'s keyword set.
    let q = topic_question(AssistTopic::SalaryAnswer).to_lowercase();
    assert!(q
        .split(|c: char| !c.is_alphanumeric())
        .any(|t| t == "salary"));
}

#[test]
fn topic_requires_draft_refuses_a_topic_outside_draft_mode() {
    let err =
        topic_requires_draft(Some(AssistTopic::CompanyBrief), AssistMode::Rewrite).unwrap_err();
    assert_eq!(err.to_string(), TOPIC_REQUIRES_DRAFT_MESSAGE);
}

#[test]
fn topic_requires_draft_admits_a_topic_in_draft_mode_and_no_topic_in_either_mode() {
    assert!(topic_requires_draft(Some(AssistTopic::SalaryAnswer), AssistMode::Draft).is_ok());
    assert!(topic_requires_draft(None, AssistMode::Draft).is_ok());
    assert!(topic_requires_draft(None, AssistMode::Rewrite).is_ok());
}
