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
//!    extension. The per-frame token (3) is what actually authenticates.
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
use std::sync::atomic::{AtomicBool, Ordering};

use futures::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;

use crate::applications::{
    normalize_job_url, ApplicationMeta, ApplicationOrigin, ApplicationStore,
};
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, APPLICATIONS_CHANGED};

pub mod auth;
pub mod handshake;
#[cfg(test)]
mod import_tests;
pub mod native_host;
pub mod register;
#[cfg(test)]
mod test;

/// Native-messaging host name — the registered identifier the browser uses to
/// spawn our relay (our exe in `--native-host` mode). MUST match the extension
/// side exactly (`apps/extension`). The host-manifest filename is this with a
/// `.json` suffix.
pub const NATIVE_HOST_NAME: &str = "app.aijobhunter.bridge";

/// On-disk host-manifest filename the browser reads to find + spawn the host.
pub const NATIVE_HOST_MANIFEST: &str = "app.aijobhunter.bridge.json";

/// Wire `type` strings — the Rust mirror of the shared `EXTENSION_MESSAGE_TYPES`
/// in `packages/shared/src/ipc/extension-protocol.ts`. A parity test
/// ([`test`]) pins these to the TS literals so the two can never drift.
pub mod msg {
    /// Handshake step 1 (extension → desktop): `{ protocol, clientNonce }`. NO
    /// token — the proof (step 3) authenticates. Must be the FIRST frame.
    pub const HELLO: &str = "hello";
    /// Handshake step 2 (desktop → extension): `{ serverNonce }`.
    pub const CHALLENGE: &str = "challenge";
    /// Handshake step 3 (extension → desktop): `{ proof }` where
    /// `proof = HMAC-SHA256(token, CLIENT_MSG)`. The token is NEVER on the wire
    /// in v2; the desktop verifies `proof` constant-time (see [`super::handshake`]).
    pub const AUTH: &str = "auth";
    /// Handshake step 4 (desktop → extension): `{ serverProof }` where
    /// `serverProof = HMAC-SHA256(token, SERVER_MSG)` — the desktop proving IT
    /// knows the token so the extension can reject a rogue/port-squatting peer.
    pub const AUTH_OK: &str = "auth.ok";
    /// Force-cutover reply (desktop → extension): sent, then the socket closes,
    /// when a connection's first frame is not a valid protocol-2 `hello` (e.g. an
    /// old extension's legacy `{type:'auth', token}` frame, or a lower protocol).
    pub const UPDATE_REQUIRED: &str = "update.required";
    pub const IMPORT_REQUEST: &str = "import.request";
    pub const IMPORT_RESULT: &str = "import.result";
    /// Extension → desktop: fetch the contact profile for assisted autofill; no
    /// payload (authed by the already-authenticated session). Returned only when
    /// the autofill opt-in is on, else a refusal `error`.
    pub const PROFILE_GET: &str = "profile.get";
    /// Desktop → extension: the contact-profile fields for autofill (or an `error`).
    pub const PROFILE_RESULT: &str = "profile.result";
    /// RESERVED (not handled yet) — live ATS match for the open posting.
    pub const MATCH_LIVE: &str = "match.live";
    /// RESERVED (not handled yet) — "have I already applied to this URL?".
    pub const APPLIED_CHECK: &str = "applied.check";
}

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

/// Managed Tauri state for the bridge. Commands read the bound port + token off
/// this; the server flips `connected` while a socket is paired.
pub struct BridgeState {
    /// `Some` once a port in [`PORT_RANGE`] bound; `None` if the bridge is
    /// disabled (no free port / startup failure).
    port: Mutex<Option<u16>>,
    /// The pairing secret. Persisted to disk; rotated by `regenerate`.
    token: Mutex<String>,
    /// Last-writer-wins status hint: set `true` once the extension's client
    /// proof verifies (the v2 mutual handshake completes — never on the bare WS
    /// handshake nor the `hello`/`challenge` exchange, so an unauthenticated
    /// client is never reported as connected) and `false` when a socket loop
    /// exits. It is **not** a refcount — this single bool assumes the de-facto
    /// single extension socket (loopback, one extension), so with concurrent
    /// sockets the second to close clears the flag while the first is still
    /// open. If concurrent sockets ever become real, promote to an
    /// `AtomicUsize` refcount.
    connected: AtomicBool,
    /// Assisted-autofill opt-in (default OFF, persisted to [`AUTOFILL_OPTIN_FILE`]).
    /// A `profile.get` returns the contact profile only while this is on; off ⇒
    /// the desktop replies with a clear refusal (never silently). This is the
    /// consent gate for sending the user's saved contact details into a page.
    autofill_enabled: AtomicBool,
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
            connected: AtomicBool::new(false),
            autofill_enabled: AtomicBool::new(load_autofill_optin(data_dir)),
            data_dir: data_dir.to_path_buf(),
        }
    }

    /// Current bound port, if any.
    pub fn port(&self) -> Option<u16> {
        *self.port.lock()
    }

    /// Whether an authenticated extension socket is currently paired.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// The current pairing token.
    pub fn token(&self) -> String {
        self.token.lock().clone()
    }

    /// Rotate the pairing token: generate a new secret, persist it, and return
    /// it. Any already-paired socket will fail its next per-frame token check
    /// and must re-pair with the new value.
    pub fn regenerate_token(&self) -> String {
        let fresh = new_token();
        *self.token.lock() = fresh.clone();
        if let Err(e) = persist_token(&self.data_dir, &fresh) {
            log::warn!("[extension_bridge] failed to persist regenerated token (non-fatal): {e}");
        }
        fresh
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
            log::warn!("[extension_bridge] failed to persist autofill opt-in (non-fatal): {e}");
        }
    }

    fn set_port(&self, port: Option<u16>) {
        *self.port.lock() = port;
    }

    fn set_connected(&self, connected: bool) {
        self.connected.store(connected, Ordering::Relaxed);
    }
}

/// Factory-reset hook: rotate the token so a wiped install re-pairs from scratch,
/// and return the autofill opt-in to its default OFF (consent must be re-granted).
impl crate::data_store::Resettable for BridgeState {
    fn reset(&self) {
        self.regenerate_token();
        self.set_autofill_enabled(false);
    }
}

/// A 32-byte random token, lowercase hex (64 chars).
fn new_token() -> String {
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Read the persisted token, or create + persist a fresh one on first run (or
/// if the stored value is corrupt/empty).
fn load_or_create_token(data_dir: &Path) -> String {
    let path = data_dir.join(TOKEN_FILE);
    if let Ok(s) = std::fs::read_to_string(&path) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let fresh = new_token();
    if let Err(e) = persist_token(data_dir, &fresh) {
        log::warn!("[extension_bridge] failed to persist initial token (non-fatal): {e}");
    }
    fresh
}

/// Read the persisted autofill opt-in (`"1"` ⇒ on). Absent / any other value ⇒
/// OFF, so a first run and a corrupt flag both default to the safe (off) state.
fn load_autofill_optin(data_dir: &Path) -> bool {
    std::fs::read_to_string(data_dir.join(AUTOFILL_OPTIN_FILE))
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}

fn persist_autofill_optin(data_dir: &Path, enabled: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(
        data_dir.join(AUTOFILL_OPTIN_FILE),
        if enabled { "1" } else { "0" },
    )
}

fn persist_token(data_dir: &Path, token: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    std::fs::write(data_dir.join(TOKEN_FILE), token)?;
    // Restrict the token file to the owner (best-effort) on unix so a
    // multi-user box can't read the pairing secret. Applies on both
    // first-create and rotate.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(
            data_dir.join(TOKEN_FILE),
            std::fs::Permissions::from_mode(0o600),
        );
    }
    Ok(())
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

/// Perform the WS handshake (validating `Origin`), then service frames until the
/// socket closes or fails a guard. `connected` flips true only once a frame
/// passes the per-frame token gate (not on the bare handshake); a wrong-token
/// frame replies `unauthorized` and closes the socket without marking connected.
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
            log::warn!("[extension_bridge] rejected handshake from origin: {origin:?}");
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
                // Includes the rejected-origin case (handshake refused with 403).
                log::warn!("[extension_bridge] handshake rejected/failed: {e}");
                return;
            }
        };

    let state = match app.try_state::<BridgeState>() {
        Some(s) => s,
        None => return,
    };
    // NOT marked connected yet: the bare WS handshake (loopback + origin) is not
    // authentication. The socket walks the v2 mutual handshake below; `connected`
    // flips true only once the extension's client proof verifies (an `AuthOk`
    // decision), so an unauthenticated socket is never reported as connected.

    let (mut writer, mut reader) = ws.split();
    // Per-connection handshake state; every socket starts by expecting `hello`.
    let mut conn = ConnState::AwaitingHello;

    while let Some(frame) = reader.next().await {
        let msg = match frame {
            Ok(m) => m,
            Err(e) => {
                log::warn!("[extension_bridge] read error: {e}");
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
                let _ = writer.send(Message::text(reply)).await;
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
                // is the socket authorized; mark it connected and reply auth.ok.
                conn = ConnState::Authenticated;
                state.set_connected(true);
                Some(reply)
            }
            FrameDecision::Reply(text) => Some(text),
            FrameDecision::Import { req_id, payload } => {
                let outcome = handle_import(&app, payload).await;
                Some(result_reply(&req_id, outcome))
            }
            FrameDecision::Profile { req_id } => Some(handle_profile(&app, &req_id)),
        };
        if let Some(reply) = reply {
            if writer.send(Message::text(reply)).await.is_err() {
                break;
            }
        }
    }

    state.set_connected(false);
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
    /// A ready-to-send reply from an authenticated frame (a reserved / unknown
    /// message type acknowledged as an error). Stays `Authenticated`.
    Reply(String),
    /// An authenticated `import.request` to dispatch through [`handle_import`].
    Import { req_id: String, payload: Value },
    /// An authenticated `profile.get` to answer through [`handle_profile`]. Carries
    /// no payload — the reply is gated on the autofill opt-in, not on any input.
    Profile { req_id: String },
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
/// token. Routes `import.request` / `profile.get`; reserved / unknown types get
/// an `import.result` error reply (never a panic).
fn advance_authenticated(kind: &str, req_id: String, envelope: &Value) -> FrameDecision {
    match kind {
        msg::IMPORT_REQUEST => {
            let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
            FrameDecision::Import { req_id, payload }
        }
        // Assisted autofill: fetch the contact profile fresh (gated on the opt-in).
        msg::PROFILE_GET => FrameDecision::Profile { req_id },
        // Reserved message types — acknowledged as unimplemented, never panic.
        msg::MATCH_LIVE | msg::APPLIED_CHECK => FrameDecision::Reply(result_reply(
            &req_id,
            Err(AppError::Validation(format!(
                "message type '{kind}' is not implemented"
            ))),
        )),
        other => FrameDecision::Reply(result_reply(
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

/// The successful import outcome: the created/merged application id, its status,
/// and the parsed title/company (so the popup can name the imported job).
struct ImportOk {
    application_id: String,
    status: String,
    title: String,
    company: String,
    /// True when nothing usable parsed and a stub was persisted (empty title) —
    /// the extension surfaces this so the user knows to complete the row.
    partial: bool,
}

/// Build a canonical `import.result` envelope (success or error). The error's
/// `to_string()` becomes the `error` field the extension surfaces.
fn result_reply(req_id: &str, outcome: AppResult<ImportOk>) -> String {
    let payload = match outcome {
        Ok(ok) => json!({
            "applicationId": ok.application_id,
            "status": ok.status,
            "title": ok.title,
            "company": ok.company,
            "partial": ok.partial,
        }),
        Err(e) => json!({ "error": e.to_string() }),
    };
    json!({
        "type": msg::IMPORT_RESULT,
        "reqId": req_id,
        "payload": payload,
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
        return Err(AppError::Validation(
            "Autofill is off. Turn it on in AI Job Hunter → Settings → Accounts → Browser extension."
                .to_string(),
        ));
    }
    let profile =
        profile.ok_or_else(|| AppError::Config("contact profile unavailable".to_string()))?;
    Ok(AutofillProfile::from_contact(profile))
}

/// Build a `profile.result` envelope (success carries the flat profile; refusal /
/// failure carries `error`). Mirrors [`result_reply`] for the import path.
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

/// Answer an authenticated `profile.get`: read the opt-in + the contact profile
/// off app state and return a ready-to-send `profile.result` reply. Fetch-fresh —
/// nothing is cached; the desktop is the sole owner of the PII.
fn handle_profile(app: &AppHandle, req_id: &str) -> String {
    let enabled = app
        .try_state::<BridgeState>()
        .map(|s| s.autofill_enabled())
        .unwrap_or(false);
    let profile = app
        .try_state::<crate::contact_profile::ContactProfileStore>()
        .map(|s| s.get());
    profile_result_reply(req_id, resolve_profile(enabled, profile.as_ref()))
}

/// Persist a parsed [`JobPosting`] from an import as a Saved Application and
/// return `(application_id, status_id)`. This is the *entire* persistence side
/// effect of an import: it touches the [`ApplicationStore`] only and has **no
/// access to the `PostingsCache`**, so an import can never enter the
/// Jobs/discovery feed. Split out of [`handle_import`] (which needs an
/// `AppHandle` for event/notification plumbing) so the import → Application
/// contract is unit-testable without a Tauri app — see `import_tests.rs`.
fn persist_import_application(
    store: &ApplicationStore,
    normalized_url: &str,
    posting: &crate::scraping::types::JobPosting,
    applied: Option<bool>,
) -> AppResult<(String, String)> {
    let meta = ApplicationMeta {
        company: posting.company.clone(),
        title: posting.title.clone(),
        job_description: posting.description.clone().unwrap_or_default(),
        ..Default::default()
    };
    let id = store.upsert_for_origin(
        normalized_url,
        &posting.source,
        &meta,
        ApplicationOrigin::Saved,
        applied,
    )?;
    let status = store
        .get(&id)
        .map(|a| a.status.as_id().to_string())
        .unwrap_or_else(|| "saved".to_string());
    Ok((id, status))
}

/// A posting is usable for an import only if it carries a real title; an
/// empty-title parse means the extractor degraded (blocked fetch / unknown page).
fn usable(p: &crate::scraping::types::JobPosting) -> bool {
    !p.title.trim().is_empty()
}

/// Core import: parse the posting (Scan mode from provided HTML, else URL mode
/// via the resolver), upsert the Applications aggregate from it (Application
/// only — not the postings cache), emit the change event, and return the
/// application id + status.
async fn handle_import(app: &AppHandle, payload: Value) -> AppResult<ImportOk> {
    let url = payload
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let html = payload
        .get("html")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let applied = payload.get("applied").and_then(|v| v.as_bool());

    if url.is_empty() {
        return Err(AppError::Validation("url is required".to_string()));
    }

    // Centralized SPA/list-view normalization: if the user imported from a board's
    // search/SPA view (selected job id in a query param), rewrite to the canonical
    // single-job URL so BOTH import modes resolve the SELECTED job, not the list
    // shell. `None` → already a direct page / unknown host (use the URL as-is).
    let canonical = crate::scraping::scrape_url::canonical_job_url(&url);
    let effective_url = canonical.as_deref().unwrap_or(url.as_str());

    // URL / SSRF safety on whatever we will actually fetch + store (the canonical
    // link when rewritten, else the original). Normalize (http(s) only) then guard
    // the host against loopback/private/link-local/`*.local`.
    let normalized = normalize_job_url(effective_url);
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "url is not a valid http(s) URL".to_string(),
        ));
    }
    if !auth::is_safe_import_url(effective_url) {
        return Err(AppError::Validation(
            "url host is not allowed (private/loopback)".to_string(),
        ));
    }

    // Shared rate + concurrency budget with the scrape_resolve_url IPC command.
    // resolve() now follows up to 2 redirect hops per call (up to 3 outbound
    // fetches), so every resolve() call must hold a limiter slot — the same
    // "scrape_url" key and constants the command uses. `try_state` gracefully
    // handles the (startup-failure) case where Limiter was never managed.
    let limiter = app
        .try_state::<std::sync::Arc<crate::limits::Limiter>>()
        .map(|s| s.inner().clone());
    let acquire_slot = || -> AppResult<Option<crate::limits::ConcurrencyGuard>> {
        match &limiter {
            Some(l) => l
                .acquire(
                    "scrape_url",
                    crate::limits::SCRAPE_RATE_MAX,
                    crate::limits::SCRAPE_CONCURRENCY_MAX,
                )
                .map(Some),
            None => Ok(None), // Limiter not managed (startup failure) — allow through
        }
    };

    // At most one network fetch. The captured DOM is parsed ONLY for a direct page
    // (no canonical rewrite) — for a SPA/list view the DOM is the list shell, not the
    // selected job, so we resolve the canonical URL instead and never parse the shell.
    let mut posting: Option<crate::scraping::types::JobPosting> =
        if let Some(c) = canonical.as_deref() {
            let _guard = acquire_slot()?;
            crate::scraping::scrape_url::resolve(c).await? // SPA/list view → selected job's canonical URL
        } else if let Some(h) = html.as_deref() {
            crate::scraping::scrape_url::parse_from_html(&url, h) // direct page → captured authenticated DOM
        } else {
            let _guard = acquire_slot()?;
            crate::scraping::scrape_url::resolve(effective_url).await? // URL mode, no DOM → server fetch
        };

    // Single server-fetch fallback: only the direct-page DOM path that came up unusable
    // (a board API may resolve the same URL where the DOM parse missed). Skipped for the
    // canonical and URL-mode branches because they already fetched `effective_url`.
    if !posting.as_ref().is_some_and(usable) && canonical.is_none() && html.is_some() {
        let _guard = acquire_slot()?;
        if let Some(p) = crate::scraping::scrape_url::resolve(effective_url).await? {
            if usable(&p) || posting.is_none() {
                posting = Some(p);
            }
        }
    }

    // Never lose an import click: if nothing usable parsed, persist a stub the user
    // can complete later (title empty → flagged partial), instead of erroring out.
    let (posting, partial) = if posting.as_ref().is_some_and(usable) {
        (posting.unwrap(), false)
    } else {
        let host = reqwest::Url::parse(effective_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .unwrap_or_default();
        let stub = crate::scraping::types::JobPosting {
            id: format!("url:{effective_url}"),
            external_id: None,
            title: String::new(),
            company: host,
            location: None,
            url: effective_url.to_string(),
            source: "url".to_string(),
            description: None,
            requirements: None,
            posted_at: None,
            captured_at: chrono::Utc::now().timestamp_millis(),
            extra: std::collections::HashMap::new(),
        };
        (stub, true)
    };

    // An import is a deliberate pursuit, NOT a discovery: it creates only the
    // status-bearing Application below. It is intentionally NOT added to the
    // in-memory postings cache (the Jobs/discovery feed via
    // `commands::scrape::scrape_list_postings`), so an imported job shows up
    // under Applications only — never in the Jobs page. The Application carries
    // the title/company the detail-page tailoring needs; the JD is re-resolved
    // there if required.

    // Upsert the status-bearing Application (Saved origin → `saved` unless the
    // request flags it applied). Merges onto any existing row for this URL.
    let store = app
        .try_state::<ApplicationStore>()
        .ok_or_else(|| AppError::Config("applications store unavailable".to_string()))?;
    let (id, status) = persist_import_application(store.inner(), &normalized, &posting, applied)?;

    // A partial stub has an empty title — fall back to the company (host) so the
    // event payload and toast still name something the user recognizes.
    let title_is_blank = posting.title.trim().is_empty();
    let display_name = if title_is_blank {
        posting.company.clone()
    } else {
        posting.title.clone()
    };
    let body = if title_is_blank {
        posting.company.clone()
    } else {
        format!("{} · {}", posting.title, posting.company)
    };

    // Tell the renderer to refresh (Applications + Jobs views) and surface a
    // live toast. Carry the title/company/status so the toast can name the job
    // without a refetch race.
    emit_event(
        app,
        APPLICATIONS_CHANGED,
        json!({
            "applicationId": id.clone(),
            "title": display_name.clone(),
            "company": posting.company.clone(),
            "status": status.clone(),
        }),
    );

    // Also drop a Notification Center record. Best-effort and additive — the
    // lists still refresh via the `applications:changed` emit above; this only
    // adds the inbox entry + a focused-window toast, with an OS banner only when
    // the window is unfocused (the import UX intent). Route → the Applications
    // view, highlighting the just-imported row.
    let mut search = serde_json::Map::new();
    search.insert("highlight".to_string(), Value::String(id.clone()));
    crate::commands::notifications::push_and_notify(
        app,
        crate::notifications::NewNotification {
            kind: "import.result".to_string(),
            title: format!("Imported {display_name}"),
            body,
            route: Some(crate::notifications::NotificationRoute {
                to: "/applications".to_string(),
                search: Some(search),
            }),
        },
        crate::commands::notifications::OsBanner::WhenUnfocused,
    );

    Ok(ImportOk {
        application_id: id,
        status,
        title: posting.title,
        company: posting.company,
        partial,
    })
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
