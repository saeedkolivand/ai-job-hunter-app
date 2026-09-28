use super::super::*;

use super::super::super::test_support::bridge_state;
use crate::applications::normalize_job_url;

// ─────────────────────────────────────────────────────────────────────────────

/// A frame whose length exceeds MAX_FRAME_BYTES is closed (CloseOverCap) before
/// any parse — even though it carries the correct token, it never dispatches.
#[test]
fn frame_over_size_cap_is_closed_without_dispatch() {
    let (_dir, state) = bridge_state();
    // One byte over the cap. Content is irrelevant — the size guard runs first.
    let oversize = "a".repeat(MAX_FRAME_BYTES + 1);
    assert!(oversize.len() > MAX_FRAME_BYTES);

    // The size guard runs before parse/state — state-independent.
    match advance_frame(&state, &ConnState::Authenticated, &oversize) {
        FrameDecision::CloseOverCap => {}
        other => panic!("an over-cap frame must be CloseOverCap, got {other:?}"),
    }
}

/// Exactly AT the cap is NOT over-size — it proceeds to parse (and, being
/// non-JSON here, drops). This pins the boundary so the guard is `>` not `>=`.
#[test]
fn frame_exactly_at_cap_is_not_over_size() {
    let (_dir, state) = bridge_state();
    let at_cap = "a".repeat(MAX_FRAME_BYTES);
    assert_eq!(at_cap.len(), MAX_FRAME_BYTES);

    // Not over-cap → it is parsed; raw "aaa…" is not JSON → Drop (not CloseOverCap).
    match advance_frame(&state, &ConnState::Authenticated, &at_cap) {
        FrameDecision::Drop => {}
        other => panic!("a frame exactly at the cap must parse (Drop here), got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// B3. SSRF private-host import rejection (MEDIUM) + non-JSON drop
//
// An authenticated import.request whose `url` is a private/loopback host must be
// rejected by the SSRF gate `handle_import` applies (`is_safe_import_url`) before
// any persistence. advance_frame (in the Authenticated state) routes it to Import;
// we then assert the exact gate handle_import runs rejects the host, so nothing
// is ever persisted.
// ─────────────────────────────────────────────────────────────────────────────

/// A private-host import URL classifies as Import (session-authenticated) but is
/// blocked by the host SSRF guard handle_import applies before persisting.
#[test]
fn frame_private_host_import_is_blocked_by_ssrf_gate() {
    let (_dir, state) = bridge_state();
    let private_url = "http://192.168.1.1/job";

    // v2: no token on the frame; the socket is already session-authenticated.
    let frame = json!({
        "reqId": "r-ssrf",
        "type": msg::IMPORT_REQUEST,
        "payload": { "url": private_url }
    })
    .to_string();

    // Authenticated → Import; the URL is non-empty after normalization …
    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::Import { payload, .. } => {
            let url = payload.get("url").and_then(|u| u.as_str()).unwrap();
            assert!(
                !normalize_job_url(url).is_empty(),
                "a http private URL still normalizes non-empty (scheme is allowed)"
            );
            // … but the SSRF host guard handle_import runs rejects the private host,
            // returning the exact 'url host is not allowed' error → no persistence.
            assert!(
                !super::super::super::auth::is_safe_import_url(url),
                "private host must be rejected by the import SSRF guard"
            );
        }
        other => panic!("authenticated import must classify as Import, got {other:?}"),
    }
}

/// A non-JSON frame (with no token field) is dropped silently — no reply, no
/// dispatch.
#[test]
fn frame_non_json_is_dropped() {
    let (_dir, state) = bridge_state();
    match advance_frame(&state, &ConnState::Authenticated, "this is not json {") {
        FrameDecision::Drop => {}
        other => panic!("non-JSON must Drop, got {other:?}"),
    }
}
