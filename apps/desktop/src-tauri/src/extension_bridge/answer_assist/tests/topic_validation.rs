//! Topic validation — the actual `resolve_answer_assist` call-site sequence:
//! `parse_topic(payload)?; topic_requires_draft(topic, mode)?;`. The two
//! functions have their own isolated unit tests in `answer_assist_topic.rs`;
//! this covers the composed branch as it is actually wired here.

use serde_json::json;

use crate::extension_bridge::answer_assist_topic::{
    parse_topic, topic_requires_draft, AssistTopic,
};

use super::super::AssistMode;

#[test]
fn topic_validation_accepts_a_recognized_topic_in_draft_mode() {
    let topic = parse_topic(&json!({ "topic": "salary-answer" })).unwrap();
    assert_eq!(topic, Some(AssistTopic::SalaryAnswer));
    assert!(topic_requires_draft(topic, AssistMode::Draft).is_ok());
}

#[test]
fn topic_validation_refuses_a_recognized_topic_combined_with_rewrite_mode() {
    let topic = parse_topic(&json!({ "topic": "company-brief" })).unwrap();
    assert!(topic_requires_draft(topic, AssistMode::Rewrite).is_err());
}

#[test]
fn topic_validation_refuses_a_malformed_topic_value_before_the_mode_check_ever_runs() {
    // `mode: "rewrite"` here to prove the parse failure (`?`) wins independently of whichever
    // mode the request also carries — the mode check never even runs.
    assert!(parse_topic(&json!({ "topic": "bogus", "mode": "rewrite" })).is_err());
}
