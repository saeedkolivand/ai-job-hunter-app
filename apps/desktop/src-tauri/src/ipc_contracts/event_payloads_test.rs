//! Wire-shape tests for the generated event payload structs. Pins the serialized
//! JSON to the historical `json!` shape: `thinking`/`error` keys are ABSENT (not
//! null) unless set, and `JobEvent` serializes its raw-ident `r#type` field as the
//! `type` wire key. Hand-written — `event_payloads.rs` is @generated/DO-NOT-EDIT.

use super::event_payloads::{AiStreamChunk, AiStreamChunkError, JobEvent};
use serde_json::{json, to_value};

#[test]
fn ai_stream_normal_delta_frame_omits_thinking_and_error() {
    let v = to_value(AiStreamChunk {
        job_id: "j".into(),
        delta: "hi".into(),
        done: false,
        error: None,
        thinking: None,
    })
    .unwrap();
    assert_eq!(v, json!({ "jobId": "j", "delta": "hi", "done": false }));
    assert!(v.get("thinking").is_none());
    assert!(v.get("error").is_none());
}

#[test]
fn ai_stream_thinking_frame_omits_error() {
    let v = to_value(AiStreamChunk {
        job_id: "j".into(),
        delta: "reason".into(),
        done: false,
        error: None,
        thinking: Some(true),
    })
    .unwrap();
    assert_eq!(
        v,
        json!({ "jobId": "j", "delta": "reason", "done": false, "thinking": true })
    );
    assert!(v.get("error").is_none());
}

#[test]
fn ai_stream_error_frame_omits_thinking() {
    let v = to_value(AiStreamChunk {
        job_id: "j".into(),
        delta: String::new(),
        done: true,
        error: Some(AiStreamChunkError {
            code: "GENERATION_FAILED".into(),
            message: "boom".into(),
        }),
        thinking: None,
    })
    .unwrap();
    assert_eq!(
        v,
        json!({
            "jobId": "j",
            "delta": "",
            "done": true,
            "error": { "code": "GENERATION_FAILED", "message": "boom" }
        })
    );
    assert!(v.get("thinking").is_none());
}

#[test]
fn job_event_serializes_type_key_and_ts() {
    let v = to_value(JobEvent {
        r#type: "job.stream".into(),
        job_id: "j".into(),
        data: Some(json!({ "count": 1 })),
        ts: 1700000000000,
    })
    .unwrap();
    assert_eq!(
        v,
        json!({ "type": "job.stream", "jobId": "j", "data": { "count": 1 }, "ts": 1700000000000i64 })
    );
    // The raw-ident Rust field `r#type` must serialize as the `type` wire key.
    assert!(v.get("type").is_some());
    assert!(v.get("r#type").is_none());
}
