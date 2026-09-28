use super::super::*;

use super::super::super::test_support::bridge_state;
use super::{awaiting_auth, reply_error};

// ─────────────────────────────────────────────────────────────────────────────
// B1c. Post-auth dispatch — import/profile ONLY in the Authenticated state
// ─────────────────────────────────────────────────────────────────────────────

/// An authenticated `import.request` classifies as `Import` (positive control),
/// carrying the payload verbatim to `handle_import`.
#[test]
fn authenticated_import_classifies_as_import() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::IMPORT_REQUEST,
        "reqId": "r-ok",
        "payload": { "url": "https://jobs.example.com/posting/3" }
    })
    .to_string();

    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::Import { req_id, payload } => {
            assert_eq!(req_id, "r-ok");
            assert_eq!(
                payload.get("url").and_then(|u| u.as_str()),
                Some("https://jobs.example.com/posting/3")
            );
        }
        other => panic!("an authenticated import.request must be Import, got {other:?}"),
    }
}

/// An authenticated `profile.get` classifies as `Profile`.
#[test]
fn authenticated_profile_get_classifies_as_profile() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::PROFILE_GET,
        "reqId": "r-prof",
        "payload": serde_json::Value::Null,
    })
    .to_string();
    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::Profile { req_id } => assert_eq!(req_id, "r-prof"),
        other => panic!("an authenticated profile.get must be Profile, got {other:?}"),
    }
}

/// An unrecognized type in the Authenticated state gets an `import.result`
/// error reply (never a panic). `match.live` used to be the last type
/// exercising this fallback (RESERVED, unhandled) — now that it dispatches to
/// a real handler (see `authenticated_match_live_classifies_as_match_live`
/// below), this covers the fallback with a genuinely unknown verb instead.
#[test]
fn authenticated_unknown_type_replies_with_error() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": "totally.unknown.verb",
        "reqId": "r-unknown",
        "payload": serde_json::Value::Null,
    })
    .to_string();
    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::Reply(reply) => {
            let err = reply_error(&reply).expect("unknown type must carry an error");
            assert!(err.contains("unknown message type"), "got {err:?}");
        }
        other => panic!("an unknown type must be a Reply(error), got {other:?}"),
    }
}

/// An authenticated `match.live` classifies as `MatchLive`, carrying the
/// payload verbatim (mirrors the `Import`/`AppliedCheck` classify tests).
#[test]
fn authenticated_match_live_classifies_as_match_live() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::MATCH_LIVE,
        "reqId": "r-match",
        "payload": { "url": "https://jobs.example.com/posting/7", "html": "<html></html>" }
    })
    .to_string();

    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::MatchLive { req_id, payload } => {
            assert_eq!(req_id, "r-match");
            assert_eq!(
                payload.get("url").and_then(|u| u.as_str()),
                Some("https://jobs.example.com/posting/7")
            );
        }
        other => panic!("an authenticated match.live must be MatchLive, got {other:?}"),
    }
}

/// A `match.live` before the handshake completes is NEVER dispatched — same
/// invariant as `import_before_handshake_is_not_dispatched`: only a hello may
/// be the first frame.
#[test]
fn match_live_before_handshake_is_not_dispatched() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::MATCH_LIVE,
        "reqId": "r-early",
        "payload": { "url": "https://jobs.example.com/posting/early", "html": "<html></html>" },
    })
    .to_string();
    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(_) => {}
        other => panic!("a match.live before hello must NOT be MatchLive, got {other:?}"),
    }
}

/// Same mid-handshake bypass guard as `non_auth_frame_mid_handshake_is_unauthorized`
/// — a `match.live` cannot skip the proof step either.
#[test]
fn match_live_mid_handshake_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": msg::MATCH_LIVE,
        "reqId": "r-skip",
        "payload": { "url": "https://jobs.example.com/posting/skip", "html": "<html></html>" },
    })
    .to_string();
    match advance_frame(&state, &conn, &frame) {
        FrameDecision::Unauthorized => {}
        other => panic!("a match.live mid-handshake must be Unauthorized, got {other:?}"),
    }
}

/// An authenticated `applied.check` classifies as `AppliedCheck`, carrying the
/// payload verbatim (mirrors the `Import`/`Profile` classify tests above).
#[test]
fn authenticated_applied_check_classifies_as_applied_check() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::APPLIED_CHECK,
        "reqId": "r-applied",
        "payload": { "url": "https://jobs.example.com/posting/9" }
    })
    .to_string();

    match advance_frame(&state, &ConnState::Authenticated, &frame) {
        FrameDecision::AppliedCheck { req_id, payload } => {
            assert_eq!(req_id, "r-applied");
            assert_eq!(
                payload.get("url").and_then(|u| u.as_str()),
                Some("https://jobs.example.com/posting/9")
            );
        }
        other => panic!("an authenticated applied.check must be AppliedCheck, got {other:?}"),
    }
}

/// An `applied.check` before the handshake completes is NEVER dispatched — same
/// invariant as `import_before_handshake_is_not_dispatched`: only a hello may
/// be the first frame.
#[test]
fn applied_check_before_handshake_is_not_dispatched() {
    let (_dir, state) = bridge_state();
    let frame = json!({
        "type": msg::APPLIED_CHECK,
        "reqId": "r-early",
        "payload": { "url": "https://jobs.example.com/posting/early" },
    })
    .to_string();
    match advance_frame(&state, &ConnState::AwaitingHello, &frame) {
        FrameDecision::Outdated(_) => {}
        other => panic!("an applied.check before hello must NOT be AppliedCheck, got {other:?}"),
    }
}

/// Same mid-handshake bypass guard as `non_auth_frame_mid_handshake_is_unauthorized`
/// — an `applied.check` cannot skip the proof step either.
#[test]
fn applied_check_mid_handshake_is_unauthorized() {
    let (_dir, state) = bridge_state();
    let (conn, _correct) = awaiting_auth(&state);
    let frame = json!({
        "type": msg::APPLIED_CHECK,
        "reqId": "r-skip",
        "payload": { "url": "https://jobs.example.com/posting/skip" },
    })
    .to_string();
    match advance_frame(&state, &conn, &frame) {
        FrameDecision::Unauthorized => {}
        other => panic!("an applied.check mid-handshake must be Unauthorized, got {other:?}"),
    }
}
