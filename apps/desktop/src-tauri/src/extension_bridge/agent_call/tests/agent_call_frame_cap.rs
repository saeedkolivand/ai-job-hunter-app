//! Tests for the outgoing frame-size ceiling (`agent_call.rs`).

use super::super::*;

/// Builds a reply string just past [`super::super::super::MAX_FRAME_BYTES`] and
/// checks the substitution fires. The size is the ONLY thing that differs
/// from the sibling under-cap test below, so together they mutation-check
/// `enforce_frame_cap`'s comparison in both directions: delete the check and
/// this test fails; invert it and the sibling fails.
#[test]
fn enforce_frame_cap_refuses_an_oversized_reply_with_the_result_too_large_sentinel() {
    let oversized = "x".repeat(super::super::super::MAX_FRAME_BYTES + 1);
    let (reply, dispatched) =
        enforce_frame_cap("req-1", "autopilot", "autopilot_list", oversized, true);

    assert!(
        !dispatched,
        "the span must record what actually went on the wire, not what dispatch alone decided"
    );
    let parsed: Value = serde_json::from_str(&reply).expect("the substitute is valid JSON");
    let payload = &parsed["payload"];
    assert_eq!(payload["error"].as_str().unwrap(), ERR_RESULT_TOO_LARGE);
    assert!(!payload["dispatched"].as_bool().unwrap());
    assert_eq!(payload["namespace"].as_str().unwrap(), "autopilot");
    assert_eq!(payload["command"].as_str().unwrap(), "autopilot_list");
    assert_eq!(parsed["reqId"].as_str().unwrap(), "req-1");

    let detail = payload["detail"].as_str().unwrap();
    // The MEASURED byte count, never an estimate — this is the one number a
    // caller can act on, and its absence is what made #1135 undiagnosable.
    assert!(
        detail.contains(&(super::super::super::MAX_FRAME_BYTES + 1).to_string()),
        "detail must carry the measured size: {detail}"
    );
    // Says outright that the command RAN — `dispatched:false` above means
    // "no result delivered", and a caller that read it as "nothing happened"
    // would re-run a mutation that already took effect.
    assert!(
        detail.contains("RAN"),
        "detail must not imply nothing ran: {detail}"
    );
    // NOT the MCP cap's "narrow the query" advice: no argument on
    // `autopilot_list` can narrow anything (issue #1135's whole point).
    assert!(
        !detail.contains("narrow the query"),
        "advice that presupposes a parameter this command does not have: {detail}"
    );
    // The substitute is itself deliverable — a refusal that also blew the cap
    // would reproduce the very failure it reports.
    assert!(reply.len() <= super::super::super::MAX_FRAME_BYTES);
}

/// The other direction of the same guard: an ordinary reply must pass through
/// byte-for-byte, with `dispatched` untouched. The second half sits exactly
/// AT the cap rather than merely "small", so it also pins the boundary as
/// `>` and not `>=` — a still-deliverable frame must not be refused.
#[test]
fn enforce_frame_cap_passes_an_under_cap_reply_through_untouched() {
    let reply = call_result_reply("req-2", "jobs", "jobs_list", Ok(json!([{ "id": "j-1" }])));
    let (out, dispatched) = enforce_frame_cap("req-2", "jobs", "jobs_list", reply.clone(), true);
    assert_eq!(
        out, reply,
        "an under-cap reply must not be rewritten at all"
    );
    assert!(dispatched);

    let at_cap = "y".repeat(super::super::super::MAX_FRAME_BYTES);
    let (out, dispatched) = enforce_frame_cap("req-3", "jobs", "jobs_list", at_cap.clone(), true);
    assert_eq!(out.len(), at_cap.len());
    assert!(dispatched);
}

// ── Paged list commands (issue #1136) ────────────────────────────────────
