//! Token-bucket throttle for `agent.query`, shared across every connection for a pairing (lives
//! on `BridgeState`, not per-connection) — split out of `agent_read.rs` under the R8 LOC cap.

use super::RES_BEST_MATCHES;

// ── Throttle (on BridgeState, not per-connection — see module doc) ─────────

/// Minimal token bucket — the exact math `match_live::MatchLiveThrottle` uses,
/// but parameterized (`burst`/`refill_secs` are fields, not consts) because
/// [`AgentQueryThrottle`] needs TWO differently-tuned instances, not one.
struct TokenBucket {
    tokens: f64,
    last: std::time::Instant,
    burst: f64,
    refill_secs: f64,
}

impl TokenBucket {
    fn new(burst: f64, refill_secs: f64) -> Self {
        Self {
            tokens: burst,
            last: std::time::Instant::now(),
            burst,
            refill_secs,
        }
    }

    fn try_acquire_at(&mut self, now: std::time::Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed / self.refill_secs).min(self.burst);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Milliseconds until this bucket would hold one full token, computed from its CURRENT
    /// fractional `tokens` count (issue #1155) — a pure read, not a second clock advance. Only
    /// meaningful called right after a failed [`Self::try_acquire_at`] in the SAME tick: that
    /// call already set `self.tokens`/`self.last` to "now", so there is nothing left to advance.
    fn retry_after_ms(&self) -> u64 {
        if self.tokens >= 1.0 {
            return 0;
        }
        let needed_secs = (1.0 - self.tokens) * self.refill_secs;
        (needed_secs * 1000.0).ceil() as u64
    }
}

/// Cheap-read bucket (`job`/`profile`/`automations`/`schema`): burst 10,
/// refilling one token/second — generous for a scripted CLI polling loop.
pub(in crate::extension_bridge::agent_read) const AGENT_CHEAP_BURST: f64 = 10.0;
pub(in crate::extension_bridge::agent_read) const AGENT_CHEAP_REFILL_SECS: f64 = 1.0;
/// `best-matches` bucket: burst 1, refilling one token every 30s. Sized off
/// the measured worst case in `commands::autopilot::autopilot_best_matches`'s
/// own doc (3.03s at 2000 found-jobs, 12.3s at 4000) — this PR calls that
/// command UNMODIFIED (issue #1084's own preference: prefer the already-public
/// fn over re-wrapping its private blocking half or duplicating its
/// clustering, both of which either widen visibility across a domain
/// boundary this PR doesn't own — `commands::autopilot` — or fork a second
/// copy of `compute_best_matches`'s logic). That leaves the compute itself
/// UN-truncated per call; this bucket is what stops repeated invocation from
/// stacking that cost, not a pre-clustering cap on `found_jobs`. A follow-up
/// in the matching domain could add a real compute-side cap if that's not
/// enough — flagged in the PR1 handoff.
const AGENT_BEST_MATCHES_BURST: f64 = 1.0;
// `pub(super)` (issue #1155) — `extension_bridge::test`'s
// `bridge_state_agent_retry_after_ms_reads_the_same_bucket_try_acquire_agent_drew_from` anchors to
// this value directly, so a `BridgeState`-level test can't be satisfied by any hardcoded constant.
pub(in crate::extension_bridge) const AGENT_BEST_MATCHES_REFILL_SECS: f64 = 30.0;

/// Token-bucket throttle for `agent.query`, shared across EVERY connection for
/// this pairing (lives on `BridgeState`, not per-connection) for the same
/// reason as `match_live::MatchLiveThrottle`: a CLI invocation is a fresh
/// process + fresh socket every time, so a per-connection bucket would be
/// bypassed by construction. A SEPARATE struct from `MatchLiveThrottle` (not
/// a generic shared one) — that struct's own doc reserves exactly this
/// scenario ("a future compute-heavy verb") for its own instance, since
/// per-verb cost profiles differ; `best-matches` alone does real CPU work
/// while the other five resources are cheap in-memory reads, so this struct
/// carries TWO independently-sized buckets rather than one shared bucket.
pub(in crate::extension_bridge) struct AgentQueryThrottle {
    cheap: TokenBucket,
    best_matches: TokenBucket,
}

impl AgentQueryThrottle {
    pub(in crate::extension_bridge) fn new() -> Self {
        Self {
            cheap: TokenBucket::new(AGENT_CHEAP_BURST, AGENT_CHEAP_REFILL_SECS),
            best_matches: TokenBucket::new(
                AGENT_BEST_MATCHES_BURST,
                AGENT_BEST_MATCHES_REFILL_SECS,
            ),
        }
    }

    /// Try to consume one token at `now` (explicit clock — directly
    /// unit-testable without a real sleep; production always goes through
    /// [`Self::try_acquire`]). An unrecognized `resource` draws from the
    /// cheap bucket — harmless, since it will fail resource-name validation
    /// right after in [`handle_agent_query`] anyway.
    pub(in crate::extension_bridge::agent_read) fn try_acquire_at(
        &mut self,
        resource: &str,
        now: std::time::Instant,
    ) -> bool {
        if resource == RES_BEST_MATCHES {
            self.best_matches.try_acquire_at(now)
        } else {
            self.cheap.try_acquire_at(now)
        }
    }

    pub(in crate::extension_bridge) fn try_acquire(&mut self, resource: &str) -> bool {
        self.try_acquire_at(resource, std::time::Instant::now())
    }

    /// [`TokenBucket::retry_after_ms`] for whichever bucket `resource` draws from — same routing
    /// [`Self::try_acquire_at`] uses, so the two can never disagree about which bucket a resource
    /// belongs to. `pub(super)` (issue #1155) — `BridgeState::agent_retry_after_ms` is the one
    /// caller, reached right after a failed `try_acquire` for the same resource.
    pub(in crate::extension_bridge) fn retry_after_ms(&self, resource: &str) -> u64 {
        if resource == RES_BEST_MATCHES {
            self.best_matches.retry_after_ms()
        } else {
            self.cheap.retry_after_ms()
        }
    }
}
