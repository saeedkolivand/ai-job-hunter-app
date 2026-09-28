//! retryAfterMs wiring pin (issue #1155, "Mutation A") -- redistributed from the crate-level
//! `test.rs` (R8 relief).

/// Source-text pin for the connection loop's two throttled-reply arms (issue #1155, HIGH review
/// finding A2-r1-AC-2, "Mutation A": both `FrameDecision::AgentQuery`'s and
/// `FrameDecision::AgentCall`'s arms hardcoded `retryAfterMs` to a literal `0u64`, discarding
/// `agent_retry_after_ms`'s result, and the whole suite stayed green). That loop is `start()`'s
/// own `async fn`, driven by a real socket + `AppHandle` -- this crate has no `tauri::test`
/// mock-app harness (see `server::tests::spawn_detached_runs_without_an_ambient_tokio_runtime`'s
/// doc), so it cannot be called directly the way a bucket-wiring test calls the method it wires
/// TO. A literal scan of `connection_dispatch.rs`'s own source is the fallback this repo already
/// uses for the identical problem (`tests/architecture.rs`'s `job_complete_sites_in`): assert
/// both arms still read `retry_after_ms` off `state.agent_retry_after_ms(..)` AND that the
/// computed value is what actually reaches `throttled_reply`'s third argument.
///
/// TR-04 fix (test-author round): the ORIGINAL needles here only checked that the call
/// expression `state.agent_retry_after_ms(..)` appears somewhere in the file -- never that its
/// result reaches `throttled_reply`. `let _retry_after_ms = state.agent_retry_after_ms(..); …
/// throttled_reply(&req_id, &payload, 0)` keeps both old needles green (the call expression is
/// still textually present) while hardcoding the reply back to the exact regression this test
/// exists to catch. The needles below now span the whole `let retry_after_ms = …` binding
/// through to `retry_after_ms,` as `throttled_reply`'s own third positional argument, so a
/// literal at the call site (or a renamed/unread binding) reddens this.
#[test]
fn the_throttled_dispatch_arms_read_retry_after_ms_off_bridge_state_not_a_constant() {
    let src = include_str!("../connection_dispatch.rs");
    let agent_query_needle = r#"let retry_after_ms = state.agent_retry_after_ms(agent_read::resource_name(&payload));
            Some(agent_read::throttled_reply(
                &req_id,
                &payload,
                retry_after_ms,
            ))"#;
    let agent_call_needle = r#"let retry_after_ms = state.agent_retry_after_ms(agent_call::throttle_key(command));
            Some(agent_call::throttled_reply(
                &req_id,
                &payload,
                retry_after_ms,
            ))"#;
    for needle in [agent_query_needle, agent_call_needle] {
        assert!(
            src.contains(needle),
            "connection_dispatch.rs must still bind `retry_after_ms` from \
             `state.agent_retry_after_ms(..)` and pass that SAME binding as `throttled_reply`'s \
             third argument -- a future edit that hardcodes the call site (e.g. to a literal `0`) \
             while leaving the computation dangling unread would otherwise leave every other test \
             green. Expected:\n{needle}"
        );
    }
}
