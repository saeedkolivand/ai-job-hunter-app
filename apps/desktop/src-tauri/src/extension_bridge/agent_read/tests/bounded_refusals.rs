//! Tests for the bounded refusals and the success-path frame cap (issue #1151).

use super::super::reply::{
    bounded_result_reply, origin_refused_reply, throttled_reply, CLI_ONLY_MESSAGE,
};
use super::super::*;

/// Mirrors `agent_call`'s own `a_refusal_built_from_a_cap_sized_identifier_still_fits_the_frame_cap`
/// — a `resource` at the incoming frame cap must still fit the OUTGOING one once clamped, and the
/// clamp (not the last-resort envelope) must be what made it fit.
#[test]
fn a_throttled_reply_built_from_a_cap_sized_resource_still_fits_the_frame_cap() {
    let cap = super::super::super::MAX_FRAME_BYTES;
    let huge = "n".repeat(cap);
    let payload = json!({ "resource": huge.clone() });
    let reply = throttled_reply(&huge, &payload, 1_000);
    assert!(
        reply.len() <= cap,
        "the refusal is {} B, over the {cap} B cap it exists to enforce",
        reply.len()
    );
    let parsed: Value = serde_json::from_str(&reply).expect("the refusal is valid JSON");
    let clamped = crate::extension_bridge::agent_call::clamp_ident(&huge).to_string();
    assert_eq!(
        parsed["payload"]["resource"], clamped,
        "the resource must be CLAMPED, not dropped"
    );
    assert_eq!(parsed["reqId"], clamped);
    assert_ne!(
        parsed["payload"]["detail"],
        crate::extension_bridge::agent_call::REFUSAL_UNDELIVERABLE_DETAIL,
        "fitting via the last-resort envelope means the clamp did not do its job"
    );
}

/// Issue #1151 HIGH review finding A2-r1-AC-1: the two mutation cases above only prove
/// `bounded_result_reply` itself is correct, never that its TWO call sites — this one and
/// `handle_agent_query`'s — actually route through it. `origin_refused_reply` takes no
/// `AppHandle`, so unlike `handle_agent_query` it CAN be driven directly: mirrors the
/// `throttled_reply` cap-sized test above, one call site over. Reverting
/// `origin_refused_reply`'s call from `bounded_result_reply` back to a raw `agent_result_reply`
/// (the review's "Mutation 3") makes this fail — an unclamped cap-sized `resource` blows the
/// reply past `MAX_FRAME_BYTES`.
#[test]
fn origin_refused_reply_built_from_a_cap_sized_resource_still_fits_the_frame_cap() {
    let cap = super::super::super::MAX_FRAME_BYTES;
    let huge = "n".repeat(cap);
    let payload = json!({ "resource": huge.clone() });
    let reply = origin_refused_reply(&huge, &payload);
    assert!(
        reply.len() <= cap,
        "the refusal is {} B, over the {cap} B cap it exists to enforce",
        reply.len()
    );
    let parsed: Value = serde_json::from_str(&reply).expect("the refusal is valid JSON");
    let clamped = crate::extension_bridge::agent_call::clamp_ident(&huge).to_string();
    assert_eq!(
        parsed["payload"]["resource"], clamped,
        "the resource must be CLAMPED, not dropped"
    );
    assert_eq!(parsed["reqId"], clamped);
    assert_eq!(
        parsed["payload"]["error"], CLI_ONLY_MESSAGE,
        "a cap-sized resource must not push this refusal into the result_too_large fallback"
    );
}

/// `handle_agent_query`'s OTHER call site of `bounded_result_reply` cannot be driven directly the
/// same way — it is `async fn(app: &AppHandle, ..)` and this crate has no `tauri::test` mock-app
/// harness (see `extension_bridge::test::spawn_detached_runs_without_an_ambient_tokio_runtime`'s
/// doc for why that's a deliberately deferred, separately-reviewed change, not an oversight here).
/// A literal scan of this module's own source is the fallback this repo already uses for the
/// identical problem (`tests/architecture.rs`'s `job_complete_sites_in`): assert
/// `handle_agent_query`'s body still ends in `bounded_result_reply(..)`, never a bare
/// `agent_result_reply(..)` (the review's "Mutation 2"). Catches the mutation via a source-text
/// scan of `handle_agent_query`'s own body, not `bounded_result_reply`'s.
#[test]
fn handle_agent_query_routes_its_reply_through_the_frame_capped_builder() {
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/extension_bridge/agent_read.rs"
    ));
    let start = src
        .find("pub(super) async fn handle_agent_query")
        .expect("handle_agent_query must still exist under this exact signature");
    let body = &src[start..];
    let end = body
        .find("\n}\n")
        .expect("handle_agent_query's closing brace")
        + "\n}\n".len();
    let body = &body[..end];
    assert!(
        body.contains("bounded_result_reply("),
        "handle_agent_query must build its reply via bounded_result_reply (issue #1151's frame cap)"
    );
    assert!(
        !body.contains("agent_result_reply("),
        "handle_agent_query must not fall back to the raw, unbounded agent_result_reply"
    );
    // Issue #1151 AC-3: the SAME source scan pins the fallback arm's embedded resource name too
    // — `bounded_result_reply` only clamps the envelope's `resource`/`reqId`, not a copy inside
    // the "unknown agent resource '…'" message itself, so that copy must be clamped inline.
    assert!(
        body.contains("clamp_ident(other)"),
        "the fallback arm's 'unknown agent resource' message must clamp `other` inline, not \
         just rely on bounded_result_reply's envelope clamp"
    );
}

/// Issue #1151 AC-3 (MEDIUM review finding): behavioral half of the scan test above — with `other`
/// clamped inline (mirrors `handle_agent_query`'s fallback arm exactly), a cap-sized unknown
/// resource still reports its REAL cause instead of collapsing into the generic
/// `result_too_large` sentinel, which used to send the next debugger to the wrong place (a
/// "narrow the request" hint for what was actually a plain unrecognized resource name).
#[test]
fn unknown_resource_error_clamps_the_embedded_resource_so_the_real_cause_survives_the_frame_cap() {
    let cap = super::super::super::MAX_FRAME_BYTES;
    let huge = "n".repeat(cap);
    let outcome: AppResult<Value> = Err(AppError::Validation(format!(
        "unknown agent resource '{}'",
        crate::extension_bridge::agent_call::clamp_ident(&huge)
    )));
    let reply = bounded_result_reply("req-6", &huge, outcome);
    assert!(
        reply.len() <= cap,
        "the reply is {} B, over the cap",
        reply.len()
    );
    let parsed: Value = serde_json::from_str(&reply).unwrap();
    assert_ne!(
        parsed["payload"]["error"],
        crate::extension_bridge::agent_call::ERR_RESULT_TOO_LARGE,
        "a cap-sized unknown resource must not collapse into the generic result_too_large \
         sentinel — that hides the real cause behind the wrong remedy"
    );
    assert!(
        parsed["payload"]["error"]
            .as_str()
            .unwrap()
            .starts_with("unknown agent resource '"),
        "the real cause must survive: {}",
        parsed["payload"]["error"]
    );
}

/// Issue #1151's own test: an over-cap SUCCESS reply (a resource fn's own data, not a refusal)
/// must be substituted with a `result_too_large` refusal that itself fits — mirrors
/// `agent_call::enforce_frame_cap`'s own guard, one wire type over.
#[test]
fn bounded_result_reply_refuses_an_oversized_success_payload_with_result_too_large() {
    let cap = super::super::super::MAX_FRAME_BYTES;
    let oversized = json!({ "padding": "x".repeat(cap + 1) });
    let reply = bounded_result_reply("req-4", RES_JOB, Ok(oversized));
    assert!(
        reply.len() <= cap,
        "the substitute itself must fit: {} B",
        reply.len()
    );
    let parsed: Value = serde_json::from_str(&reply).expect("the substitute is valid JSON");
    let p = &parsed["payload"];
    assert_eq!(p["ok"], false);
    assert_eq!(
        p["error"],
        crate::extension_bridge::agent_call::ERR_RESULT_TOO_LARGE
    );
    assert!(p["detail"].as_str().unwrap().contains("frame cap"));
}

/// The other direction: an ordinary under-cap reply must pass through untouched.
#[test]
fn bounded_result_reply_passes_an_under_cap_reply_through_untouched() {
    let reply = bounded_result_reply("req-5", RES_SCHEMA, Ok(schema_value()));
    let parsed: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(parsed["payload"]["ok"], true);
    assert_eq!(parsed["payload"]["resource"], RES_SCHEMA);
}

// ── PR1 — extension read tier: the autofill gate + the extension's own reply cap ──────────────
