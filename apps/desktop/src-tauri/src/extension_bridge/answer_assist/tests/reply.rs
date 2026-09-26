//! `answer_assist_reply` — the discriminated-union wire reply.

use serde_json::Value;

use crate::error::AppError;
use crate::extension_bridge::msg;

use super::super::errors::AI_ASSIST_OFF_MESSAGE;
use super::super::reply::{answer_assist_reply, AnswerAssistOk};

#[test]
fn answer_assist_reply_carries_ok_payload() {
    let reply = answer_assist_reply(
        "req-1",
        Ok(AnswerAssistOk {
            question: "Why this role?".to_string(),
            draft: "Because…".to_string(),
            sourced_web: true,
            sourced_brief: false,
            sourced_salary: false,
        }),
    );
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], msg::ANSWER_ASSIST_RESULT);
    assert_eq!(v["reqId"], "req-1");
    assert_eq!(v["payload"]["ok"], true);
    assert_eq!(v["payload"]["question"], "Why this role?");
    assert_eq!(v["payload"]["draft"], "Because…");
    assert_eq!(v["payload"]["sourced"]["web"], true);
    assert_eq!(v["payload"]["sourced"]["brief"], false);
    assert_eq!(v["payload"]["sourced"]["salary"], false);
}

#[test]
fn answer_assist_reply_carries_error_and_no_success_fields() {
    let reply = answer_assist_reply(
        "req-2",
        Err(AppError::Validation(AI_ASSIST_OFF_MESSAGE.to_string())),
    );
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(v["payload"]["error"], AI_ASSIST_OFF_MESSAGE);
    assert!(v["payload"].get("draft").is_none());
}
