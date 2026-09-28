use super::super::*;

use super::super::super::test_support::bridge_state;
use super::awaiting_auth;

/// An authenticated `status.update` classifies as `StatusUpdate`, carrying the
/// payload verbatim (mirrors `authenticated_applied_check_classifies_as_applied_check`).
#[test]
fn authenticated_status_update_classifies_as_status_update() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::STATUS_UPDATE,
        "reqId": "r-status",
        "payload": { "url": "https://jobs.example.com/posting/10", "to": "applied" }
    })
    .to_string();

    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::StatusUpdate { req_id, payload } => {
            assert_eq!(req_id, "r-status");
            assert_eq!(
                payload.get("url").and_then(|u| u.as_str()),
                Some("https://jobs.example.com/posting/10")
            );
            assert_eq!(payload.get("to").and_then(|t| t.as_str()), Some("applied"));
        }
        other => panic!("an authenticated status.update must be StatusUpdate, got {other:?}"),
    }
}

/// A `status.update` before the handshake completes is NEVER dispatched — same
/// invariant as `applied_check_before_handshake_is_not_dispatched`: only a
/// hello may be the first frame.
#[test]
fn status_update_before_handshake_is_not_dispatched() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::STATUS_UPDATE,
        "reqId": "r-early",
        "payload": { "url": "https://jobs.example.com/posting/early", "to": "applied" },
    })
    .to_string();
    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(_) => {}
        other => panic!("a status.update before hello must NOT be StatusUpdate, got {other:?}"),
    }
}

/// Same mid-handshake bypass guard as `applied_check_mid_handshake_is_unauthorized`
/// — a `status.update` cannot skip the proof step either. This is the one
/// verb where skipping this guard would matter most (it is a WRITE), so it
/// gets the same three auth-boundary tests as every other authenticated verb.
#[test]
fn status_update_mid_handshake_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": msg::STATUS_UPDATE,
        "reqId": "r-skip",
        "payload": { "url": "https://jobs.example.com/posting/skip", "to": "applied" },
    })
    .to_string();
    match advance_frame(&state, &conn, &frame) {
        FrameDecision::Unauthorized => {}
        other => panic!("a status.update mid-handshake must be Unauthorized, got {other:?}"),
    }
}
/// An authenticated `answers.save` classifies as `AnswersSave`, carrying the
/// payload verbatim (mirrors `authenticated_status_update_classifies_as_status_update`).
#[test]
fn authenticated_answers_save_classifies_as_answers_save() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::ANSWERS_SAVE,
        "reqId": "r-answers",
        "payload": {
            "url": "https://jobs.example.com/posting/11",
            "answers": [{ "question": "Why this role?", "answer": "Because I love it." }],
        }
    })
    .to_string();

    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::AnswersSave { req_id, payload } => {
            assert_eq!(req_id, "r-answers");
            assert_eq!(
                payload.get("url").and_then(|u| u.as_str()),
                Some("https://jobs.example.com/posting/11")
            );
            assert!(payload.get("answers").is_some_and(|a| a.is_array()));
        }
        other => panic!("an authenticated answers.save must be AnswersSave, got {other:?}"),
    }
}

/// An `answers.save` before the handshake completes is NEVER dispatched —
/// same invariant as `status_update_before_handshake_is_not_dispatched`.
#[test]
fn answers_save_before_handshake_is_not_dispatched() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::ANSWERS_SAVE,
        "reqId": "r-early",
        "payload": { "url": "https://jobs.example.com/posting/early", "answers": [] },
    })
    .to_string();
    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(_) => {}
        other => panic!("an answers.save before hello must NOT be AnswersSave, got {other:?}"),
    }
}

/// Same mid-handshake bypass guard as `status_update_mid_handshake_is_unauthorized`
/// — `answers.save` cannot skip the proof step either (it is a WRITE, so it
/// gets the same three auth-boundary tests as every other authenticated verb).
#[test]
fn answers_save_mid_handshake_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": msg::ANSWERS_SAVE,
        "reqId": "r-skip",
        "payload": { "url": "https://jobs.example.com/posting/skip", "answers": [] },
    })
    .to_string();
    match advance_frame(&state, &conn, &frame) {
        FrameDecision::Unauthorized => {}
        other => panic!("an answers.save mid-handshake must be Unauthorized, got {other:?}"),
    }
}
/// An authenticated `answers.suggest` classifies as `AnswersSuggest`, carrying
/// the payload verbatim (mirrors `authenticated_answers_save_classifies_as_answers_save`).
#[test]
fn authenticated_answers_suggest_classifies_as_answers_suggest() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::ANSWERS_SUGGEST,
        "reqId": "r-suggest",
        "payload": { "questions": ["Why this role?"] }
    })
    .to_string();

    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::AnswersSuggest { req_id, payload } => {
            assert_eq!(req_id, "r-suggest");
            assert_eq!(
                payload
                    .get("questions")
                    .and_then(|q| q.as_array())
                    .map(Vec::len),
                Some(1)
            );
        }
        other => panic!("an authenticated answers.suggest must be AnswersSuggest, got {other:?}"),
    }
}

/// An `answers.suggest` before the handshake completes is NEVER dispatched —
/// same invariant as `answers_save_before_handshake_is_not_dispatched`.
#[test]
fn answers_suggest_before_handshake_is_not_dispatched() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::ANSWERS_SUGGEST,
        "reqId": "r-early",
        "payload": { "questions": [] },
    })
    .to_string();
    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(_) => {}
        other => {
            panic!("an answers.suggest before hello must NOT be AnswersSuggest, got {other:?}")
        }
    }
}

/// Same mid-handshake bypass guard as `answers_save_mid_handshake_is_unauthorized`
/// — `answers.suggest` cannot skip the proof step either (it returns the
/// user's own past answer text, so it gets the same three auth-boundary tests
/// as every other authenticated verb).
#[test]
fn answers_suggest_mid_handshake_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": msg::ANSWERS_SUGGEST,
        "reqId": "r-skip",
        "payload": { "questions": [] },
    })
    .to_string();
    match advance_frame(&state, &conn, &frame) {
        FrameDecision::Unauthorized => {}
        other => panic!("an answers.suggest mid-handshake must be Unauthorized, got {other:?}"),
    }
}
