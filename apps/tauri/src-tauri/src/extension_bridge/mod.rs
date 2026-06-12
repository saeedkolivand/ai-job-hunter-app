//! Browser-extension ⇄ desktop bridge — a loopback-only WebSocket server.
//!
//! Feature 2: the browser extension's "Save this job" button opens a WS to the
//! desktop app and sends an [`import.request`](shared `extension-protocol.ts`)
//! frame. The desktop scrapes/parses the posting, persists it the same way the
//! in-app scrape path does (postings cache + [`crate::applications`] aggregate),
//! and replies with `import.result`.
//!
//! ## Security model (layered — see [`auth`])
//! 1. **Loopback only** — the listener binds `127.0.0.1`; no LAN/remote reach.
//! 2. **Origin allowlist** — the WS handshake's `Origin` must be a
//!    `chrome-extension://`/`moz-extension://` id in [`auth::ALLOWED_EXTENSION_IDS`]
//!    (or a dev override from `platform::config::extension_dev_origins`).
//! 3. **Per-frame token** — every envelope carries the paired secret; a mismatch
//!    closes the socket.
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
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;

use crate::applications::{
    normalize_job_url, ApplicationMeta, ApplicationOrigin, ApplicationStore,
};
use crate::error::{AppError, AppResult};

pub mod auth;
#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod test;

/// Wire `type` strings — the Rust mirror of the shared `EXTENSION_MESSAGE_TYPES`
/// in `packages/shared/src/ipc/extension-protocol.ts`. A parity test
/// ([`test`]) pins these to the TS literals so the two can never drift.
pub mod msg {
    pub const IMPORT_REQUEST: &str = "import.request";
    pub const IMPORT_RESULT: &str = "import.result";
    /// RESERVED (not handled yet) — live ATS match for the open posting.
    pub const MATCH_LIVE: &str = "match.live";
    /// RESERVED (not handled yet) — "have I already applied to this URL?".
    pub const APPLIED_CHECK: &str = "applied.check";
}

/// The Tauri event the bridge emits after a successful import so the renderer
/// live-refreshes its Applications view. The frontend slice subscribes to this.
pub const APPLICATIONS_CHANGED_EVENT: &str = "applications:changed";

/// Hard cap on a single WS message. Job HTML can be large (a few hundred KB), so
/// 2 MB leaves generous headroom while blocking a memory-exhaustion frame.
pub const MAX_FRAME_BYTES: usize = 2 * 1024 * 1024;

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
    /// True while at least one authenticated socket is open.
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

/// Spawn the bridge server on the existing tokio runtime. Fire-and-forget: a
/// bind failure logs and leaves the bridge disabled (port stays `None`) — it
/// never panics or blocks boot. Call once from the Tauri `setup`.
pub fn start(app: AppHandle) {
    tokio::spawn(async move {
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
                    tokio::spawn(async move {
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
/// socket closes or fails a guard. One authenticated socket flips `connected`.
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
    state.set_connected(true);

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
        // An over-cap frame closes the socket; a bad token / unknown type replies
        // with an error; only an authenticated import request reaches `app` state.
        let reply = match classify_frame(&state, &text) {
            FrameDecision::CloseOverCap => {
                log::warn!("[extension_bridge] frame over size cap — closing");
                break;
            }
            FrameDecision::Drop => None,
            FrameDecision::Reply(text) => Some(text),
            FrameDecision::Import { req_id, payload } => {
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
    /// A ready-to-send `import.result` reply text (bad token, or a reserved /
    /// unknown message type acknowledged as an error).
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
        return FrameDecision::Reply(result_reply(
            &req_id,
            Err(AppError::Validation("unauthorized".to_string())),
        ));
    }

    match kind {
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

/// The successful import outcome: the created/merged application id + its status.
struct ImportOk {
    application_id: String,
    status: String,
}

/// Build a canonical `import.result` envelope (success or error). The error's
/// `to_string()` becomes the `error` field the extension surfaces.
fn result_reply(req_id: &str, outcome: AppResult<ImportOk>) -> String {
    let payload = match outcome {
        Ok(ok) => json!({
            "applicationId": ok.application_id,
            "status": ok.status,
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

/// Core import: parse the posting (Scan mode from provided HTML, else URL mode
/// via the resolver), persist it into the postings cache + the Applications
/// aggregate, emit the change event, and return the application id + status.
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

    // URL / SSRF safety: normalize (http(s) only — empty means rejected) then
    // guard the host against loopback/private/link-local/`*.local`.
    let normalized = normalize_job_url(&url);
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "url is not a valid http(s) URL".to_string(),
        ));
    }
    if !auth::is_safe_import_url(&url) {
        return Err(AppError::Validation(
            "url host is not allowed (private/loopback)".to_string(),
        ));
    }

    // Parse the posting. Scan mode reuses the fetch-free parser on the supplied
    // (authenticated) DOM; URL mode runs the full resolver.
    let posting = match html {
        Some(html) => crate::scraping::scrape_url::parse_from_html(&url, &html),
        None => crate::scraping::scrape_url::resolve(&url).await?,
    };
    let posting = match posting {
        Some(p) => p,
        None => {
            return Err(AppError::Parse(
                "could not parse a job posting from this page".to_string(),
            ))
        }
    };

    // Persist into the in-memory postings cache so the Jobs page shows it
    // (mirrors commands::scrape's add). Best-effort.
    if let Some(cache) = app.try_state::<Mutex<crate::postings::PostingsCache>>() {
        if let Ok(item_json) = serde_json::to_value(&posting) {
            cache.lock().add(item_json);
        }
    }

    // Upsert the status-bearing Application (Saved origin → `saved` unless the
    // request flags it applied). Merges onto any existing row for this URL.
    let store = app
        .try_state::<ApplicationStore>()
        .ok_or_else(|| AppError::Config("applications store unavailable".to_string()))?;
    let meta = ApplicationMeta {
        company: posting.company.clone(),
        title: posting.title.clone(),
        ..Default::default()
    };
    let id = store.upsert_for_origin(
        &normalized,
        &posting.source,
        &meta,
        ApplicationOrigin::Saved,
        applied,
    )?;

    let status = store
        .get(&id)
        .map(|a| a.status.as_id().to_string())
        .unwrap_or_else(|| "saved".to_string());

    // Tell the renderer to refresh (Applications + Jobs views).
    let _ = app.emit(APPLICATIONS_CHANGED_EVENT, json!({ "applicationId": id }));

    Ok(ImportOk {
        application_id: id,
        status,
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
