//! Tests for the cheap/best-matches token buckets (`throttle.rs`).

use super::super::throttle::{AGENT_CHEAP_BURST, AGENT_CHEAP_REFILL_SECS};
use super::super::*;

#[test]
fn cheap_bucket_allows_a_burst_then_refuses() {
    let mut t = AgentQueryThrottle::new();
    let now = std::time::Instant::now();
    for _ in 0..(AGENT_CHEAP_BURST as usize) {
        assert!(t.try_acquire_at(RES_SCHEMA, now));
    }
    assert!(!t.try_acquire_at(RES_SCHEMA, now), "cheap burst exhausted");
}

/// `agent_call::PAGINATED_LIST_NOTE` spells THESE two constants out in
/// prose for a consumer that cannot read this source, and it cannot
/// import them (they are private here) — so the pin lives on this side,
/// where both are visible, and is `format!`-derived rather than a third
/// hand-written copy. Change either constant, or the wording in the note,
/// and this fails.
#[test]
fn the_paged_row_note_spells_out_this_modules_cheap_throttle_numbers() {
    let note = crate::extension_bridge::agent_call::reshape::PAGINATED_LIST_NOTE;
    for expected in [
        format!("burst {}", AGENT_CHEAP_BURST as usize),
        format!("every {} s", AGENT_CHEAP_REFILL_SECS as usize),
    ] {
        assert!(
            note.contains(&expected),
            "the paged-row note must state `{expected}`: {note}"
        );
    }
}

#[test]
fn best_matches_bucket_is_much_tighter_than_cheap() {
    let mut t = AgentQueryThrottle::new();
    let now = std::time::Instant::now();
    assert!(t.try_acquire_at(RES_BEST_MATCHES, now));
    assert!(
        !t.try_acquire_at(RES_BEST_MATCHES, now),
        "best-matches burst is 1"
    );
    // The cheap bucket is a wholly separate instance — unaffected.
    assert!(t.try_acquire_at(RES_JOB, now));
}

#[test]
fn best_matches_bucket_refills_slowly() {
    let mut t = AgentQueryThrottle::new();
    let t0 = std::time::Instant::now();
    assert!(t.try_acquire_at(RES_BEST_MATCHES, t0));
    assert!(!t.try_acquire_at(RES_BEST_MATCHES, t0));
    let almost = t0 + std::time::Duration::from_secs_f64(AGENT_BEST_MATCHES_REFILL_SECS - 1.0);
    assert!(
        !t.try_acquire_at(RES_BEST_MATCHES, almost),
        "must not refill before a full interval"
    );
    let full = t0 + std::time::Duration::from_secs_f64(AGENT_BEST_MATCHES_REFILL_SECS);
    assert!(t.try_acquire_at(RES_BEST_MATCHES, full));
}

// ── forbidden-key sweep across every non-schema resource ────────────────
