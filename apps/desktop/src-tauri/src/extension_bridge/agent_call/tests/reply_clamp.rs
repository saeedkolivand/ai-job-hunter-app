//! Tests for the identifier clamp and the bounded-refusal-reply guarantee (`reply.rs`).

use super::super::super::agent_cli::policy::POLICY;
use super::super::reply::REFUSAL_IDENT_CAP;
use super::super::*;

/// The reported defect, reproduced at its reported size: `reqId`, `namespace`
/// and `command` are caller-supplied and bounded only by the 8 MiB INCOMING
/// frame, so a cap-sized `command` used to make the `result_too_large`
/// substitute measure 8,389,135 B against an 8,388,608 B ceiling — a refusal
/// that reproduced the failure it was reporting. Covers all THREE refusal
/// paths, including the two (`throttled_reply`/`origin_refused_reply`) that
/// never pass through `enforce_frame_cap` at all.
///
/// The `assert_eq!` on the clamped identifier is what makes this a real
/// mutation check: `refusal_reply`'s measure-and-degrade fallback would keep
/// the length assertion green on its own, so the test also insists the reply
/// still NAMES its target and carries its REAL detail — i.e. that the clamp,
/// not the last-resort envelope, is what made it fit.
#[test]
fn a_refusal_built_from_a_cap_sized_identifier_still_fits_the_frame_cap() {
    let cap = super::super::super::MAX_FRAME_BYTES;
    let huge = "n".repeat(cap);
    let payload = json!({ "namespace": huge.clone(), "command": huge.clone() });

    let cases = [
        ("throttled", throttled_reply(&huge, &payload, 1_500)),
        ("origin_refused", origin_refused_reply(&huge, &payload)),
        (
            "result_too_large",
            enforce_frame_cap(&huge, &huge, &huge, "x".repeat(cap + 1), true).0,
        ),
    ];

    for (label, reply) in cases {
        assert!(
            reply.len() <= cap,
            "{label}: the refusal is {} B, over the {cap} B cap it exists to enforce",
            reply.len()
        );

        let parsed: Value = serde_json::from_str(&reply).expect("the refusal is valid JSON");
        let payload = &parsed["payload"];
        assert_eq!(parsed["type"], super::super::super::msg::AGENT_CALL_RESULT);
        assert!(!payload["dispatched"].as_bool().unwrap());

        let clamped = "n".repeat(REFUSAL_IDENT_CAP);
        assert_eq!(
            payload["namespace"].as_str().unwrap(),
            clamped,
            "{label}: the identifier must be CLAMPED, not dropped"
        );
        assert_eq!(payload["command"].as_str().unwrap(), clamped);
        assert_eq!(parsed["reqId"].as_str().unwrap(), clamped);
        assert_ne!(
            payload["detail"].as_str().unwrap(),
            REFUSAL_UNDELIVERABLE_DETAIL,
            "{label}: fitting via the last-resort envelope means the clamp did not do its job"
        );
    }
}

/// The other direction: the clamp must be invisible to every identifier that
/// can really occur. Driven off the REAL `POLICY` table rather than a
/// hand-picked sample, so a future row long enough to be truncated fails here
/// instead of silently shipping a refusal that misnames its own target.
#[test]
fn the_identifier_clamp_leaves_every_real_identifier_untouched() {
    for name in [
        "",
        "jobs",
        "jobs_list",
        "req-1",
        "documents_export_document",
    ] {
        assert_eq!(clamp_ident(name), name);
    }
    for entry in POLICY {
        let (namespace, command) = split_path(entry.path);
        assert_eq!(clamp_ident(namespace), namespace);
        assert_eq!(clamp_ident(command), command);
    }

    // End to end: an ordinary refusal still echoes both verbatim and carries
    // its own real detail.
    let reply = throttled_reply(
        "req-9",
        &json!({ "namespace": "jobs", "command": "jobs_list" }),
        2_000,
    );
    let parsed: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(parsed["payload"]["namespace"], "jobs");
    assert_eq!(parsed["payload"]["command"], "jobs_list");
    assert_eq!(parsed["reqId"], "req-9");
    assert_eq!(
        parsed["payload"]["detail"],
        super::super::super::agent_read::THROTTLED_MESSAGE
    );
    assert_eq!(parsed["payload"]["retryAfterMs"], 2_000);
}

/// Issue #1155's own two-part ask for the `call-*` tier: `rate_limited` carries a positive
/// `retryAfterMs` (never invented — passed straight through from the caller, who reads it off
/// the shared bucket) and the sentinel/detail split every other refusal on this surface already
/// uses.
#[test]
fn throttled_reply_carries_a_positive_retry_after_and_the_rate_limited_sentinel() {
    let reply = throttled_reply(
        "req-throttle",
        &json!({ "namespace": "autopilot", "command": "autopilot_best_matches" }),
        30_000,
    );
    let parsed: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(parsed["payload"]["error"], ERR_RATE_LIMITED);
    assert_eq!(parsed["payload"]["retryAfterMs"], 30_000);
    assert!(parsed["payload"]["retryAfterMs"].as_u64().unwrap() > 0);
    assert_eq!(
        parsed["payload"]["detail"],
        super::super::super::agent_read::THROTTLED_MESSAGE
    );
    // Identity — the refused command is still named, same as every other refusal here.
    assert_eq!(parsed["payload"]["namespace"], "autopilot");
    assert_eq!(parsed["payload"]["command"], "autopilot_best_matches");
}

/// [A2-r2-AC-r2-1] `retryAfterMs` must be ABSENT (not present-as-`null`) on every
/// non-throttle refusal — `call_result_reply` used to always insert the key,
/// disagreeing with `agent_read::sentinel_refusal_reply`'s `extra` merge
/// (`json!({})` for `bounded_result_reply`, i.e. no key at all). A client
/// keying on `'retryAfterMs' in payload` must see the SAME presence/absence
/// split on both tiers, or it waits 0 ms on a refusal that was never a
/// throttle.
#[test]
fn a_non_throttle_refusal_never_carries_a_retry_after_key() {
    let reply = origin_refused_reply(
        "req-1",
        &json!({ "namespace": "jobs", "command": "jobs_list" }),
    );
    let parsed: Value = serde_json::from_str(&reply).unwrap();
    let payload = parsed["payload"].as_object().expect("payload is an object");
    assert!(
        !payload.contains_key("retryAfterMs"),
        "a non-throttle refusal must omit `retryAfterMs` entirely, not set it to null: {payload:?}"
    );

    // The throttle refusal is the ONE case that carries the key, and it must
    // be a real number, never null.
    let throttled = throttled_reply(
        "req-2",
        &json!({ "namespace": "jobs", "command": "jobs_list" }),
        1_000,
    );
    let parsed: Value = serde_json::from_str(&throttled).unwrap();
    assert!(parsed["payload"]["retryAfterMs"].is_u64());
}

/// `&value[..REFUSAL_IDENT_CAP]` panics when the cap lands mid-codepoint, and
/// release is `panic = "abort"` — inside a frame handler that is a silent
/// process death, so the boundary walk is load-bearing, not tidiness. 256 is
/// not a multiple of 3, so the 3-byte case exercises the walk itself.
#[test]
fn the_identifier_clamp_cuts_on_a_char_boundary() {
    for wide in ["字", "é", "🙂"] {
        let value = wide.repeat(500);
        let clamped = clamp_ident(&value);
        assert!(
            clamped.len() <= REFUSAL_IDENT_CAP,
            "{wide}: clamped to {} B",
            clamped.len()
        );
        assert!(
            value.starts_with(clamped),
            "{wide}: the clamp must be a prefix, never a re-encode"
        );
        // Nothing was cut in half: the prefix round-trips as real UTF-8 and
        // every char in it is the original one.
        assert!(clamped.chars().all(|c| c.to_string() == wide));
        assert!(
            clamped.len() > REFUSAL_IDENT_CAP - 4,
            "{wide}: the walk must back up to the nearest boundary, not much further"
        );
    }
}

// ── reshape_reply ordering (backend-architect review: nothing pinned the
// three response steps to an order) ──
