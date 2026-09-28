//! `BridgeState`'s throttle-bucket WIRING tests — redistributed from the crate-level `test.rs`
//! (R8 relief). Each bucket's own math (burst/refill/isolation) is unit-tested where the bucket
//! type itself lives (e.g. `match_live_throttle::tests`, `agent_read::tests::throttle`); these
//! tests instead go through the `BridgeState` method a real dispatch loop calls, so a bug in
//! *that* wiring — not just the bucket — fails here.
//!
//! `connected` is a refcount, not a last-writer-wins flag, so pairing a second browser and then
//! closing ONE of them must not report "disconnected" while the other socket is still open (the
//! bug this fixes: whichever socket closed LAST used to decide connectivity for every other
//! still-open one).

use super::super::*;

use super::super::super::test_support::bridge_state;

#[test]
fn two_authenticated_sockets_one_closing_stays_connected() {
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());
    assert!(!s.is_connected());

    assert!(s.inc_connected(), "first auth is the 0→1 transition");
    assert!(s.is_connected());
    assert!(
        !s.inc_connected(),
        "second browser pairing with the same token is 1→2, not a transition"
    );
    assert!(s.is_connected());

    assert!(
        !s.dec_connected(),
        "one socket closing (2→1) must not report the last-connection transition"
    );
    assert!(
        s.is_connected(),
        "the other browser is still paired — must still read connected"
    );

    assert!(
        s.dec_connected(),
        "the second socket closing is the real 1→0 transition"
    );
    assert!(!s.is_connected());
}

#[test]
fn dec_connected_without_a_prior_increment_saturates_at_zero() {
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());
    assert!(!s.is_connected());

    // Mirrors an unauthenticated socket's teardown (rejected origin, failed
    // proof, over-cap/outdated first frame): `handle_connection` never calls
    // `inc_connected` for it, so this must be a no-op. Also proves the
    // saturating decrement itself: an `AtomicUsize::fetch_sub` on a zero count
    // would otherwise wrap to `usize::MAX`, which `is_connected` (`count > 0`)
    // would misreport as connected.
    assert!(!s.dec_connected());
    assert!(
        !s.is_connected(),
        "an unmatched decrement must never wrap below zero"
    );
}

// ── match.live throttle (MEDIUM: reconnect-proof, lives on BridgeState) ──────

#[test]
fn match_live_throttle_survives_reconnect() {
    // A per-connection instance (the pre-fix design) would hand a brand-new,
    // full bucket to every socket — including a reconnect, which on a
    // loopback WS is a cheap, near-instant handshake an automated client can
    // trivially repeat. The bucket must live on BridgeState instead, so it
    // survives across connections.
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());

    for _ in 0..3 {
        assert!(
            s.try_acquire_match_live(),
            "burst allowance on the first connection"
        );
    }
    assert!(
        !s.try_acquire_match_live(),
        "burst exhausted on the first connection"
    );

    // Simulate a reconnect: a fresh socket/task against the SAME BridgeState
    // (the one Tauri manages for the app's whole lifetime) — must NOT see a
    // refreshed bucket.
    assert!(
        !s.try_acquire_match_live(),
        "a reconnect must not reset the match.live token bucket"
    );
}

#[test]
fn match_live_throttle_shared_across_sequential_connections() {
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());

    // "Connection 1" spends part of the shared burst.
    assert!(s.try_acquire_match_live());
    assert!(s.try_acquire_match_live());

    // "Connection 2" (a later socket against the same BridgeState) only gets
    // what's LEFT of the shared budget, not a fresh burst of its own.
    assert!(
        s.try_acquire_match_live(),
        "one token remains in the shared budget"
    );
    assert!(
        !s.try_acquire_match_live(),
        "the shared budget is exhausted — connection 2 does not get its own fresh burst"
    );
}

// ── agent.query throttle (MEDIUM: reconnect-proof, lives on BridgeState) ────
// Mirrors `match_live_throttle_survives_reconnect` above: every OTHER
// `AgentQueryThrottle` test (`agent_read.rs`) constructs the struct directly
// and drives `try_acquire_at`, which proves nothing about the wiring through
// `BridgeState::try_acquire_agent` itself — this goes through that method,
// against one shared `BridgeState`, the same way a real reconnecting CLI
// invocation would.

#[test]
fn agent_query_throttle_survives_reconnect() {
    // `best-matches`' bucket has a burst of exactly 1 (see
    // `agent_read::AGENT_BEST_MATCHES_BURST`), so a single connection
    // exhausts it in one call — a per-connection instance (the bug this
    // guards against) would hand a fresh, full bucket to every reconnect,
    // which on a loopback WS an automated CLI invocation can trivially
    // repeat every process launch.
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());

    assert!(
        s.try_acquire_agent("best-matches"),
        "burst allowance on the first connection"
    );
    assert!(
        !s.try_acquire_agent("best-matches"),
        "burst exhausted on the first connection"
    );

    // Simulate a reconnect: a fresh socket/task against the SAME
    // BridgeState (the one Tauri manages for the app's whole lifetime) —
    // must NOT see a refreshed bucket.
    assert!(
        !s.try_acquire_agent("best-matches"),
        "a reconnect must not reset the agent.query token bucket"
    );
}

/// Issue #1155 (HIGH review finding A2-r1-AC-2, "Mutation A"): every `AgentQueryThrottle` test in
/// `agent_read.rs` constructs that struct directly and calls `retry_after_ms` on it, which proves
/// nothing about `BridgeState::agent_retry_after_ms` — the ONE method the dispatch loop in `mod.rs`
/// actually calls before building a `rate_limited` reply. Mirrors
/// `agent_query_throttle_survives_reconnect` above, one method over: goes through
/// `BridgeState::try_acquire_agent`/`agent_retry_after_ms` against a shared `BridgeState`, not the
/// bucket directly, so a bug in THAT wiring — not just in the bucket math — would fail this.
#[test]
fn bridge_state_agent_retry_after_ms_reads_the_same_bucket_try_acquire_agent_drew_from() {
    let dir = tempfile::tempdir().unwrap();
    let s = BridgeState::load(dir.path());

    assert!(s.try_acquire_agent("best-matches"), "burst allowance");
    assert!(
        !s.try_acquire_agent("best-matches"),
        "burst exhausted — the wait must now be positive"
    );
    // [A2-r2-AC-r2-3] A RANGE, not `assert_eq!` — this value is `ceil(full_wait - elapsed)`
    // against a real `Instant::now()` (no injected clock on `BridgeState`, unlike
    // `agent_read`'s own bucket tests), so even a single scheduler preemption between the two
    // `try_acquire_agent` calls above shaves whole milliseconds off it. The lower bound is still
    // impossible for a hardcoded 0/1 ms placeholder, or the cheap bucket's unrelated 1000 ms
    // refill, to satisfy — only the best-matches bucket's OWN (tighter) refill rate can land
    // here.
    let full_wait_ms =
        (crate::extension_bridge::agent_read::AGENT_BEST_MATCHES_REFILL_SECS * 1000.0) as u64;
    let wait = s.agent_retry_after_ms("best-matches");
    assert!(
        wait > full_wait_ms.saturating_sub(1_000) && wait <= full_wait_ms,
        "must read the best-matches bucket's OWN (tighter) refill rate through the wiring \
         (expected in ({}, {full_wait_ms}], got {wait})",
        full_wait_ms.saturating_sub(1_000)
    );
}

// ── settings.set throttle on BridgeState (R7, guard rail #4) ──────────────

#[test]
fn bridge_state_try_acquire_settings_set_throttles_a_burst() {
    let (_dir, state) = bridge_state();
    let mut admitted = 0;
    for _ in 0..20 {
        if state.try_acquire_settings_set() {
            admitted += 1;
        }
    }
    assert!(
        admitted < 20,
        "an unbounded burst of settings.set must eventually be throttled"
    );
    assert!(admitted > 0, "a reasonable burst must still be admitted");
}
