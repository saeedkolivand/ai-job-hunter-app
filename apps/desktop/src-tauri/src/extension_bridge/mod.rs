//! Browser-extension ⇄ desktop bridge — a loopback-only WebSocket server.
//!
//! Feature 2: the browser extension's "Save this job" button opens a WS to the
//! desktop app and sends an [`import.request`](shared `extension-protocol.ts`)
//! frame. The desktop scrapes/parses the posting, creates a [`crate::applications`]
//! aggregate from it (Application only — an import is a pursuit, not a discovery,
//! so it does NOT enter the postings cache / Jobs feed), and replies with
//! `import.result`.
//!
//! ## Security model (layered — see [`auth`])
//! 1. **Loopback only** — the listener binds `127.0.0.1`; no LAN/remote reach.
//! 2. **Origin allowlist** (defense-in-depth, not the primary boundary) — the
//!    WS handshake's `Origin` must be an allowed extension origin: Chrome is
//!    pinned by store id (`chrome-extension://<id>` in
//!    [`auth::ALLOWED_EXTENSION_IDS`]); Firefox is accepted by UUID **shape**
//!    (`moz-extension://<uuid>`), since its per-install internal UUID is
//!    unknowable in advance — see [`auth::is_allowed_origin`]. A dev override
//!    (`platform::config::extension_dev_origins`) admits a locally-loaded
//!    extension. The mutual HMAC handshake below (3) is what actually
//!    authenticates.
//! 3. **Mutual HMAC challenge-response (protocol v2)** — the pairing token is
//!    NEVER transmitted. On connect the extension sends [`msg::HELLO`]
//!    `{protocol, clientNonce}`; the desktop replies [`msg::CHALLENGE`]
//!    `{serverNonce}`; the extension sends [`msg::AUTH`] `{proof}` where
//!    `proof = HMAC-SHA256(token, CLIENT_MSG)`; the desktop verifies it
//!    **constant-time** ([`handshake::verify_client_proof`]) and, on success,
//!    replies [`msg::AUTH_OK`] `{serverProof}` (`HMAC-SHA256(token, SERVER_MSG)`)
//!    so the extension can prove the desktop is genuine (not a port-squatter).
//!    `connected` flips true ONLY once the client proof verifies — the WS
//!    handshake and the `hello`/`challenge` exchange alone do NOT authorize.
//!    After auth the socket is session-authenticated: `import.request` /
//!    `profile.get` frames carry NO token (see [`advance_frame`]). A first frame
//!    that is not a valid protocol-2 `hello` (an old extension's legacy
//!    `{type:'auth', token}` frame, a lower protocol) gets [`msg::UPDATE_REQUIRED`]
//!    and the socket closes — a hard cutover with no dual-support path.
//! 4. **Size cap** — frames over [`MAX_FRAME_BYTES`] are rejected.
//! 5. **URL/SSRF guard** — the imported `url` is normalized (http(s) only) and
//!    run through [`auth::is_safe_public_host`] (rejects loopback/private/
//!    link-local/`*.local`) before any fetch.
//!
//! ## Layering
//! This is an **L3 shell** module (like `commands`/`tray`/`updater`): it holds an
//! `AppHandle`, emits Tauri events, and reaches down into L1 (`applications`,
//! `scraping`) — never the reverse. Server startup is fire-and-forget with
//! graceful failure: a bind error logs + disables the bridge but never blocks app
//! boot.

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use futures::StreamExt;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;

use crate::error::{AppError, AppResult};
use crate::events::{emit_event, EXTENSION_BRIDGE_CHANGED};
use crate::observability::sanitize_reason;

use self::persist::{
    load_ai_assist_optin, load_autofill_optin, load_or_create_token, new_token,
    persist_ai_assist_optin, persist_autofill_optin, persist_token,
};

/// The `agent.query` read-only agent/CLI surface (issue #1084 PR 1) — see its
/// module doc.
mod agent_read;
mod answer_assist;
mod answer_rewrite;
mod answers_save;
mod answers_suggest;
mod applied_check;
mod assist_registry;
pub mod auth;
mod autofill_check;
mod autotrack;
pub mod handshake;
mod import_flow;
#[cfg(test)]
mod import_tests;
mod match_live;
/// Wire `type` constants (the TS-mirrored protocol table) — see its module doc.
pub mod msg;
pub mod native_host;
mod persist;
pub mod register;
/// The `token.revoked` wire surface + its no-oracle gate — see its module doc.
mod revoke;
mod status_update;
mod stream;
#[cfg(test)]
mod test;

/// Re-exported so `answer_assist` (a sibling of [`stream`]) can keep
/// referring to it as `super::FrameSink` — see [`stream`]'s module doc for
/// the streaming relay this abstracts over.
pub(crate) use stream::FrameSink;

/// Refusal text for the assisted-autofill opt-in gate — shared verbatim by
/// [`resolve_profile`] (`profile.get`) and
/// [`answers_save::resolve_answers_save`] (`answers.save`), the fill/capture
/// mirror pair riding the SAME consent gate. A single constant (not two
/// copies) so the two can never drift.
pub(crate) const AUTOFILL_OFF_MESSAGE: &str =
    "Autofill is off. Turn it on in AI Job Hunter → Settings → Accounts → Browser extension.";

/// Native-messaging host name — the registered identifier the browser uses to
/// spawn our relay (our exe in `--native-host` mode). MUST match the extension
/// side exactly (`apps/extension`). The host-manifest filename is this with a
/// `.json` suffix.
pub const NATIVE_HOST_NAME: &str = "app.aijobhunter.bridge";

/// On-disk host-manifest filename the browser reads to find + spawn the host.
pub const NATIVE_HOST_MANIFEST: &str = "app.aijobhunter.bridge.json";

/// Handshake protocol version carried in the `hello` frame. MUST match the TS
/// `EXTENSION_PROTOCOL_VERSION` in `packages/shared/.../extension-protocol-constants.ts`.
/// A `hello` with a lower (or absent) protocol is treated as an outdated client.
pub const PROTOCOL_VERSION: u64 = 2;

/// Hard cap on a single WS message. A job page's full `outerHTML` can run to a
/// few MB, so 8 MB matches the scraper's per-response cap
/// ([`crate::scraping::http`]) — a full-page DOM capture isn't silently dropped
/// — while still blocking a memory-exhaustion frame.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// First port tried, then the rest of the inclusive range until one binds.
const PORT_RANGE: std::ops::RangeInclusive<u16> = 47615..=47620;

/// File under the app data dir holding the persisted pairing token.
const TOKEN_FILE: &str = "extension_token";

/// File under the app data dir holding the assisted-autofill opt-in flag
/// (`"1"` = on, anything else / absent = off). Default OFF: the desktop returns
/// the contact profile for a `profile.get` only when this is on.
const AUTOFILL_OPTIN_FILE: &str = "extension_autofill_optin";

/// File under the app data dir holding the AI-answer-assist opt-in, as one
/// small JSON blob (`{"enabled":bool}`). It USED to also carry a
/// provider/model/base_url snapshot; that snapshot is gone (task #16 — a draft
/// resolves the active provider from the backend-owned
/// [`crate::ai_config::AiConfigStore`] at answer-time), so only `enabled` is
/// read/written now. An OLD file that still has the extra fields is read back
/// fine — the extras are ignored. Default OFF, absent/corrupt file → OFF (the
/// safe state).
const AI_ASSIST_OPTIN_FILE: &str = "extension_ai_assist_optin";

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
    autotrack_enabled: AtomicBool,
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
    data_dir: PathBuf,
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
            autofill_enabled: AtomicBool::new(load_autofill_optin(data_dir)),
            ai_assist_enabled: AtomicBool::new(load_ai_assist_optin(data_dir)),
            autotrack_enabled: AtomicBool::new(autotrack::load_autotrack_optin(data_dir)),
            match_live_limiter: Mutex::new(match_live::MatchLiveThrottle::new()),
            agent_query_limiter: Mutex::new(agent_read::AgentQueryThrottle::new()),
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

    /// The current pairing token.
    pub fn token(&self) -> String {
        self.token.lock().clone()
    }

    /// Rotate the pairing token: REVOKE every live pairing, generate a new
    /// secret, persist it, and return it. The single rotation path — Settings →
    /// "Regenerate" ([`crate::commands::extension_bridge::extension_bridge_regenerate_token`])
    /// and the factory-reset hook ([`crate::data_store::Resettable`]) both come
    /// through here, so revocation can't apply to one and not the other.
    ///
    /// Order, all four steps under ONE hold of the token lock:
    /// 1. signal [`Self::subscribe_revoke`]'s receivers — each live connection
    ///    task sends `token.revoked` on its socket **if that socket is
    ///    authenticated** and then closes it (see `handle_connection`);
    /// 2. zero the live-connection count — no pairing survives a rotation, so
    ///    [`Self::is_connected`] must read false immediately rather than
    ///    waiting on every socket's teardown;
    /// 3. bump [`Self::rotation_epoch`], which is what stops a revoked socket's
    ///    LATE teardown from decrementing a count that a newly re-paired
    ///    browser now owns (see [`Self::dec_connected_for_epoch`]);
    /// 4. rotate + persist the secret.
    ///
    /// **TWO invariants are load-bearing here — a refactor must preserve BOTH:**
    /// - **The single lock hold around steps 1–4.** `revoke_tx.send()` must not
    ///   move outside it. The lock is what orders the revoke against the swap:
    ///   any reader that observes the NEW token is guaranteed to observe a
    ///   revoke that was ALREADY broadcast, so no socket can read a fresh token
    ///   and still be missed by the signal.
    /// - **Subscribe-at-accept** (in `handle_connection`), because the proof is
    ///   verified OUTSIDE this lock: a socket can still authenticate on a stale
    ///   token clone read just before step 1. Because its receiver already
    ///   existed, `broadcast::Sender::send` buffered the signal for it, so its
    ///   very next read-loop iteration resolves `Revoked` and tears it down.
    ///
    /// Drop either one and that window reopens. Any socket that re-runs the v2
    /// handshake afterwards with the old token fails the client-proof check and
    /// must re-pair with the new value.
    pub fn regenerate_token(&self) -> String {
        let fresh = new_token();
        {
            // One hold, four steps — see the doc above for why BOTH this lock
            // scope and the subscribe-at-accept in `handle_connection` are
            // load-bearing. The lock alone does NOT exclude a concurrent
            // handshake (`advance_auth` verifies outside it, via a `token()`
            // clone); it orders the revoke ahead of the swap. The buffered
            // broadcast is what catches a socket that authenticated on the
            // stale clone, and `dec_connected_for_epoch` is what stops that
            // socket's late teardown from stealing a newer pairing's count.
            let mut token = self.token.lock();
            let _ = self.revoke_tx.send(());
            self.connected.store(0, Ordering::Relaxed);
            self.rotation_epoch.fetch_add(1, Ordering::Relaxed);
            *token = fresh.clone();
        }
        if let Err(e) = persist_token(&self.data_dir, &fresh) {
            let reason = sanitize_reason(&e.to_string());
            log::warn!("[extension_bridge] failed to persist regenerated token: {reason}");
        }
        fresh
    }

    /// Subscribe to the pairing-revocation signal — one receiver per accepted
    /// connection, taken at accept time (BEFORE the handshake) so a rotation
    /// that races the handshake can never slip past a socket. Only sockets that
    /// exist at rotation time are signalled: a broadcast is an edge, never
    /// replayed state, so a connection opened after the rotation is not told
    /// anything (it is already handshaking against the new token).
    pub(super) fn subscribe_revoke(&self) -> tokio::sync::broadcast::Receiver<()> {
        self.revoke_tx.subscribe()
    }

    /// Whether assisted autofill is opted in (the `profile.get` consent gate).
    pub fn autofill_enabled(&self) -> bool {
        self.autofill_enabled.load(Ordering::Relaxed)
    }

    /// Set (and persist) the assisted-autofill opt-in. A persist failure is
    /// non-fatal but leaves the in-memory value authoritative for this run.
    pub fn set_autofill_enabled(&self, enabled: bool) {
        self.autofill_enabled.store(enabled, Ordering::Relaxed);
        if let Err(e) = persist_autofill_optin(&self.data_dir, enabled) {
            let reason = sanitize_reason(&e.to_string());
            log::warn!("[extension_bridge] failed to persist autofill opt-in: {reason}");
        }
    }

    /// Whether AI-answer-assist is opted in (the `answer.assist` consent gate
    /// — SEPARATE from `autofill_enabled`). This is the billable-AI consent
    /// boundary (ADR-0011): `answer.assist` is refused unless it is on.
    pub fn ai_assist_enabled(&self) -> bool {
        self.ai_assist_enabled.load(Ordering::Relaxed)
    }

    /// Set (and persist) the AI-answer-assist opt-in. A persist failure is
    /// non-fatal but leaves the in-memory value authoritative for this run.
    /// The opt-in no longer carries a provider snapshot: a draft resolves the
    /// active provider from the backend-owned
    /// [`crate::ai_config::AiConfigStore`] at answer-time (task #16), so this
    /// is a bare boolean gate — the exact shape of `set_autofill_enabled`.
    pub fn set_ai_assist(&self, enabled: bool) {
        self.ai_assist_enabled.store(enabled, Ordering::Relaxed);
        if let Err(e) = persist_ai_assist_optin(&self.data_dir, enabled) {
            let reason = sanitize_reason(&e.to_string());
            log::warn!("[extension_bridge] failed to persist ai-assist opt-in: {reason}");
        }
    }

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
    pub(super) fn try_acquire_agent(&self, resource: &str) -> bool {
        self.agent_query_limiter.lock().try_acquire(resource)
    }

    fn set_port(&self, port: Option<u16>) {
        *self.port.lock() = port;
    }

    /// Record a socket reaching `Authenticated`. Returns `true` iff this was
    /// the 0→1 transition (the first paired browser) — the caller uses this to
    /// emit [`crate::events::EXTENSION_BRIDGE_CHANGED`] only on a real
    /// transition, not on every additional pairing.
    fn inc_connected(&self) -> bool {
        self.connected.fetch_add(1, Ordering::Relaxed) == 0
    }

    /// Record one authenticated socket's teardown. Saturating: a decrement
    /// past zero (should never happen — callers only decrement a connection
    /// that itself incremented, see `handle_connection`'s `authenticated`
    /// flag) stays at zero rather than wrapping `AtomicUsize` to `usize::MAX`,
    /// which `is_connected` (`count > 0`) would otherwise misreport as
    /// connected. Returns `true` iff this was the 1→0 transition (the last
    /// paired browser disconnected).
    fn dec_connected(&self) -> bool {
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
    pub(super) fn rotation_epoch(&self) -> u64 {
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
    pub(super) fn dec_connected_for_epoch(&self, epoch: u64) -> bool {
        if self.rotation_epoch() != epoch {
            return false;
        }
        self.dec_connected()
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
    }
}

/// Spawn the bridge server via the Tauri async runtime. Fire-and-forget: a
/// bind failure logs and leaves the bridge disabled (port stays `None`) — it
/// never panics or blocks boot. Call once from the Tauri `setup`, which runs on
/// the main thread with **no** ambient Tokio reactor in scope — so this routes
/// through [`spawn_detached`] ([`tauri::async_runtime::spawn`]), the house idiom
/// for spawning from a sync/no-runtime context (a bare `tokio::spawn` here
/// panics with "there is no reactor running").
pub fn start(app: AppHandle) {
    spawn_detached(async move {
        let Some(state) = app.try_state::<BridgeState>() else {
            log::warn!("[extension_bridge] BridgeState not managed — bridge disabled");
            return;
        };

        let listener = match bind_listener().await {
            Some((listener, port)) => {
                state.set_port(Some(port));
                log::info!("[extension_bridge] listening on 127.0.0.1:{port}");
                listener
            }
            None => {
                log::warn!(
                    "[extension_bridge] no free port in {}..={} — bridge disabled",
                    PORT_RANGE.start(),
                    PORT_RANGE.end()
                );
                return;
            }
        };

        loop {
            match listener.accept().await {
                Ok((stream, _peer)) => {
                    let conn_app = app.clone();
                    spawn_detached(async move {
                        handle_connection(conn_app, stream).await;
                    });
                }
                Err(e) => {
                    log::warn!("[extension_bridge] accept error (continuing): {e}");
                }
            }
        }
    });
}

/// Fire-and-forget spawn through the Tauri async runtime. Unlike a bare
/// `tokio::spawn`, this does **not** require an ambient Tokio reactor in the
/// caller's scope, so it is safe to call from the sync `setup` hook (which runs
/// on the main thread with no runtime). This is the house idiom shared with
/// `updater`/`tray`/`autopilot_scheduler`. Isolated as a one-line helper so the
/// no-runtime spawn path is exercisable from a plain `#[test]` (no ambient
/// runtime) — a regression to bare `tokio::spawn` would panic that test.
fn spawn_detached<F>(fut: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    tauri::async_runtime::spawn(fut);
}

/// Try each port in [`PORT_RANGE`] in order; return the first that binds.
async fn bind_listener() -> Option<(TcpListener, u16)> {
    probe_ports(PORT_RANGE).await
}

/// Probe an explicit, ordered port range on loopback and return the first that
/// binds (or `None` if all are busy). Factored out of [`bind_listener`] so the
/// fallback/graceful-disable logic is testable against a caller-controlled range
/// of known-busy/known-free ports instead of the fixed [`PORT_RANGE`] (whose
/// availability on CI is non-deterministic). Behaviorally identical to the prior
/// inline loop — same order, same first-binds-wins, same `None` when exhausted.
async fn probe_ports(range: std::ops::RangeInclusive<u16>) -> Option<(TcpListener, u16)> {
    for port in range {
        let addr = (Ipv4Addr::LOCALHOST, port);
        if let Ok(listener) = TcpListener::bind(addr).await {
            return Some((listener, port));
        }
    }
    None
}

/// Perform the WS handshake (validating `Origin`), then drive the v2 mutual
/// HMAC challenge-response before servicing any application frame:
/// `hello{clientNonce}` → `challenge{serverNonce}` →
/// `auth{proof}` — verified **constant-time** via
/// [`handshake::verify_client_proof`] — → `auth.ok{serverProof}`. The pairing
/// token is never transmitted; both sides only prove they know it. `connected`
/// flips true ONLY once the client proof verifies (see [`ConnState`]); the bare
/// WS handshake and the `hello`/`challenge` exchange never mark it connected. A
/// FAILED proof closes the socket with **NO reply** — by design, so a wrong
/// token and a genuine app crash are indistinguishable on the wire (no oracle
/// for a peer probing for a valid token). Once authenticated, subsequent frames
/// (`import.request`, `profile.get`, …) are session-authorized — no per-frame
/// secret. A first frame that isn't a valid protocol-2 `hello` (a legacy
/// plaintext-token `auth` frame, a lower protocol) gets `update.required` and
/// closes — the v1→v2 force cutover, no dual-support path.
///
/// A multi-second streaming `answer.assist` handler runs on its OWN spawned
/// task, never awaited inline here — see [`stream`]'s module doc. Every
/// outbound frame (handshake replies, synchronous verb replies, and a
/// streaming task's own `assist.chunk`/`assist.done`/terminal reply) funnels
/// through one channel into [`stream::run_writer`], the sole writer of the
/// live WS sink; this loop only ever enqueues (never awaits a socket write),
/// so it keeps polling `reader.next()` — including a same-connection
/// `assist.cancel` — while a stream is in flight.
///
/// The read loop also observes [`stream::run_writer`]'s own `JoinHandle` (via
/// [`stream::next_step`]), not just `reader.next()` — [`stream::run_writer`] is
/// spawned DETACHED, so if nothing else raced it, this loop would never learn
/// it ended (a `WRITE_STALL` timeout, or a genuine send error) until its OWN
/// next inbound frame — which, for a stalled-but-open peer or a quiet/idle
/// connection, may never arrive, leaving `cancel_all`/`dec_connected`
/// below delayed indefinitely while the app still believes the extension is
/// connected. Racing the writer handle alongside `reader.next()` means a
/// writer-task end tears the connection down immediately instead.
///
/// The same race carries a third arm: this connection's
/// [`BridgeState::subscribe_revoke`] receiver. A token rotation must reach even
/// a QUIET socket at once — an authenticated one is sent [`msg::TOKEN_REVOKED`]
/// (an unauthenticated one is closed silently, no oracle) and both tear down.
async fn handle_connection(app: AppHandle, stream: TcpStream) {
    use tokio_tungstenite::tungstenite::handshake::server::ErrorResponse;
    use tokio_tungstenite::tungstenite::http::StatusCode;

    let dev_origins = crate::platform::config::extension_dev_origins();
    // Origin allowlist enforced IN the handshake: a disallowed `Origin` is
    // refused with 403 before the socket upgrades, so a non-extension page never
    // reaches the frame loop. The closure's `Result<_, ErrorResponse>` is the
    // signature tungstenite's `Callback` trait mandates; `ErrorResponse`
    // (http::Response<Option<String>>) is inherently large, so the
    // `result_large_err` lint is unavoidable here — scoped-allow with reason.
    #[allow(clippy::result_large_err)] // API-imposed Callback signature (tungstenite)
    let callback = move |req: &Request, res: Response| {
        let origin = req
            .headers()
            .get("origin")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if auth::is_allowed_origin(origin, &dev_origins) {
            Ok(res)
        } else {
            auth::warn_rejected_origin_once(origin);
            let resp = ErrorResponse::new(Some("forbidden origin".to_string()));
            let (mut parts, body) = resp.into_parts();
            parts.status = StatusCode::FORBIDDEN;
            Err(ErrorResponse::from_parts(parts, body))
        }
    };

    // Cap both message + frame size at the handshake so an oversized frame is
    // rejected by the protocol layer before we ever buffer it. `WebSocketConfig`
    // is `#[non_exhaustive]`; its setters are consuming builders.
    let ws_config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));

    let ws =
        match tokio_tungstenite::accept_hdr_async_with_config(stream, callback, Some(ws_config))
            .await
        {
            Ok(ws) => ws,
            Err(e) => {
                auth::log_handshake_failure(&e);
                return;
            }
        };

    let state = match app.try_state::<BridgeState>() {
        Some(s) => s,
        None => return,
    };
    // NOT counted connected yet: the bare WS handshake (loopback + origin) is
    // not authentication. The socket walks the v2 mutual handshake below; the
    // live-connection count only increments once the extension's client proof
    // verifies (an `AuthOk` decision), so an unauthenticated socket is never
    // counted. Tracked per-connection so teardown below only decrements a
    // socket that actually incremented (never on an unauthenticated close).
    let mut authenticated = false;
    // Which rotation the `connected` count this socket adds belongs to — set
    // alongside `authenticated` at `AuthOk`. Only meaningful while
    // `authenticated`; see `BridgeState::dec_connected_for_epoch`.
    let mut counted_epoch = 0u64;
    // Subscribed HERE — before the handshake, not at `AuthOk`. This is the
    // load-bearing half of the rotation race (see
    // `BridgeState::regenerate_token`): the proof is verified OUTSIDE the token
    // lock, so a socket can still authenticate on a stale token clone; because
    // its receiver already existed when the rotation broadcast, the signal is
    // buffered and its next read-loop iteration tears it down anyway. Moving
    // this subscription later would reopen that window.
    //
    // Signalling an unauthenticated socket is safe: the read loop closes it
    // WITHOUT the `token.revoked` frame — the same silent close a failed proof
    // gets, so no frame ever confirms a token guess.
    let mut revoked_rx = state.subscribe_revoke();

    let (writer, mut reader) = ws.split();
    // The ONE task that ever writes to the live WS sink — see `stream`'s
    // module doc. Every reply below is enqueued (a synchronous, non-blocking
    // channel `send`), never awaited directly against the socket, so a
    // spawned streaming task never blocks this loop from polling
    // `reader.next()` again.
    let (out_tx, out_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    // Kept (not fire-and-forget-dropped) so the read loop below can race it
    // via [`next_step`] — see `handle_connection`'s own doc for why.
    let mut writer_task = tokio::spawn(stream::run_writer(writer, out_rx));

    // Per-connection handshake state; every socket starts by expecting `hello`.
    let mut conn = ConnState::AwaitingHello;
    // The `match.live` throttle lives on `BridgeState` (shared across every
    // connection for this pairing, reconnect-proof) — consulted below via
    // `state.try_acquire_match_live()`, not a per-connection instance.
    // In-flight streaming `answer.assist` jobs for THIS connection only — see
    // `stream::AssistStreamRegistry`'s doc for why this is per-connection
    // rather than a field on the global `BridgeState`.
    let assist_streams = std::sync::Arc::new(stream::AssistStreamRegistry::default());

    loop {
        let frame =
            match stream::next_step(reader.next(), &mut writer_task, revoked_rx.recv()).await {
                stream::NextStep::Frame(frame) => frame,
                stream::NextStep::Revoked => {
                    // The pairing token was rotated out from under this socket
                    // (Settings → "Regenerate", or a factory reset). WHICH frames
                    // go out — and whether any go out at all — is decided by the
                    // pure [`revoke_frames`], which is where the no-oracle rule is
                    // pinned by tests: an unauthenticated socket gets NOTHING and
                    // just closes, because telling it its pairing was revoked
                    // would confirm the token it was proving against had been the
                    // real one (ADR-0010).
                    if authenticated {
                        log::info!(
                            "[extension_bridge] pairing token rotated — revoking an authenticated \
                         session and closing it"
                        );
                    }
                    // Cancel BEFORE enqueueing the close: a streaming task holds
                    // its own `out_tx` clone, so a still-running generation would
                    // otherwise keep pushing `assist.chunk`s AFTER the `Close` we
                    // just queued (frames behind a close frame), and keep burning
                    // billable provider spend for a session that is already gone.
                    // The post-loop `cancel_all` stays as the catch-all for every
                    // other exit path; calling it twice is a no-op.
                    assist_streams.cancel_all(&app);
                    // Enqueued, not awaited: `run_writer` outlives this loop (it
                    // holds the sink until its channel drains), so the revoke
                    // frame reaches the peer before the close does.
                    for frame in revoke::revoke_frames(authenticated) {
                        let _ = out_tx.send(frame);
                    }
                    break;
                }
                stream::NextStep::RevokeWatchLost => {
                    // The revocation channel closed (shutdown, or a refactor that
                    // dropped the sender). Tear down like any other transport end
                    // — deliberately WITHOUT a `token.revoked`, so a channel
                    // lifecycle change can never unpair every browser at once.
                    log::warn!(
                        "[extension_bridge] revocation channel closed — closing this connection \
                         without sending a revoke"
                    );
                    break;
                }
                stream::NextStep::WriterEnded => {
                    // See `next_step`'s doc + `handle_connection`'s own doc: the
                    // writer task ending (a `WRITE_STALL` timeout or a send error)
                    // must tear this connection down immediately, not wait for
                    // this loop's own next inbound frame — which, for a
                    // stalled-but-open or quiet/idle connection, may never come.
                    // Falls through to the SAME cancel_all + dec_connected
                    // cleanup below as every other exit path.
                    log::warn!(
                        "[extension_bridge] writer task ended (write-stall timeout or a \
                     send error) — tearing down the connection"
                    );
                    break;
                }
            };
        let Some(frame) = frame else {
            break;
        };
        let msg = match frame {
            Ok(m) => m,
            Err(e) => {
                let reason = sanitize_reason(&e.to_string());
                log::warn!("[extension_bridge] read error: {reason}");
                break;
            }
        };
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Binary(b) => match String::from_utf8(b.to_vec()) {
                Ok(s) => s,
                Err(_) => continue,
            },
            Message::Close(_) => break,
            // Ping/Pong are handled by tungstenite; ignore other control frames.
            _ => continue,
        };

        // Advance the handshake state machine (pure — no app state). An over-cap
        // frame closes; an outdated first frame gets `update_required` then close;
        // a failed proof closes without marking connected; only an authenticated
        // import/profile frame reaches `app` state.
        let reply = match advance_frame(&state, &conn, &text) {
            FrameDecision::CloseOverCap => {
                log::warn!("[extension_bridge] frame over size cap — closing");
                break;
            }
            FrameDecision::Drop => None,
            FrameDecision::Outdated(reply) => {
                // Force cutover: the first frame was not a valid protocol-2 hello
                // (a legacy token `auth`, a missing/older protocol). Tell the
                // client to update, then close — no dual-support path.
                log::warn!(
                    "[extension_bridge] rejected outdated/legacy first frame — \
                     sending update_required and closing"
                );
                let _ = out_tx.send(Message::text(reply));
                break;
            }
            FrameDecision::Unauthorized => {
                // A handshake step failed (bad/absent proof, or an unexpected
                // frame mid-handshake). Close WITHOUT a reply and without ever
                // marking the socket connected.
                log::warn!("[extension_bridge] handshake auth failed — closing");
                break;
            }
            FrameDecision::Challenge { reply, next } => {
                // hello accepted → advance to AwaitingAuth (still NOT connected).
                conn = next;
                Some(reply)
            }
            FrameDecision::AuthOk(reply) => {
                // Client proof verified — the mutual handshake completes. Only now
                // is the socket authorized; count it connected and reply auth.ok.
                conn = ConnState::Authenticated;
                authenticated = true;
                if state.inc_connected() {
                    // 0→1: the first paired browser — notify the renderer so the
                    // Settings pill flips immediately instead of waiting on its
                    // 30s poll.
                    emit_event(&app, EXTENSION_BRIDGE_CHANGED, json!({ "connected": true }));
                }
                // Read AFTER the increment (see `rotation_epoch`'s doc): this
                // stamps WHICH rotation the count we just added belongs to, so
                // the teardown below can only give it back while it is still
                // ours.
                counted_epoch = state.rotation_epoch();
                Some(reply)
            }
            FrameDecision::Reply(text) => Some(text),
            FrameDecision::Import { req_id, payload } => {
                let outcome = import_flow::handle_import(&app, payload).await;
                Some(import_flow::result_reply(&req_id, outcome))
            }
            FrameDecision::Profile { req_id } => Some(handle_profile(&app, &req_id)),
            FrameDecision::AppliedCheck { req_id, payload } => {
                Some(applied_check::handle_applied_check(&app, &req_id, &payload))
            }
            FrameDecision::StatusUpdate { req_id, payload } => {
                Some(status_update::handle_status_update(&app, &req_id, &payload))
            }
            FrameDecision::AutotrackCheck { req_id } => Some(autotrack::autotrack_result_reply(
                &req_id,
                state.autotrack_enabled(),
            )),
            FrameDecision::AutofillCheck { req_id } => Some(
                autofill_check::autofill_check_result_reply(&req_id, state.autofill_enabled()),
            ),
            FrameDecision::AnswersSave { req_id, payload } => {
                Some(answers_save::handle_answers_save(&app, &req_id, &payload))
            }
            FrameDecision::AnswersSuggest { req_id, payload } => Some(
                answers_suggest::handle_answers_suggest(&app, &req_id, &payload),
            ),
            FrameDecision::MatchLive { req_id, payload } if state.try_acquire_match_live() => {
                Some(match_live::handle_match_live(&app, &req_id, &payload).await)
            }
            FrameDecision::MatchLive { req_id, .. } => Some(match_live::throttled_reply(&req_id)),
            FrameDecision::AgentQuery { req_id, payload }
                if state.try_acquire_agent(agent_read::resource_name(&payload)) =>
            {
                Some(agent_read::handle_agent_query(&app, &req_id, &payload).await)
            }
            FrameDecision::AgentQuery { req_id, payload } => Some(agent_read::throttled_reply(
                &req_id,
                agent_read::resource_name(&payload),
            )),
            FrameDecision::AnswerAssist { req_id, payload } => {
                // Spawned onto its OWN task (see `stream::spawn_answer_assist`)
                // so a multi-second stream never blocks THIS loop's
                // `reader.next()` — the HIGH fix: an `assist.cancel` for this
                // very stream (or any other frame) must still be read while it
                // is in flight. No reply here for the normal path — the
                // spawned task sends its own `assist.chunk`/`assist.done`/
                // terminal reply through `out_tx`. A duplicate `reqId` is
                // rejected SYNCHRONOUSLY inside `spawn_answer_assist` (before
                // it ever spawns) with its own `answer.assist.result` error
                // reply, also via `out_tx` — see that function's doc.
                stream::spawn_answer_assist(
                    app.clone(),
                    req_id,
                    payload,
                    out_tx.clone(),
                    std::sync::Arc::clone(&assist_streams),
                );
                None
            }
            FrameDecision::AssistCancel { req_id } => {
                assist_streams.cancel(&app, &req_id);
                None
            }
        };
        if let Some(reply) = reply {
            if out_tx.send(Message::text(reply)).is_err() {
                break;
            }
        }
    }

    // The socket is gone (closed, errored, or an over-cap/outdated/failed
    // handshake broke the loop) — cancel every stream still registered for
    // THIS connection, not just the one an explicit `assist.cancel` might
    // have named. Otherwise a client disconnect mid-`answer.assist` leaves
    // the billable generation running to completion with no consumer ever
    // reading it. Per-connection by construction (`assist_streams` is this
    // socket's own registry — see `stream`'s module doc for why that's
    // never a global `BridgeState` field).
    assist_streams.cancel_all(&app);
    // Only a socket that actually reached `Authenticated` (and so incremented
    // the count above) decrements it here — an unauthenticated socket's
    // teardown (a rejected origin, a failed proof, an over-cap/outdated first
    // frame) must never touch the count. After a REVOKE the count was already
    // zeroed by `regenerate_token`, and the epoch moved on — so this decrement
    // is SKIPPED entirely rather than merely saturating: by the time a socket
    // parked in a long dispatch await gets here, a browser may already have
    // re-paired on the new token, and giving back a count we no longer own
    // would take that live pairing 1→0. The rotation path owns the
    // notification instead (the Settings mutation refetches the status; a
    // factory reset emits the event itself).
    if authenticated && state.dec_connected_for_epoch(counted_epoch) {
        // 1→0: the last paired browser disconnected — with two browsers
        // sharing one token, this now only fires once the SECOND socket also
        // closes, not on whichever one happens to close first.
        emit_event(
            &app,
            EXTENSION_BRIDGE_CHANGED,
            json!({ "connected": false }),
        );
    }
}

/// Per-connection handshake state. A socket starts `AwaitingHello`; a valid
/// protocol-2 `hello` moves it to `AwaitingAuth` (holding the two fresh nonces);
/// a **verified** client proof moves it to `Authenticated`. Only in
/// `Authenticated` are `import.request` / `profile.get` frames honored — the
/// socket is session-authenticated, so those frames carry no token.
#[cfg_attr(test, derive(Debug, Clone, PartialEq, Eq))]
enum ConnState {
    /// Fresh socket — the next frame must be a protocol-2 `hello`.
    AwaitingHello,
    /// `hello` accepted; a `challenge` was sent. The next frame must be
    /// `auth { proof }`; these nonces bind the expected proof.
    AwaitingAuth {
        server_nonce: String,
        client_nonce: String,
    },
    /// Mutual handshake complete — subsequent frames are session-authorized.
    Authenticated,
}

/// Outcome of the per-frame handshake/dispatch decision, isolated from any
/// `AppHandle` so the size gate + handshake state machine are unit-testable. The
/// connection loop runs the (async, app-stateful) import only for
/// [`FrameDecision::Import`]; every other variant is resolved here from pure
/// inputs (+ the token off [`BridgeState`] for the constant-time proof check).
#[cfg_attr(test, derive(Debug))]
enum FrameDecision {
    /// Frame exceeds [`MAX_FRAME_BYTES`] — close the socket without parsing.
    CloseOverCap,
    /// Not JSON, or an ignorable frame — drop silently, no reply, stay in state.
    Drop,
    /// The first frame was not a valid protocol-2 `hello` (a legacy `{type:'auth',
    /// token}` frame, a missing/older protocol): send this ready-to-send
    /// [`msg::UPDATE_REQUIRED`] reply, then CLOSE. Force cutover — no dual path.
    Outdated(String),
    /// A handshake step failed (bad/absent proof, or an unexpected frame
    /// mid-handshake): CLOSE without a reply and without marking connected.
    /// Distinct from [`FrameDecision::AuthOk`] so the loop never authorizes a
    /// socket whose proof did not verify.
    Unauthorized,
    /// `hello` accepted: send this `challenge` reply and advance to `next`
    /// (`AwaitingAuth`). NOT yet connected.
    Challenge { reply: String, next: ConnState },
    /// The client proof VERIFIED (constant-time): send this `auth.ok` reply, mark
    /// the socket connected, and advance to `Authenticated`.
    AuthOk(String),
    /// A ready-to-send reply from an authenticated frame (an unknown message
    /// type acknowledged as an error). Stays `Authenticated`.
    Reply(String),
    /// An authenticated `import.request` to dispatch through
    /// [`import_flow::handle_import`].
    Import { req_id: String, payload: Value },
    /// An authenticated `profile.get` to answer through [`handle_profile`]. Carries
    /// no payload — the reply is gated on the autofill opt-in, not on any input.
    Profile { req_id: String },
    /// An authenticated `applied.check` to answer through
    /// [`applied_check::handle_applied_check`]. Carries the payload verbatim so
    /// the handler can read `url`. Read-only by construction: resolved from the
    /// local `ApplicationStore` only — never the network.
    AppliedCheck { req_id: String, payload: Value },
    /// An authenticated `status.update` to answer through
    /// [`status_update::handle_status_update`]. Carries the payload verbatim so
    /// the handler can read `url` + `to`. The ONLY write this dispatch can
    /// route to besides `Import`;
    /// [`status_update::resolve_status_update`] is what actually restricts it
    /// to `saved → applied` on an exact match.
    StatusUpdate { req_id: String, payload: Value },
    /// An authenticated `autotrack.check` (Task #22) — a pure read of the
    /// auto-track opt-in off [`BridgeState`]. No payload; the loop answers it
    /// with `autotrack::autotrack_result_reply`.
    AutotrackCheck { req_id: String },
    /// An authenticated `autofill.check` (Task #30) — a pure read of the
    /// assisted-autofill opt-in off [`BridgeState`]. Mirrors
    /// [`FrameDecision::AutotrackCheck`] exactly. No payload; the loop
    /// answers it with `autofill_check::autofill_check_result_reply`.
    AutofillCheck { req_id: String },
    /// An authenticated `answers.save` to answer through
    /// [`answers_save::handle_answers_save`]. Carries the payload verbatim so
    /// the handler can read `url` + `answers`.
    AnswersSave { req_id: String, payload: Value },
    /// An authenticated `answers.suggest` to answer through
    /// [`answers_suggest::handle_answers_suggest`]. Carries the payload
    /// verbatim so the handler can read `questions`.
    AnswersSuggest { req_id: String, payload: Value },
    /// An authenticated `match.live` to answer through
    /// [`match_live::handle_match_live`]. Carries the payload verbatim so the
    /// handler can read `url` + `html`.
    MatchLive { req_id: String, payload: Value },
    /// An authenticated `answer.assist` to answer through
    /// [`answer_assist::handle_answer_assist`]. Carries the payload verbatim
    /// so the handler can read `question` + `url` + `searchWeb`.
    AnswerAssist { req_id: String, payload: Value },
    /// An authenticated `assist.cancel` — cancel the in-flight stream named
    /// by `req_id` on THIS connection's own
    /// [`stream::AssistStreamRegistry`]. No reply is ever sent for this frame.
    AssistCancel { req_id: String },
    /// An authenticated `agent.query` (issue #1084 PR 1) to answer through
    /// [`agent_read::handle_agent_query`]. Carries the payload verbatim so
    /// the handler can read `resource` (+ `url`/`limit`).
    AgentQuery { req_id: String, payload: Value },
}

/// The per-message handshake gate + dispatch routing (size cap → JSON parse →
/// state-machine step) — everything that does NOT need an `AppHandle`. Pure
/// aside from reading the pairing token off [`BridgeState`] for the
/// constant-time proof check; the loop performs the I/O and the app-stateful
/// import/profile work. See [`ConnState`] for the state transitions.
fn advance_frame(state: &BridgeState, conn: &ConnState, text: &str) -> FrameDecision {
    if text.len() > MAX_FRAME_BYTES {
        return FrameDecision::CloseOverCap;
    }

    let envelope: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return FrameDecision::Drop, // not JSON — drop silently
    };

    let kind = envelope.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let req_id = envelope
        .get("reqId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let payload = envelope.get("payload");

    match conn {
        ConnState::AwaitingHello => advance_hello(kind, &req_id, payload),
        ConnState::AwaitingAuth {
            server_nonce,
            client_nonce,
        } => advance_auth(state, kind, &req_id, payload, server_nonce, client_nonce),
        ConnState::Authenticated => advance_authenticated(kind, req_id, &envelope),
    }
}

/// Handshake step 1: the FIRST frame must be a valid protocol-2 `hello`. A legacy
/// `{type:'auth', token}` frame, a missing/older `protocol`, or a malformed
/// `clientNonce` are all treated as an outdated client → `update_required` + close.
fn advance_hello(kind: &str, req_id: &str, payload: Option<&Value>) -> FrameDecision {
    if kind != msg::HELLO {
        return FrameDecision::Outdated(update_required_reply(req_id));
    }
    let protocol = payload
        .and_then(|p| p.get("protocol"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let client_nonce = payload
        .and_then(|p| p.get("clientNonce"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if protocol < PROTOCOL_VERSION || !handshake::is_valid_nonce(client_nonce) {
        return FrameDecision::Outdated(update_required_reply(req_id));
    }
    // Fresh server nonce (CSPRNG, per connection). Bind it + the client nonce into
    // the next state so the proof is verified against exactly this pair.
    let server_nonce = handshake::new_nonce();
    let reply = challenge_reply(req_id, &server_nonce);
    FrameDecision::Challenge {
        reply,
        next: ConnState::AwaitingAuth {
            server_nonce,
            client_nonce: client_nonce.to_string(),
        },
    }
}

/// Handshake step 3: only an `auth { proof }` is valid here. The proof is verified
/// CONSTANT-TIME against `HMAC-SHA256(token, CLIENT_MSG)`; on success the desktop
/// proves ITSELF via `serverProof` (step 4). Any other frame, or a bad/absent
/// proof, closes the socket (never connected).
fn advance_auth(
    state: &BridgeState,
    kind: &str,
    req_id: &str,
    payload: Option<&Value>,
    server_nonce: &str,
    client_nonce: &str,
) -> FrameDecision {
    if kind != msg::AUTH {
        return FrameDecision::Unauthorized;
    }
    let proof = payload
        .and_then(|p| p.get("proof"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let token = state.token();
    if !handshake::verify_client_proof(&token, server_nonce, client_nonce, proof) {
        log::warn!("[extension_bridge] handshake: client proof failed constant-time verification");
        return FrameDecision::Unauthorized;
    }
    let server_proof = handshake::server_proof(&token, server_nonce, client_nonce);
    FrameDecision::AuthOk(auth_ok_reply(req_id, &server_proof))
}

/// Post-auth dispatch: the socket is session-authenticated, so frames carry no
/// token. Routes `import.request` / `profile.get` / `applied.check` /
/// `status.update` / `answers.save` / `answers.suggest` / `match.live` /
/// `answer.assist` / `assist.cancel`; an unknown type gets an `import.result`
/// error reply (never a panic).
fn advance_authenticated(kind: &str, req_id: String, envelope: &Value) -> FrameDecision {
    match kind {
        msg::IMPORT_REQUEST => {
            let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
            FrameDecision::Import { req_id, payload }
        }
        // Assisted autofill: fetch the contact profile fresh (gated on the opt-in).
        msg::PROFILE_GET => FrameDecision::Profile { req_id },
        // "Have I already applied to this URL?" — pure, read-only store lookup.
        msg::APPLIED_CHECK => {
            let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
            FrameDecision::AppliedCheck { req_id, payload }
        }
        // "Mark this URL applied" — the narrowest possible write (saved → applied
        // on an exact URL-key match only).
        msg::STATUS_UPDATE => {
            let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
            FrameDecision::StatusUpdate { req_id, payload }
        }
        // "Is auto-track on?" — a pure read of the opt-in (no payload). Task #22.
        msg::AUTOTRACK_CHECK => FrameDecision::AutotrackCheck { req_id },
        // "Is assisted autofill on?" — a pure read of the opt-in (no payload).
        // Task #30, mirrors AUTOTRACK_CHECK exactly.
        msg::AUTOFILL_CHECK => FrameDecision::AutofillCheck { req_id },
        // "Save my answers from this page" — a consent-gated append-only write.
        msg::ANSWERS_SAVE => {
            let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
            FrameDecision::AnswersSave { req_id, payload }
        }
        // "Suggest answers for this form" — a consent-gated, read-only fuzzy match.
        msg::ANSWERS_SUGGEST => {
            let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
            FrameDecision::AnswersSuggest { req_id, payload }
        }
        // "Check fit" — score the résumé against the captured DOM.
        msg::MATCH_LIVE => {
            let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
            FrameDecision::MatchLive { req_id, payload }
        }
        // "Help me answer this question" — the first billable-AI bridge verb.
        msg::ANSWER_ASSIST => {
            let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
            FrameDecision::AnswerAssist { req_id, payload }
        }
        // Cancel an in-flight stream — no payload to read, `req_id` names the
        // target (see `msg::ASSIST_CANCEL`'s doc).
        msg::ASSIST_CANCEL => FrameDecision::AssistCancel { req_id },
        // The read-only agent/CLI surface (issue #1084 PR 1).
        msg::AGENT_QUERY => {
            let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
            FrameDecision::AgentQuery { req_id, payload }
        }
        // Unknown message types — acknowledged as an error, never panic.
        other => FrameDecision::Reply(import_flow::result_reply(
            &req_id,
            Err(AppError::Validation(format!(
                "unknown message type '{other}'"
            ))),
        )),
    }
}

/// Build the `challenge` reply (handshake step 2) carrying the fresh server nonce.
fn challenge_reply(req_id: &str, server_nonce: &str) -> String {
    json!({
        "type": msg::CHALLENGE,
        "reqId": req_id,
        "payload": { "serverNonce": server_nonce },
    })
    .to_string()
}

/// Build the `auth.ok` reply (handshake step 4) carrying the desktop's proof.
fn auth_ok_reply(req_id: &str, server_proof: &str) -> String {
    json!({
        "type": msg::AUTH_OK,
        "reqId": req_id,
        "payload": { "serverProof": server_proof },
    })
    .to_string()
}

/// Build the `update_required` force-cutover reply. Sent, then the socket closes,
/// when the first frame is not a valid protocol-2 `hello` (an old extension).
fn update_required_reply(req_id: &str) -> String {
    json!({
        "type": msg::UPDATE_REQUIRED,
        "reqId": req_id,
        "payload": {
            "error": "Update the AI Job Hunter browser extension to reconnect (bridge protocol v2)."
        },
    })
    .to_string()
}

// ── Assisted autofill (profile.get → profile.result) ──────────────────────────

/// The contact-profile fields sent to the extension for assisted autofill. A
/// flat, string-only projection of [`crate::contact_profile::ContactProfile`]
/// (location collapsed to its default free-text string) — the extension fills
/// matching empty form fields from it and never persists it. Every field is
/// optional (a sparse profile is normal); absent fields are omitted from the wire
/// payload entirely.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AutofillProfile {
    #[serde(skip_serializing_if = "Option::is_none")]
    full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    linkedin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    github: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    website: Option<String>,
    /// Additional labelled links (Portfolio, Dribbble, Stack Overflow, …) beyond
    /// the named platform fields — see [`clean_extra_links`] for the projection
    /// rules. Additive/optional on the wire: an old extension ignores the key,
    /// and it is omitted entirely (not `[]`) when there is nothing to send, so
    /// an old desktop's replies (which never carry it) parse identically.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    extra_links: Vec<crate::contact_profile::ContactLink>,
}

/// Cap on the number of extra links projected to the extension — a form has no
/// use for an unbounded list, and this bounds the reply size.
const MAX_EXTRA_LINKS: usize = 10;

/// Filter + cap the stored extra links for the wire: drop an entry with an empty
/// label, drop a url that (after trimming) is empty or not `http(s)`, then keep
/// at most [`MAX_EXTRA_LINKS`] of what remains, in order. `photo` is never
/// projected at all — unrelated to this list and always dropped.
fn clean_extra_links(
    links: &[crate::contact_profile::ContactLink],
) -> Vec<crate::contact_profile::ContactLink> {
    links
        .iter()
        .filter_map(|link| {
            let label = link.label.trim();
            let url = link.url.trim();
            if label.is_empty() {
                return None;
            }
            let lower = url.to_ascii_lowercase();
            if !(lower.starts_with("http://") || lower.starts_with("https://")) {
                return None;
            }
            Some(crate::contact_profile::ContactLink {
                label: label.to_string(),
                url: url.to_string(),
            })
        })
        .take(MAX_EXTRA_LINKS)
        .collect()
}

impl AutofillProfile {
    /// Project a stored [`ContactProfile`] to the flat autofill shape. Empty /
    /// whitespace-only values are dropped so the extension never fills a blank.
    fn from_contact(p: &crate::contact_profile::ContactProfile) -> Self {
        fn clean(v: &Option<String>) -> Option<String> {
            v.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        }
        Self {
            full_name: clean(&p.full_name),
            email: clean(&p.email),
            phone: clean(&p.phone),
            // Collapse the localized location to its default string; the extension
            // fills a single free-text location field.
            location: p
                .location
                .as_ref()
                .map(|l| l.default.trim().to_string())
                .filter(|s| !s.is_empty()),
            linkedin: clean(&p.linkedin),
            github: clean(&p.github),
            website: clean(&p.website),
            extra_links: clean_extra_links(&p.extra_links),
        }
    }
}

/// The opt-in-gated core of a `profile.get`: refuse with a clear, actionable
/// error when autofill is off (never silently return nothing), else project the
/// profile. Pure (no `AppHandle`) so the consent gate is unit-testable.
fn resolve_profile(
    enabled: bool,
    profile: Option<&crate::contact_profile::ContactProfile>,
) -> AppResult<AutofillProfile> {
    if !enabled {
        return Err(AppError::Validation(AUTOFILL_OFF_MESSAGE.to_string()));
    }
    let profile =
        profile.ok_or_else(|| AppError::Config("contact profile unavailable".to_string()))?;
    Ok(AutofillProfile::from_contact(profile))
}

/// Build a `profile.result` envelope (success carries the flat profile; refusal /
/// failure carries `error`). Mirrors [`import_flow::result_reply`] for the
/// import path.
fn profile_result_reply(req_id: &str, outcome: AppResult<AutofillProfile>) -> String {
    let payload = match outcome {
        Ok(p) => serde_json::to_value(&p).unwrap_or_else(|_| json!({})),
        Err(e) => json!({ "error": e.to_string() }),
    };
    json!({
        "type": msg::PROFILE_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

/// Read the opt-in + the contact profile off app state and resolve the
/// `profile.get` outcome. Fetch-fresh — nothing is cached; the desktop is the
/// sole owner of the PII. Factored out of [`handle_profile`] so the agent
/// `profile` resource ([`agent_read::profile_resource`], a CHILD module —
/// visible to it with no widening needed) reuses this EXACT consent gate +
/// projection rather than a second profile path.
fn profile_outcome(app: &AppHandle) -> AppResult<AutofillProfile> {
    let enabled = app
        .try_state::<BridgeState>()
        .map(|s| s.autofill_enabled())
        .unwrap_or(false);
    let profile = app
        .try_state::<crate::contact_profile::ContactProfileStore>()
        .map(|s| s.get());
    resolve_profile(enabled, profile.as_ref())
}

/// Answer an authenticated `profile.get`: return a ready-to-send
/// `profile.result` reply.
fn handle_profile(app: &AppHandle, req_id: &str) -> String {
    profile_result_reply(req_id, profile_outcome(app))
}

/// Manage the bridge state and register its factory-reset hook. Returns the
/// state handle so `start` can be wired right after. Mirrors the
/// `manage_resettable` pattern but is bridge-specific (it returns nothing app
/// state can't already resolve via `app.state::<BridgeState>()`).
pub fn manage(
    app: &tauri::App,
    registry: &mut crate::commands::privacy::ResetRegistry,
    data_dir: &Path,
) {
    crate::commands::privacy::manage_resettable(
        app,
        registry,
        "extension_bridge",
        BridgeState::load(data_dir),
    );
}
