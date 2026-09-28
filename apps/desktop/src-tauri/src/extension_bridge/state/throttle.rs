//! Per-pairing throttle-acquisition + connection-counting accessors on `BridgeState` — split
//! from `state.rs` (R8 relief; a second `impl BridgeState` block, legal since a private field
//! stays visible to the defining module's descendants, and `state::throttle` is one).

use std::sync::atomic::Ordering;

use super::{now_unix_ms, BridgeState};

impl BridgeState {
    /// Try to consume one `match.live` token from the throttle shared across
    /// every connection for this pairing — see
    /// [`match_live::MatchLiveThrottle`]'s doc for why this lives on
    /// `BridgeState` instead of per-connection (a reconnect must not refresh
    /// the burst).
    pub fn try_acquire_match_live(&self) -> bool {
        self.match_live_limiter.lock().try_acquire()
    }

    /// Try to consume one `agent.query` token for `resource` — `best-matches`
    /// draws from its OWN tighter bucket; every other resource shares the
    /// cheap-read bucket. See [`agent_read::AgentQueryThrottle`]'s doc.
    pub(in crate::extension_bridge) fn try_acquire_agent(&self, resource: &str) -> bool {
        self.agent_query_limiter.lock().try_acquire(resource)
    }

    /// Try to consume one `settings.set` token — per pairing (R7, guard rail #4). See
    /// [`settings::SettingsSetThrottle`]'s doc.
    pub(in crate::extension_bridge) fn try_acquire_settings_set(&self) -> bool {
        self.settings_set_limiter.lock().try_acquire()
    }

    /// Try to consume one `document.export` token — per pairing (PR2). See
    /// [`document_export_throttle::DocumentExportThrottle`]'s doc.
    pub(in crate::extension_bridge) fn try_acquire_document_export(&self) -> bool {
        self.document_export_limiter.lock().try_acquire()
    }

    /// Milliseconds until [`Self::try_acquire_document_export`] would next admit one token — call
    /// this ONLY right after a failed [`Self::try_acquire_document_export`], same discipline as
    /// [`Self::agent_retry_after_ms`].
    pub(in crate::extension_bridge) fn document_export_retry_after_ms(&self) -> u64 {
        self.document_export_limiter.lock().retry_after_ms()
    }

    /// Try to consume one `applied.check.batch` token (PR3). See
    /// [`applied_check_batch::AppliedCheckBatchThrottle`]'s doc.
    pub(in crate::extension_bridge) fn try_acquire_applied_check_batch(&self) -> bool {
        self.applied_check_batch_limiter.lock().try_acquire()
    }

    /// Call ONLY right after a failed [`Self::try_acquire_applied_check_batch`] — same discipline
    /// as [`Self::document_export_retry_after_ms`].
    pub(in crate::extension_bridge) fn applied_check_batch_retry_after_ms(&self) -> u64 {
        self.applied_check_batch_limiter.lock().retry_after_ms()
    }

    /// Milliseconds until [`Self::try_acquire_agent`] would next admit one token for `resource`
    /// (issue #1155) — call this ONLY right after a failed [`Self::try_acquire_agent`] for the
    /// SAME resource, in the SAME dispatch match arm: that failed call already advanced the
    /// correct bucket's clock to "now", so this is a pure read of its current fractional token
    /// count, not a second `Instant::now()` advance. Both `agent.query`'s own throttle refusal
    /// and `agent.call`'s (via `agent_call::throttle_key`) read this SAME bucket, never a
    /// duplicated rate constant.
    pub(in crate::extension_bridge) fn agent_retry_after_ms(&self, resource: &str) -> u64 {
        self.agent_query_limiter.lock().retry_after_ms(resource)
    }

    pub(in crate::extension_bridge) fn set_port(&self, port: Option<u16>) {
        *self.port.lock() = port;
    }

    /// Record a socket reaching `Authenticated`. Returns `true` iff this was
    /// the 0→1 transition (the first paired browser) — the caller uses this to
    /// emit [`crate::events::EXTENSION_BRIDGE_CHANGED`] only on a real
    /// transition, not on every additional pairing.
    pub(in crate::extension_bridge) fn inc_connected(&self) -> bool {
        self.last_authenticated_ms
            .store(now_unix_ms(), Ordering::Relaxed);
        self.connected.fetch_add(1, Ordering::Relaxed) == 0
    }

    /// Record one authenticated socket's teardown. Saturating: a decrement
    /// past zero (should never happen — callers only decrement a connection
    /// that itself incremented, see `handle_connection`'s `authenticated`
    /// flag) stays at zero rather than wrapping `AtomicUsize` to `usize::MAX`,
    /// which `is_connected` (`count > 0`) would otherwise misreport as
    /// connected. Returns `true` iff this was the 1→0 transition (the last
    /// paired browser disconnected).
    pub(in crate::extension_bridge) fn dec_connected(&self) -> bool {
        let prev = self
            .connected
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |c| {
                Some(c.saturating_sub(1))
            })
            .unwrap_or(0);
        prev == 1
    }

    /// The rotation epoch a socket must record (AFTER its [`Self::inc_connected`])
    /// so its teardown can prove the count it wants to give back is still its
    /// own. Read after the increment, never before: a rotation landing between
    /// the two would otherwise leave the socket holding a stale epoch for a
    /// count it added AFTER the reset, and it could never give that count back.
    pub(in crate::extension_bridge) fn rotation_epoch(&self) -> u64 {
        self.rotation_epoch.load(Ordering::Relaxed)
    }

    /// [`Self::dec_connected`], but ONLY while `epoch` is still the current
    /// rotation. A revoked socket that finally tears down after a rotation
    /// (typically one parked in a long dispatch await — an `import.request`
    /// fetch, a `match.live` scrape — that had not polled its revoke receiver
    /// yet) must NOT decrement: [`Self::regenerate_token`] already zeroed the
    /// count on its behalf, and by then a browser may have re-paired on the new
    /// token, so a blind decrement would take THAT live pairing 1→0 and make
    /// `is_connected()` under-report a genuinely connected extension. Returns
    /// `false` (no transition) whenever the epoch has moved on.
    pub(in crate::extension_bridge) fn dec_connected_for_epoch(&self, epoch: u64) -> bool {
        if self.rotation_epoch() != epoch {
            return false;
        }
        self.dec_connected()
    }
}

#[cfg(test)]
mod tests;
