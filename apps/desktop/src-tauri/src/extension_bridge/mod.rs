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
//! 3. **Per-frame token** — every envelope carries the paired secret; a mismatch
//!    is rejected with the `unauthorized` reply and closes the socket. The
//!    extension verifies the pairing up front with an [`msg::AUTH`] frame sent
//!    immediately after the socket opens (a token check with no payload): a
//!    correct token gets an `import.result` reply with **no** `error`, a wrong
//!    token gets the unauthorized reply and the socket is closed. The handshake
//!    (1–2) alone does NOT mark the socket connected: `connected` flips true only
//!    once a frame passes the token gate, so a wrong-token socket is never
//!    reported as connected/authorized (see [`handle_connection`]).
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
    /// Connection-time token verification; no payload. The extension sends this
    /// first frame right after the socket opens; the desktop replies with an
    /// `import.result` envelope carrying no `error` on success (or the
    /// unauthorized error reply on a bad token, after which the socket closes).
    pub const AUTH: &str = "auth";
    pub const IMPORT_REQUEST: &str = "import.request";
    pub const IMPORT_RESULT: &str = "import.result";
    /// RESERVED (not handled yet) — live ATS match for the open posting.
    pub const MATCH_LIVE: &str = "match.live";
    /// RESERVED (not handled yet) — "have I already applied to this URL?".
    pub const APPLIED_CHECK: &str = "applied.check";
}

/// Hard cap on a single WS message. A job page's full `outerHTML` can run to a
/// few MB, so 8 MB matches the scraper's per-response cap
/// ([`crate::scraping::http`]) — a full-page DOM capture isn't silently dropped
/// — while still blocking a memory-exhaustion frame.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// First port tried, then the rest of the inclusive range until one binds.
const PORT_RANGE: std::ops::RangeInclusive<u16> = 47615..=47620;

/// File under the app data dir holding the persisted pairing token.
const TOKEN_FILE: &str = "extension_token";

/// Managed Tauri state for the bridge. Commands read the bound port + token off
/// this; the server flips `connected` while a socket is paired.
pub struct BridgeState {
    /// `Some` once a port in [`PORT_RANGE`] bound; `None` if the bridge is
    /// disabled (no free port / startup failure).
    port: Mutex<Option<u16>>,
    /// The pairing secret. Persisted to disk; rotated by `regenerate`.
    token: Mutex<String>,
    /// Last-writer-wins status hint: set `true` once a frame passes the
    /// per-frame token gate (never on the bare handshake, so a wrong-token
    /// client is never reported as connected) and `false` when a socket loop
    /// exits. It is **not** a refcount — this single bool assumes the de-facto
    /// single extension socket (loopback, one extension), so with concurrent
    /// sockets the second to close clears the flag while the first is still
    /// open. If concurrent sockets ever become real, promote to an
    /// `AtomicUsize` refcount.
    connected: AtomicBool,
    /// App data dir — where the token file lives.
    data_dir: PathBuf,
}

impl BridgeState {
    /// Load (or first-run create + persist) the pairing token, returning a state
    /// with no port yet (the server sets it once bound).
    pub fn load(data_dir: &Path) -> Self {
        let token = load_or_create_token(data_dir);
        Self {
            port: Mutex::new(None),
            token: Mutex::new(token),
            connected: AtomicBool::new(false),
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

    fn set_port(&self, port: Option<u16>) {
        *self.port.lock() = port;
    }

    fn set_connected(&self, connected: bool) {
        self.connected.store(connected, Ordering::Relaxed);
    }
}

/// Factory-reset hook: rotate the token so a wiped install re-pairs from scratch.
impl crate::data_store::Resettable for BridgeState {
    fn reset(&self) {
        self.regenerate_token();
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
    // NOT marked connected yet: the bare handshake (loopback + origin) is not
    // authentication. `connected` flips true only once a frame passes the
    // per-frame token gate (an `Import` or a `Reply` from `classify_frame`), so
    // a wrong-token socket is never reported as connected/authorized.

    let (mut writer, mut reader) = ws.split();

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

        // Classify the frame against the size + token + type gates (no app state).
        // An over-cap frame closes the socket; a bad token replies `unauthorized`
        // then closes; a reserved/unknown type replies with an error; only an
        // authenticated import request reaches `app` state.
        let reply = match classify_frame(&state, &text) {
            FrameDecision::CloseOverCap => {
                log::warn!("[extension_bridge] frame over size cap — closing");
                break;
            }
            FrameDecision::Drop => None,
            FrameDecision::Unauthorized(reply) => {
                // A wrong-token client must not linger: send the unauthorized
                // reply (best-effort) and close the socket without ever marking
                // it connected.
                let _ = writer.send(Message::text(reply)).await;
                break;
            }
            FrameDecision::Reply(text) => {
                // A token-validated frame (e.g. the `auth` ok reply, or a
                // reserved-type error) — the socket is authenticated, mark it
                // connected.
                state.set_connected(true);
                Some(text)
            }
            FrameDecision::Import { req_id, payload } => {
                // A token-validated import — authenticated, mark connected.
                state.set_connected(true);
                let outcome = handle_import(&app, payload).await;
                Some(result_reply(&req_id, outcome))
            }
        };
        if let Some(reply) = reply {
            if writer.send(Message::text(reply)).await.is_err() {
                break;
            }
        }
    }

    state.set_connected(false);
}

/// Outcome of the per-frame security/dispatch decision, isolated from any
/// `AppHandle` so the size + token + type gates are unit-testable. The connection
/// loop runs the (async, app-stateful) import only for [`FrameDecision::Import`];
/// every other variant is resolved here from pure inputs.
#[cfg_attr(test, derive(Debug))]
enum FrameDecision {
    /// Frame exceeds [`MAX_FRAME_BYTES`] — close the socket without parsing.
    CloseOverCap,
    /// Not JSON (malformed) — drop silently, no reply.
    Drop,
    /// A bad-token frame: send this ready-to-send `unauthorized` `import.result`
    /// reply text, then CLOSE the socket. Distinct from [`FrameDecision::Reply`]
    /// so the connection loop never marks an unauthorized socket connected and
    /// never lets a wrong-token client linger.
    Unauthorized(String),
    /// A ready-to-send `import.result` reply text from a **token-validated**
    /// frame (a reserved / unknown message type acknowledged as an error, or the
    /// `auth` success reply). The socket stays open and is marked connected.
    Reply(String),
    /// An authenticated `import.request` to dispatch through [`handle_import`].
    Import { req_id: String, payload: Value },
}

/// The per-message security gate + dispatch routing, with the size cap, the JSON
/// parse, the per-frame token check, and the type match — everything that does
/// NOT need an `AppHandle`. Behaviorally identical to the prior inline path in
/// [`handle_connection`]/`process_frame`: same order (size → parse → token →
/// type), same replies (`unauthorized` / "not implemented" / "unknown message
/// type"), and the same close-on-over-cap semantics.
fn classify_frame(state: &BridgeState, text: &str) -> FrameDecision {
    if text.len() > MAX_FRAME_BYTES {
        return FrameDecision::CloseOverCap;
    }

    let envelope: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return FrameDecision::Drop, // not JSON — drop silently
    };

    let token = envelope.get("token").and_then(|v| v.as_str()).unwrap_or("");
    let req_id = envelope
        .get("reqId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let kind = envelope.get("type").and_then(|v| v.as_str()).unwrap_or("");

    // Per-frame token check. A mismatch is an auth failure — reply with an error
    // (so the caller learns to re-pair) but do NOT echo or confirm the token.
    // Plain `!=` (non-constant-time) is acceptable here: the bridge binds
    // loopback only (127.0.0.1) and the token is a 256-bit random secret, so
    // remote timing side-channels are not a practical risk.
    if token.is_empty() || token != state.token() {
        log::warn!("[extension_bridge] rejected frame: bad token");
        return FrameDecision::Unauthorized(result_reply(
            &req_id,
            Err(AppError::Validation("unauthorized".to_string())),
        ));
    }

    match kind {
        // Connection-time token check. The token already passed the gate above,
        // so a minimal `Ok` outcome — serializing with NO `error` field — is the
        // "authorized" reply the extension expects (no error = authorized). An
        // `auth` frame does no import; the empty fields are never read.
        msg::AUTH => FrameDecision::Reply(result_reply(
            &req_id,
            Ok(ImportOk {
                application_id: String::new(),
                status: String::new(),
                title: String::new(),
                company: String::new(),
                partial: false,
            }),
        )),
        msg::IMPORT_REQUEST => {
            let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
            FrameDecision::Import { req_id, payload }
        }
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
