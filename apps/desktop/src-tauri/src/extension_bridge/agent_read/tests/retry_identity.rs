//! Tests for `retryAfterMs` and the refused-request identity on the throttle envelope (issue #1155).

use super::super::throttle::{AGENT_CHEAP_BURST, AGENT_CHEAP_REFILL_SECS};
use super::super::*;

#[test]
fn token_bucket_retry_after_ms_is_zero_with_a_token_available_and_positive_once_exhausted() {
    let mut t = AgentQueryThrottle::new();
    let now = std::time::Instant::now();
    assert_eq!(
        t.retry_after_ms(RES_SCHEMA),
        0,
        "a fresh bucket has a token ready"
    );
    for _ in 0..(AGENT_CHEAP_BURST as usize) {
        assert!(t.try_acquire_at(RES_SCHEMA, now));
    }
    assert!(!t.try_acquire_at(RES_SCHEMA, now));
    assert!(
        t.retry_after_ms(RES_SCHEMA) > 0,
        "an exhausted bucket must report a positive wait"
    );
}

/// Issue #1155 (HIGH review finding A2-r1-AC-2): `> 0` alone is satisfied by ANY positive
/// constant, including a hardcoded `1`ms that would reproduce the reported transcript (a caller
/// retrying instantly, getting throttled again, and abandoning the request). Anchor each bucket
/// to its OWN refill rate — the two differ (1 s vs 30 s), so no single constant can satisfy both,
/// closing the exact gap the review's "Mutation B" (`retry_after_ms` hardcoded to a constant
/// `1`ms) exploited.
#[test]
fn retry_after_ms_is_anchored_to_each_buckets_own_refill_rate_once_exhausted() {
    let mut t = AgentQueryThrottle::new();
    let now = std::time::Instant::now();

    for _ in 0..(AGENT_CHEAP_BURST as usize) {
        assert!(t.try_acquire_at(RES_SCHEMA, now));
    }
    assert!(!t.try_acquire_at(RES_SCHEMA, now));
    assert_eq!(
        t.retry_after_ms(RES_SCHEMA),
        (AGENT_CHEAP_REFILL_SECS * 1000.0) as u64,
        "the cheap bucket's wait must equal its own refill interval, not a hardcoded constant"
    );

    assert!(t.try_acquire_at(RES_BEST_MATCHES, now));
    assert!(!t.try_acquire_at(RES_BEST_MATCHES, now));
    assert_eq!(
        t.retry_after_ms(RES_BEST_MATCHES),
        (AGENT_BEST_MATCHES_REFILL_SECS * 1000.0) as u64,
        "the best-matches bucket's wait must equal ITS OWN (30x longer) refill interval"
    );
}

#[test]
fn throttled_reply_carries_the_rate_limited_sentinel_a_positive_retry_after_and_the_refused_url() {
    let payload = json!({ "resource": RES_JOB, "url": "https://example.com/job/1" });
    let reply = throttled_reply("req-1", &payload, 1_000);
    let parsed: Value = serde_json::from_str(&reply).unwrap();
    let p = &parsed["payload"];
    assert_eq!(p["ok"], false);
    assert_eq!(p["resource"], RES_JOB);
    assert_eq!(
        p["error"],
        crate::extension_bridge::agent_call::ERR_RATE_LIMITED
    );
    assert_eq!(p["detail"], THROTTLED_MESSAGE);
    assert_eq!(p["retryAfterMs"], 1_000);
    assert!(p["retryAfterMs"].as_u64().unwrap() > 0);
    // The refused request's own identity — the gap issue #1155 reports: three throttled `job`
    // lookups previously looked identical (only "resource":"job", never which url).
    assert_eq!(p["url"], "https://example.com/job/1");
}

#[test]
fn throttled_reply_echoes_the_found_jobs_autopilot_id_identity() {
    let payload = json!({ "resource": RES_FOUND_JOBS, "autopilotId": "ap-9" });
    let reply = throttled_reply("req-2", &payload, 500);
    let parsed: Value = serde_json::from_str(&reply).unwrap();
    assert_eq!(parsed["payload"]["autopilotId"], "ap-9");
}

#[test]
fn throttled_reply_names_no_identity_for_a_resource_that_has_none() {
    let payload = json!({ "resource": RES_BEST_MATCHES });
    let reply = throttled_reply("req-3", &payload, 100);
    let parsed: Value = serde_json::from_str(&reply).unwrap();
    assert!(parsed["payload"].get("url").is_none());
    assert!(parsed["payload"].get("autopilotId").is_none());
}

// ── issue #1151 — bounded refusals + the success-path frame cap ──────────
