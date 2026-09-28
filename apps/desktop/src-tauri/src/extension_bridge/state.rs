//! `BridgeState` — managed Tauri state for the bridge (R8 relief, split from `mod.rs`).
//! Commands read the bound port + token off this; the server counts `connected` up/down as
//! sockets pair/close. Core state (load/token/opt-ins) + the factory-reset `Resettable` hook
//! live here; the per-pairing throttle-acquisition + connection-counting accessors live in the
//! child `state::throttle` (needs the SAME private-field access, so it stays a descendant, not
//! a sibling).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use parking_lot::Mutex;

use super::persist::{
    load_ai_assist_optin, load_autofill_optin, load_or_create_token, persist_ai_assist_optin,
    persist_autofill_optin,
};
use super::{
    agent_read, applied_check_batch, autotrack, document_export_throttle, match_live,
    save_answers_optin, settings,
};
use crate::observability::sanitize_reason;

/// Managed Tauri state for the bridge. Commands read the bound port + token off
/// this; the server counts `connected` up/down as sockets pair/close.
pub struct BridgeState {
    /// `Some` once a port in [`PORT_RANGE`] bound; `None` if the bridge is
    /// disabled (no free port / startup failure).
    port: Mutex<Option<u16>>,
    /// The pairing secret. Persisted to disk; rotated by `regenerate`.
    token: Mutex<String>,
    /// Live-connection refcount: incremented once a socket's client proof
    /// verifies (the v2 mutual handshake completes — never on the bare WS
    /// handshake nor the `hello`/`challenge` exchange, so an unauthenticated
    /// client is never counted) and decremented when that same socket's loop
    /// exits. Multiple browsers may legitimately share one pairing token
    /// (each gets its own per-socket HMAC handshake), so this is a COUNT, not
    /// a last-writer-wins flag — otherwise whichever socket closed last would
    /// decide `is_connected()` for every other still-open one (e.g. Chrome's
    /// MV3 service worker idling its socket closed would falsely report
    /// "disconnected" while Firefox is still paired). [`Self::is_connected`]
    /// is `count > 0`.
    connected: AtomicUsize,
    /// Unix-ms of the most recent socket to reach `Authenticated`, or 0 when
    /// none ever has.
    ///
    /// The live COUNT above is the wrong thing to show a user on its own: an
    /// MV3 service worker is evicted when idle, so a perfectly healthy pairing
    /// sits at zero sockets almost all the time and the UI read "Not
    /// connected" permanently (#1258). Pairing health is "did an authenticated
    /// socket exist recently", which this records and the count cannot.
    last_authenticated_ms: AtomicU64,
    /// Assisted-autofill opt-in (default OFF, persisted to [`AUTOFILL_OPTIN_FILE`]).
    /// A `profile.get` returns the contact profile only while this is on; off ⇒
    /// the desktop replies with a clear refusal (never silently). This is the
    /// consent gate for sending the user's saved contact details into a page.
    autofill_enabled: AtomicBool,
    /// AI-answer-assist opt-in (default OFF, persisted to
    /// [`AI_ASSIST_OPTIN_FILE`]). A SEPARATE gate from `autofill_enabled` —
    /// `answer.assist` is billable provider spend, a materially different
    /// consent class from the local/free autofill verbs. A bare `AtomicBool`
    /// like `autofill_enabled` now that it no longer carries a provider
    /// snapshot: a draft resolves the active provider from the backend-owned
    /// [`crate::ai_config::AiConfigStore`] at answer-time (task #16) via
    /// [`crate::pipeline::Completer::from_active`], never a renderer snapshot.
    ai_assist_enabled: AtomicBool,
    /// Auto-track opt-in (default OFF, persisted to
    /// `autotrack::AUTOTRACK_OPTIN_FILE`).
    /// Task #22: the extension reads it (via `autotrack.check`) to decide
    /// whether to arm its gesture submit-watcher, and the desktop re-checks it
    /// before honoring an AUTO `status.update` (a write flagged `auto: true`) —
    /// defense-in-depth so a compromised extension can't auto-mark `applied`
    /// without the user's opt-in. A bare `AtomicBool`, same shape as
    /// `autofill_enabled` / `ai_assist_enabled`.
    pub(super) autotrack_enabled: AtomicBool,
    /// `saveAnswersOnSubmit` opt-in (PR4, default OFF) — its OWN consent class, a further
    /// amendment to 0009's Task #22: the trigger is a detected submit event, not a click, and it
    /// writes page-derived answer TEXT rather than flipping one status value. Re-checked before
    /// honoring an AUTO `answers.save` — see `answers_save::auto_save_refused`.
    pub(super) save_answers_on_submit_enabled: AtomicBool,
    /// Serializes compare + persist across the three consent-switch setters,
    /// so two writers racing the same key (a second paired browser, or the
    /// desktop Settings command) can't interleave and disagree memory-vs-disk.
    pub(super) optin_write_lock: Mutex<()>,
    /// `match.live` token-bucket throttle — shared across EVERY connection for
    /// this pairing, not per-connection, so a loopback reconnect (a cheap,
    /// near-instant handshake) can never reset the burst allowance. See
    /// [`match_live::MatchLiveThrottle`]'s doc.
    match_live_limiter: Mutex<match_live::MatchLiveThrottle>,
    /// `agent.query` token-bucket throttle(s) — shared across EVERY
    /// connection for this pairing for the SAME reconnect-proof reason as
    /// `match_live_limiter`; a fresh CLI process/socket per invocation must
    /// not reset the bucket. See [`agent_read::AgentQueryThrottle`]'s doc.
    agent_query_limiter: Mutex<agent_read::AgentQueryThrottle>,
    /// `settings.set` token-bucket throttle (R7, guard rail #4) — per pairing, same
    /// reconnect-proof reasoning as `match_live_limiter`/`agent_query_limiter`. Deliberately its
    /// OWN instance, never shared with `agent_query_limiter`: see [`settings::SettingsSetThrottle`]'s doc.
    settings_set_limiter: Mutex<settings::SettingsSetThrottle>,
    /// `document.export` token-bucket throttle (PR2 — documents into ATS) — per pairing, same
    /// reconnect-proof reasoning as every other throttle here. Its OWN instance, never sharing
    /// `agent_query_limiter`'s cheap-read bucket: an export is a real Typst compile (100-400ms),
    /// not a cheap DB read. See [`document_export_throttle::DocumentExportThrottle`]'s doc.
    document_export_limiter: Mutex<document_export_throttle::DocumentExportThrottle>,
    /// `applied.check.batch` throttle (PR3) — own instance (that verb amplifies N-fold; see [`applied_check_batch::AppliedCheckBatchThrottle`]).
    applied_check_batch_limiter: Mutex<applied_check_batch::AppliedCheckBatchThrottle>,
    /// Fan-out signal telling every LIVE connection task that the pairing
    /// token is being rotated (see [`Self::regenerate_token`]). A broadcast —
    /// not a per-connection registry — because that is exactly the shape the
    /// connection tasks need: each one subscribes once at accept time and races
    /// its receiver alongside `reader.next()` in the read loop (see
    /// [`stream::next_step`]), so a rotation reaches every socket without this
    /// state having to track (and reap) their senders. `send` is synchronous,
    /// which is what lets the sync `Resettable::reset(&self)` hook reach the
    /// async socket tasks at all.
    revoke_tx: tokio::sync::broadcast::Sender<()>,
    /// Monotonic rotation counter, bumped inside the same lock hold that sends
    /// the revoke and zeroes `connected`. It exists to stop a REVOKED socket's
    /// late teardown from decrementing a count that no longer belongs to it:
    /// a socket parked in a long dispatch await (an `import.request` fetch, a
    /// `match.live` scrape) may not poll its revoke receiver until well after a
    /// browser has already re-paired on the NEW token, and a blind
    /// `dec_connected` there would take that live pairing's count 1→0 —
    /// under-reporting a genuinely connected extension until its socket
    /// happens to close. Each connection records the epoch it counted itself
    /// under and only decrements while it still matches
    /// ([`Self::dec_connected_for_epoch`]).
    rotation_epoch: AtomicU64,
    /// App data dir — where the token file lives.
    pub(super) data_dir: PathBuf,
}

/// Wall-clock unix-ms. Used only to stamp the last authenticated socket for
/// the UI's "paired but idle" state (#1258) — never for ordering or security,
/// so a clock adjustment can make it look stale at worst, never unsafe.
fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl BridgeState {
    /// Load (or first-run create + persist) the pairing token, returning a state
    /// with no port yet (the server sets it once bound). The autofill opt-in is
    /// read from disk (default OFF when the flag file is absent).
    pub fn load(data_dir: &Path) -> Self {
        let token = load_or_create_token(data_dir);
        Self {
            port: Mutex::new(None),
            token: Mutex::new(token),
            connected: AtomicUsize::new(0),
            last_authenticated_ms: AtomicU64::new(0),
            autofill_enabled: AtomicBool::new(load_autofill_optin(data_dir)),
            ai_assist_enabled: AtomicBool::new(load_ai_assist_optin(data_dir)),
            autotrack_enabled: AtomicBool::new(autotrack::load_autotrack_optin(data_dir)),
            save_answers_on_submit_enabled: AtomicBool::new(
                save_answers_optin::load_save_answers_on_submit_optin(data_dir),
            ),
            optin_write_lock: Mutex::new(()),
            match_live_limiter: Mutex::new(match_live::MatchLiveThrottle::new()),
            agent_query_limiter: Mutex::new(agent_read::AgentQueryThrottle::new()),
            settings_set_limiter: Mutex::new(settings::SettingsSetThrottle::new()),
            document_export_limiter: Mutex::new(
                document_export_throttle::DocumentExportThrottle::new(),
            ),
            applied_check_batch_limiter: Mutex::new(
                applied_check_batch::AppliedCheckBatchThrottle::new(),
            ),
            // Capacity 1: the signal is a bare "rotate happened" edge, so a
            // receiver that fell behind two back-to-back rotations gets
            // `RecvError::Lagged` — which the read loop treats exactly like the
            // signal itself (it still means "your pairing is gone").
            revoke_tx: tokio::sync::broadcast::channel(1).0,
            rotation_epoch: AtomicU64::new(0),
            data_dir: data_dir.to_path_buf(),
        }
    }

    /// Current bound port, if any.
    pub fn port(&self) -> Option<u16> {
        *self.port.lock()
    }

    /// Whether at least one authenticated extension socket is currently
    /// paired (the live-connection count is non-zero — see `connected`'s doc).
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed) > 0
    }

    /// Unix-ms of the last authenticated socket, or `None` if there has never
    /// been one. Paired with [`Self::is_connected`] so the UI can tell "idle,
    /// worker asleep" (the normal MV3 state) apart from "never paired"
    /// (#1258).
    pub fn last_authenticated_ms(&self) -> Option<u64> {
        match self.last_authenticated_ms.load(Ordering::Relaxed) {
            0 => None,
            ms => Some(ms),
        }
    }

    /// The current pairing token.
    pub fn token(&self) -> String {
        self.token.lock().clone()
    }

    /// Whether assisted autofill is opted in (the `profile.get` consent gate).
    pub fn autofill_enabled(&self) -> bool {
        self.autofill_enabled.load(Ordering::Relaxed)
    }

    /// Set (and persist) the assisted-autofill opt-in; returns `true` iff
    /// changed. Holds `optin_write_lock` across the swap + persist. Always
    /// persists even on a no-op value (a stale on-disk format still needs
    /// normalizing) — only the RETURN VALUE is conditioned on `changed`.
    pub fn set_autofill_enabled(&self, enabled: bool) -> bool {
        let _guard = self.optin_write_lock.lock();
        let prev = self.autofill_enabled.swap(enabled, Ordering::Relaxed);
        if let Err(e) = persist_autofill_optin(&self.data_dir, enabled) {
            let reason = sanitize_reason(&e.to_string());
            log::warn!("[extension_bridge] failed to persist autofill opt-in: {reason}");
        }
        prev != enabled
    }

    /// Whether AI-answer-assist is opted in (the `answer.assist` consent gate
    /// — SEPARATE from `autofill_enabled`). This is the billable-AI consent
    /// boundary (ADR-0011): `answer.assist` is refused unless it is on.
    pub fn ai_assist_enabled(&self) -> bool {
        self.ai_assist_enabled.load(Ordering::Relaxed)
    }

    /// Set (and persist) the AI-answer-assist opt-in; returns `true` iff
    /// changed — same discipline as `set_autofill_enabled`. The opt-in no
    /// longer carries a provider snapshot: a draft resolves the active
    /// provider from the backend-owned [`crate::ai_config::AiConfigStore`]
    /// at answer-time (task #16), so this is a bare boolean gate.
    pub fn set_ai_assist(&self, enabled: bool) -> bool {
        let _guard = self.optin_write_lock.lock();
        let prev = self.ai_assist_enabled.swap(enabled, Ordering::Relaxed);
        if let Err(e) = persist_ai_assist_optin(&self.data_dir, enabled) {
            let reason = sanitize_reason(&e.to_string());
            log::warn!("[extension_bridge] failed to persist ai-assist opt-in: {reason}");
        }
        prev != enabled
    }
}

/// Factory-reset hook: rotate the token so a wiped install re-pairs from scratch
/// — which also REVOKES every live pairing (see [`BridgeState::regenerate_token`]),
/// so a paired browser is told to re-pair instead of surviving the reset on a
/// socket it opened before it — and return both opt-ins to their default OFF
/// (consent must be re-granted).
impl crate::data_store::Resettable for BridgeState {
    fn reset(&self) {
        self.regenerate_token();
        self.set_autofill_enabled(false);
        self.set_ai_assist(false);
        self.set_autotrack_enabled(false);
        self.set_save_answers_on_submit_enabled(false);
    }
}

mod rotation;
mod throttle;

#[cfg(test)]
mod tests;
