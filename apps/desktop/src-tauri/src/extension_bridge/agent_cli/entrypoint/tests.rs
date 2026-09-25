//! `entrypoint.rs`'s own two behaviours that are neither the `--help` text
//! (see `help.rs`) nor the mcp/help dispatch order (see `dispatch.rs`): the
//! exit-code translation, and the whole-invocation deadline. Split by topic
//! under R8's LOC cap, so the three live in `tests/`.

mod dispatch;
mod help;
mod output;

use super::*;

#[test]
fn exit_code_for_reply_reads_dispatched_for_call_and_ok_for_every_other_verb() {
    let call = Verb::Call {
        namespace: "jobs".to_string(),
        command: "jobs_list".to_string(),
        input: serde_json::json!({}),
        confirm: None,
    };
    assert_eq!(
        exit_code_for_reply(&call, &serde_json::json!({ "dispatched": true })),
        0
    );
    assert_eq!(
        exit_code_for_reply(&call, &serde_json::json!({ "dispatched": false })),
        2,
        "a call refusal is exit 2, never exit 1"
    );
    assert_eq!(
        exit_code_for_reply(&Verb::Schema, &serde_json::json!({ "ok": true })),
        0
    );
    assert_eq!(
        exit_code_for_reply(&Verb::Schema, &serde_json::json!({ "ok": false })),
        1
    );
}

/// ADR-038 §4 (Phase 3): "needs confirmation" is its OWN exit code, never
/// collapsed into the exit-2 "refusal" bucket every other `dispatched:false`
/// cause shares.
#[test]
fn exit_code_for_reply_reports_4_for_confirmation_required_and_2_for_every_other_refusal() {
    let call = Verb::Call {
        namespace: "documents".to_string(),
        command: "documents_remove".to_string(),
        input: serde_json::json!({}),
        confirm: None,
    };
    assert_eq!(
        exit_code_for_reply(
            &call,
            &serde_json::json!({ "dispatched": false, "error": "confirmation_required" }),
        ),
        4
    );
    for other_error in [
        "confirmation_mismatch",
        "proof_unavailable",
        "unknown_command",
        // Issue #1135's new app-side refusal: the reply that used to arrive
        // as a content-free `connection_lost` (exit 2 via a synthesized
        // client error) now arrives as a real, self-describing app refusal —
        // and must land on the SAME exit code, so a caller's existing
        // "exit 2 = the call did not produce a result" branch keeps working.
        agent_call::ERR_RESULT_TOO_LARGE,
    ] {
        assert_eq!(
            exit_code_for_reply(
                &call,
                &serde_json::json!({ "dispatched": false, "error": other_error }),
            ),
            2,
            "{other_error} must stay exit 2, not be confused with confirmation_required"
        );
    }
}
// ── `run_verb_within` (MAJOR fix — security review round 2): the
// WHOLE-INVOCATION deadline, generic over both the budget and the inner
// future so it's testable without waiting out the real
// `INVOCATION_TIMEOUT` or standing up a pointer file/token/socket. ────────

#[tokio::test]
async fn run_verb_within_reports_timeout_when_the_inner_future_never_resolves() {
    let budget = Duration::from_millis(50);
    // Bounded well past `budget` so a regression that re-arms or drops
    // the deadline fails this test instead of hanging the suite.
    let outcome = tokio::time::timeout(
        budget * 4,
        run_verb_within("schema", budget, std::future::pending::<i32>()),
    )
    .await;
    assert_eq!(
        outcome.ok(),
        Some(2),
        "an expired overall deadline must exit 2, the same as any other exit-2 reply"
    );
}

#[tokio::test]
async fn run_verb_within_returns_the_inner_futures_own_exit_code_when_it_finishes_first() {
    // The normal case, unaffected by this fix: a `run_verb` that
    // finishes well inside its budget must return ITS OWN exit code
    // unchanged, never be reinterpreted by the deadline wrapper.
    let budget = Duration::from_secs(5);
    let code = run_verb_within("schema", budget, std::future::ready(0)).await;
    assert_eq!(code, 0);

    let code = run_verb_within("schema", budget, std::future::ready(1)).await;
    assert_eq!(code, 1);
}
