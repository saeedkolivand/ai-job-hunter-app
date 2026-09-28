//! `match.live`'s dedicated per-pairing throttle — split from `match_live.rs` (R8 relief); see
//! that module's own doc for why this lives on `BridgeState` rather than per-connection.

use crate::error::AppError;

use super::match_live::match_result_reply;

/// Minimal token-bucket throttle for `match.live`. A deliberate "Check fit"
/// click can legitimately fire a few times in quick succession (a
/// double-click, a retry after fixing the résumé), but an unbounded stream of
/// clicks/automation should not be free to keep re-running the scorer. Burst
/// [`MATCH_LIVE_BURST`] requests, refilling one token every
/// [`MATCH_LIVE_REFILL_SECS`] (~1 req/2s sustained).
///
/// Lives on [`super::BridgeState`] (behind a `Mutex`, shared across EVERY
/// connection for this pairing) rather than per-connection — the MEDIUM
/// "reconnect-proof throttle" fix. A loopback reconnect is a cheap,
/// near-instant handshake (see `handle_connection`'s doc); a per-connection
/// instance would hand a fresh full burst to every reconnect, so an automated
/// client could trivially bypass the throttle just by reconnecting. The
/// bucket must outlive any single socket.
///
/// Scoped to `match.live` only this round: a future compute-heavy verb would
/// give itself its own throttle instance with its own constants (each verb's
/// cost profile differs) rather than share this one, so this struct is
/// deliberately NOT made generic/shared across verbs.
pub(super) struct MatchLiveThrottle {
    tokens: f64,
    last: std::time::Instant,
}

/// Requests allowed in quick succession before the bucket empties.
const MATCH_LIVE_BURST: f64 = 3.0;
/// Seconds to refill one token (~1 sustained request every this many seconds).
const MATCH_LIVE_REFILL_SECS: f64 = 2.0;

impl MatchLiveThrottle {
    pub(super) fn new() -> Self {
        Self {
            tokens: MATCH_LIVE_BURST,
            last: std::time::Instant::now(),
        }
    }

    /// Try to consume one token at `now` (an explicit clock so the refill
    /// math is directly unit-testable without a real sleep; production call
    /// sites always go through [`Self::try_acquire`]). Returns `true` (and
    /// consumes a token) when the request may proceed.
    fn try_acquire_at(&mut self, now: std::time::Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed / MATCH_LIVE_REFILL_SECS).min(MATCH_LIVE_BURST);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub(super) fn try_acquire(&mut self) -> bool {
        self.try_acquire_at(std::time::Instant::now())
    }
}

/// Fixed sentinel for [`MatchLiveThrottle`]'s refusal — deliberately generic
/// wording (not scorer/verb-specific) so it reads sensibly if a future verb
/// ever reuses the same shape.
pub(super) const THROTTLED_MESSAGE: &str = "Too many requests — try again shortly.";

/// Build the `match.result` reply for a throttled `match.live` request — a
/// `RateLimited` refusal is just another `ok:false` outcome on the SAME
/// discriminated reply [`match_result_reply`] already builds.
pub(super) fn throttled_reply(req_id: &str) -> String {
    match_result_reply(
        req_id,
        Err(AppError::RateLimited(THROTTLED_MESSAGE.to_string())),
    )
}

#[cfg(test)]
mod tests;
