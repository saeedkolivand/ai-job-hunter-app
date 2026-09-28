//! `document.export`'s dedicated per-pairing throttle — split from `document_export.rs`
//! (R8 relief); see that module's own doc for why this rides its own bucket rather than
//! sharing `agent_read::AgentQueryThrottle`'s cheap-read one.

// ── Throttle (own instance on `BridgeState`, per pairing — see module doc) ─────────

/// Burst 3, refilling one token every 10s — deliberately much tighter than
/// `agent_read::AgentQueryThrottle`'s cheap-read bucket (burst 10 @ 1/s): an export is a real
/// Typst compile, not an in-memory read.
const DOCUMENT_EXPORT_BURST: f64 = 3.0;
const DOCUMENT_EXPORT_REFILL_SECS: f64 = 10.0;

/// Minimal token bucket — the exact math `match_live::MatchLiveThrottle`/
/// `agent_read`'s private `TokenBucket` use; its own instance rather than sharing either (per-verb
/// cost profiles differ — see the module doc).
pub(super) struct DocumentExportThrottle {
    tokens: f64,
    last: std::time::Instant,
}

impl DocumentExportThrottle {
    pub(super) fn new() -> Self {
        Self {
            tokens: DOCUMENT_EXPORT_BURST,
            last: std::time::Instant::now(),
        }
    }

    fn try_acquire_at(&mut self, now: std::time::Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens =
            (self.tokens + elapsed / DOCUMENT_EXPORT_REFILL_SECS).min(DOCUMENT_EXPORT_BURST);
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

    /// Milliseconds until this bucket would hold one full token — only meaningful called right
    /// after a failed [`Self::try_acquire`] in the same tick (mirrors
    /// `agent_read`'s private `TokenBucket::retry_after_ms`).
    pub(super) fn retry_after_ms(&self) -> u64 {
        if self.tokens >= 1.0 {
            return 0;
        }
        let needed_secs = (1.0 - self.tokens) * DOCUMENT_EXPORT_REFILL_SECS;
        (needed_secs * 1000.0).ceil() as u64
    }
}

#[cfg(test)]
mod tests;
