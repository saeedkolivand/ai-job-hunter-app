//! Tests for the paired-extension caller's own gate + reply cap (PR1).

use super::super::reply::agent_result_reply;
use super::super::*;

#[test]
fn extension_gate_reply_carries_the_fixed_sentinel_and_detail() {
    let payload = json!({ "resource": RES_JOB });
    let reply = extension_gate_reply("req-ext-1", &payload);
    let v: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(v["type"], super::super::super::msg::AGENT_RESULT);
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(v["payload"]["resource"], RES_JOB);
    assert_eq!(
        v["payload"]["error"],
        crate::extension_bridge::agent_call::ERR_EXTENSION_READ_GATE
    );
    assert!(v["payload"]["detail"]
        .as_str()
        .unwrap()
        .contains("Assisted autofill"));
}

#[test]
fn extension_capped_reply_passes_a_reply_under_the_extension_cap_through_untouched() {
    let payload = json!({ "resource": RES_SCHEMA });
    let reply = agent_result_reply("req-ext-2", RES_SCHEMA, Ok(json!({ "small": true })));
    let capped = extension_capped_reply("req-ext-2", &payload, reply.clone());
    assert_eq!(capped, reply, "an under-cap reply must be untouched");
}

#[test]
fn extension_capped_reply_refuses_a_reply_over_the_extension_cap_with_result_too_large() {
    let payload = json!({ "resource": RES_JOB });
    // Build a reply that is over `EXTENSION_RESULT_MAX_BYTES` but comfortably under
    // `MAX_FRAME_BYTES`, so this pins the EXTENSION cap specifically, not the generic frame cap.
    let oversized = "x".repeat(super::super::super::EXTENSION_RESULT_MAX_BYTES + 1);
    let capped = extension_capped_reply("req-ext-3", &payload, oversized.clone());
    assert!(
        capped.len() < oversized.len(),
        "an over-cap reply must be replaced, not echoed back"
    );
    let v: Value = serde_json::from_str(&capped).unwrap();
    assert_eq!(v["payload"]["ok"], false);
    assert_eq!(
        v["payload"]["error"],
        crate::extension_bridge::agent_call::ERR_RESULT_TOO_LARGE
    );
    assert_eq!(v["payload"]["resource"], RES_JOB);
}
