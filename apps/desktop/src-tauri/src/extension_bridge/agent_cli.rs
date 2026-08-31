//! `ajh-tauri agent <verb>` — a thin CLI client over the loopback bridge
//! (issue #1084 PR 1, CLIENT half; the server half is [`super::agent_read`]).
//!
//! Runs as a MODE of the existing `ajh-tauri` binary, selected by an argv
//! sentinel in `main.rs`/`lib::run_agent_cli_if_invoked` — never a second
//! `[[bin]]` (the release upload globs only read `target/release/bundle/**`,
//! so a second binary would ship to nobody). The app must already be
//! running: this sends ONE `agent.query` frame over the same v2
//! mutual-HMAC-authenticated WebSocket the browser extension uses, prints the
//! `agent.result` payload as JSON on stdout, and exits. No DB access, no HTTP
//! server, no new port.
//!
//! ## Why not [`super::native_host::connect_bridge`]
//! That function returns on the first successful WS UPGRADE, before any
//! protocol frame — a port-squatter (anything else listening in
//! [`super::PORT_RANGE`]) would take a native-host relay down; for the CLI it
//! would misreport a squatter as a successful connection with no way to send
//! `agent.query` at all. [`connect_authenticated`] instead drives the FULL
//! v2 handshake (`hello`→`challenge`→`auth`→`auth.ok`) per candidate port and
//! only accepts the one whose **server** proof verifies (see
//! [`super::handshake::verify_server_proof`], added for this client — there
//! was previously no Rust-side implementation of this handshake; the browser
//! extension's lives in TS, `apps/extension/src/lib/bridge.ts`).
//!
//! ## Exit codes (the process-level contract — see [`run`])
//! - `0` — `agent.result` replied `{"ok":true,...}`; the payload is on stdout.
//! - `1` — `agent.result` replied `{"ok":false,...}` (a server-side refusal:
//!   rate-limited, validation, not-found, autofill off, …) — the payload
//!   (including the fixed-sentinel `error` text) is still on stdout.
//! - `2` — the round trip never completed: bad CLI usage, the app is not
//!   running, or the connection failed for a reason that says nothing about
//!   whether the pairing token itself is valid. A synthesized
//!   `{"ok":false,"resource":…,"error":<fixed sentinel>}` is printed instead
//!   of the (nonexistent) server payload. Never a raw absolute path or an
//!   echoed I/O error string — only fixed sentinels, so this CLI's own stdout
//!   never leaks a path into whatever reads it (an LLM agent's context).

use std::path::Path;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::time::{timeout, Instant};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{ClientRequestBuilder, Message};

use crate::error::{AppError, AppResult};

use super::auth::NATIVE_HOST_ORIGIN;
use super::{handshake, msg, MAX_FRAME_BYTES, PORT_RANGE, PROTOCOL_VERSION, TOKEN_FILE};

type WsStream = tokio_tungstenite::WebSocketStream<TcpStream>;

/// Wall-clock bound on each individual handshake step (send hello → await
/// challenge; send auth → await auth.ok). Generous for a loopback round trip;
/// short enough that one hung/squatting port can't stall the whole
/// invocation across [`PORT_RANGE`].
const HANDSHAKE_STEP_TIMEOUT: Duration = Duration::from_secs(5);

/// Wall-clock bound on the `agent.query` round trip itself, AFTER
/// authentication — sized above `best-matches`' measured worst case (~12.3s
/// at 4000 found jobs; see `agent_read`'s throttle doc), not the handshake
/// budget above.
const QUERY_REPLY_TIMEOUT: Duration = Duration::from_secs(30);

// ── argv → verb ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Verb {
    BestMatches { limit: Option<u64> },
    Job { url: String },
    Profile,
    Automations,
    Schema,
}

impl Verb {
    fn resource_name(&self) -> &'static str {
        match self {
            Verb::BestMatches { .. } => "best-matches",
            Verb::Job { .. } => "job",
            Verb::Profile => "profile",
            Verb::Automations => "automations",
            Verb::Schema => "schema",
        }
    }

    /// The `agent.query` frame's `payload` object for this verb.
    fn payload(&self) -> Value {
        match self {
            Verb::BestMatches { limit } => {
                let mut p = json!({ "resource": self.resource_name() });
                if let Some(limit) = limit {
                    p["limit"] = json!(limit);
                }
                p
            }
            Verb::Job { url } => json!({ "resource": self.resource_name(), "url": url }),
            Verb::Profile | Verb::Automations | Verb::Schema => {
                json!({ "resource": self.resource_name() })
            }
        }
    }
}

/// Parse `args` (excludes the program name AND the `agent` sentinel itself —
/// e.g. `["best-matches", "--limit", "10"]`). Every error message here is
/// built only from CLI flag/verb tokens the caller itself typed — never a
/// path — so it is safe to echo back verbatim. `AppError::Validation` per
/// `rust-standards`' R6 (no stringly-typed `Result<_, String>` outside
/// `error.rs`), even for this process-local, never-IPC-round-tripped parse.
fn parse_verb(args: &[String]) -> AppResult<Verb> {
    match args.first().map(String::as_str) {
        Some("best-matches") => parse_best_matches(&args[1..]),
        Some("job") => {
            let url = args
                .get(1)
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::Validation("job requires a <url> argument".to_string()))?;
            Ok(Verb::Job {
                url: url.to_string(),
            })
        }
        Some("profile") => Ok(Verb::Profile),
        Some("automations") => Ok(Verb::Automations),
        Some("schema") => Ok(Verb::Schema),
        Some(other) => Err(AppError::Validation(format!(
            "unknown verb '{other}' (expected best-matches|job|profile|automations|schema)"
        ))),
        None => Err(AppError::Validation(
            "missing verb (best-matches|job|profile|automations|schema)".to_string(),
        )),
    }
}

fn parse_best_matches(rest: &[String]) -> AppResult<Verb> {
    let mut limit = None;
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "--limit" => {
                let raw = rest
                    .get(i + 1)
                    .ok_or_else(|| AppError::Validation("--limit requires a value".to_string()))?;
                limit = Some(raw.parse::<u64>().map_err(|_| {
                    AppError::Validation("--limit must be a non-negative integer".to_string())
                })?);
                i += 2;
            }
            other => return Err(AppError::Validation(format!("unknown argument '{other}'"))),
        }
    }
    Ok(Verb::BestMatches { limit })
}

// ── agent-CLI pointer (written by `super::register::write_agent_pointer`) ──

#[derive(Debug, Deserialize)]
struct AgentPointer {
    #[serde(rename = "dataDir")]
    data_dir: String,
}

/// Read + parse the pointer file, or `None` on any I/O/parse failure — the
/// caller folds that uniformly into `app_not_running` (without a data dir
/// there is no token file to read either, so there is nothing more specific
/// to say). Never logs/echoes the path itself (path privacy).
fn read_agent_pointer() -> Option<AgentPointer> {
    let path = crate::platform::config::agent_pointer_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Read the persisted pairing token from `data_dir` — the exact file
/// [`super::persist::persist_token`] writes, read the same way
/// [`super::persist::load_or_create_token`] does (trimmed, empty ⇒ absent).
fn read_pairing_token(data_dir: &str) -> Option<String> {
    let text = std::fs::read_to_string(Path::new(data_dir).join(TOKEN_FILE)).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

// ── v2 mutual handshake — the client half (see the module doc) ─────────────

/// Build the `hello` frame (handshake step 1).
fn build_hello(client_nonce: &str) -> String {
    json!({
        "type": msg::HELLO,
        "reqId": "cli-hello",
        "payload": { "protocol": PROTOCOL_VERSION, "clientNonce": client_nonce },
    })
    .to_string()
}

/// Extract `serverNonce` from a `challenge` reply, validating its shape
/// (mirrors the server's own `is_valid_nonce` check on the client nonce).
/// `None` for anything that isn't a well-formed challenge.
fn parse_challenge(v: &Value) -> Option<String> {
    if v.get("type").and_then(Value::as_str) != Some(msg::CHALLENGE) {
        return None;
    }
    let nonce = v.get("payload")?.get("serverNonce")?.as_str()?;
    handshake::is_valid_nonce(nonce).then(|| nonce.to_string())
}

/// Build the `auth` frame (handshake step 3) carrying the client's proof.
fn build_auth(proof: &str) -> String {
    json!({
        "type": msg::AUTH,
        "reqId": "cli-auth",
        "payload": { "proof": proof },
    })
    .to_string()
}

/// Extract `serverProof` from an `auth.ok` reply. `None` for anything else
/// (a different type, a missing/non-string field).
fn parse_auth_ok(v: &Value) -> Option<String> {
    if v.get("type").and_then(Value::as_str) != Some(msg::AUTH_OK) {
        return None;
    }
    Some(v.get("payload")?.get("serverProof")?.as_str()?.to_string())
}

/// Read the next parseable JSON text frame within `dur`, silently skipping
/// ping/pong control frames. `None` on timeout, a transport error, a close,
/// or non-JSON content — every one of those collapses to the same "this port
/// gave us nothing usable" signal for the caller.
async fn next_json(ws: &mut WsStream, dur: Duration) -> Option<Value> {
    loop {
        let msg = match timeout(dur, ws.next()).await {
            Ok(Some(Ok(m))) => m,
            _ => return None,
        };
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Binary(b) => String::from_utf8(b.to_vec()).ok()?,
            Message::Close(_) => return None,
            _ => continue,
        };
        return serde_json::from_str(&text).ok();
    }
}

/// One candidate port's outcome, coarse enough to drive
/// [`classify_pairing_failure`] without leaking WHICH failure mode occurred
/// (see that function's doc for why only three buckets exist).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortOutcome {
    /// Nothing answered a TCP connect / WS upgrade on this port.
    NoUpgrade,
    /// The WS upgrade succeeded, but the connection ended BEFORE we ever sent
    /// our `auth` proof (an I/O error, a timeout, or a malformed/missing
    /// challenge). NOT evidence about our own token — issue #1084 PR1's own
    /// decision: "a crash between challenge and auth is not a pairing
    /// failure."
    PreAuthError,
    /// We sent `auth{proof}`, and the port failed to answer with a
    /// VERIFYING `auth.ok` — silence, a close, a malformed reply, or a
    /// `serverProof` that failed constant-time verification. Folded into one
    /// bucket because the server's own failed-proof path is, by design, a
    /// silent close indistinguishable from a crash (see
    /// `extension_bridge::advance_auth`'s doc) — once we have committed our
    /// proof, any non-verifying outcome is attributed to proof rejection.
    ProofRejected,
}

/// Drive the full handshake against one port. `Ok` only once the SERVER's
/// proof has verified (mutual auth complete); every other case reports
/// [`PortOutcome`] instead of the (now-dropped) socket.
async fn attempt_port(port: u16, token: &str) -> Result<WsStream, PortOutcome> {
    let tcp = TcpStream::connect(("127.0.0.1", port))
        .await
        .map_err(|_| PortOutcome::NoUpgrade)?;
    let uri = format!("ws://127.0.0.1:{port}/")
        .parse()
        .map_err(|_| PortOutcome::NoUpgrade)?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME_BYTES))
        .max_frame_size(Some(MAX_FRAME_BYTES));
    // Reuses the native-host's own sentinel Origin: this CLI is, like it, our
    // own process speaking the loopback protocol directly rather than a
    // browser extension — see `auth::is_allowed_origin`'s doc. The origin
    // check is defense-in-depth only; the mutual HMAC handshake below is the
    // real boundary.
    let request = ClientRequestBuilder::new(uri).with_header("Origin", NATIVE_HOST_ORIGIN);
    let (mut ws, _resp) = tokio_tungstenite::client_async_with_config(request, tcp, Some(config))
        .await
        .map_err(|_| PortOutcome::NoUpgrade)?;

    let client_nonce = handshake::new_nonce();
    if ws
        .send(Message::text(build_hello(&client_nonce)))
        .await
        .is_err()
    {
        return Err(PortOutcome::PreAuthError);
    }
    let server_nonce = next_json(&mut ws, HANDSHAKE_STEP_TIMEOUT)
        .await
        .and_then(|v| parse_challenge(&v))
        .ok_or(PortOutcome::PreAuthError)?;

    let proof = handshake::client_proof(token, &server_nonce, &client_nonce);
    if ws.send(Message::text(build_auth(&proof))).await.is_err() {
        // Delivery itself is unconfirmed — we never received anything that
        // could be a rejection signal, so this is NOT a proof rejection.
        return Err(PortOutcome::PreAuthError);
    }

    let server_proof = next_json(&mut ws, HANDSHAKE_STEP_TIMEOUT)
        .await
        .and_then(|v| parse_auth_ok(&v));
    match server_proof {
        Some(proof)
            if handshake::verify_server_proof(token, &server_nonce, &client_nonce, &proof) =>
        {
            Ok(ws)
        }
        _ => Err(PortOutcome::ProofRejected),
    }
}

/// Why every candidate port fell short of authenticating, folded into ONE
/// process-level verdict — see the exit-code table in the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PairingFailure {
    /// No port in [`PORT_RANGE`] answered a TCP connect/WS upgrade at all.
    AppNotRunning,
    /// Every port that upgraded also rejected our proof — the persisted
    /// pairing token is stale (this CLI process's copy, read fresh from the
    /// token file every invocation, no longer matches the app's).
    PairingRejected,
    /// At least one upgraded port failed BEFORE the proof-rejection point —
    /// inconclusive, and specifically NOT evidence the token is wrong (see
    /// [`PortOutcome::PreAuthError`]'s doc).
    ConnectionError,
}

/// Pure aggregation over this invocation's [`PortOutcome`]s — kept separate
/// from the async I/O in [`connect_authenticated`] so it is directly
/// unit-testable. Only counts ports that actually upgraded ("every port
/// completed an upgrade and every one rejected the proof" from a range where,
/// in practice, only ONE port is ever bound — the rest are simply absent).
fn classify_pairing_failure(outcomes: &[PortOutcome]) -> PairingFailure {
    let saw_upgrade = outcomes.iter().any(|o| *o != PortOutcome::NoUpgrade);
    let saw_pre_auth_error = outcomes.contains(&PortOutcome::PreAuthError);
    if !saw_upgrade {
        PairingFailure::AppNotRunning
    } else if saw_pre_auth_error {
        PairingFailure::ConnectionError
    } else {
        PairingFailure::PairingRejected
    }
}

/// For each port in [`PORT_RANGE`], drive the full handshake and accept the
/// first one whose server proof verifies. See the module doc for why this
/// must not reuse [`super::native_host::connect_bridge`].
async fn connect_authenticated(token: &str) -> Result<WsStream, PairingFailure> {
    let mut outcomes = Vec::new();
    for port in PORT_RANGE {
        match attempt_port(port, token).await {
            Ok(ws) => return Ok(ws),
            Err(outcome) => outcomes.push(outcome),
        }
    }
    Err(classify_pairing_failure(&outcomes))
}

// ── agent.query round trip ──────────────────────────────────────────────────

/// Send one `agent.query` and wait for its matching `agent.result` (by
/// `reqId`), within [`QUERY_REPLY_TIMEOUT`] overall. A `token.revoked` seen
/// instead (the pairing was rotated mid-session) is reported distinctly
/// rather than left to time out. Any other frame is ignored — a fresh,
/// one-shot connection should never see one, but ignoring rather than failing
/// on it costs nothing and is more robust to a future additive frame.
async fn send_agent_query(mut ws: WsStream, verb: &Verb) -> Result<Value, &'static str> {
    let req_id = uuid::Uuid::new_v4().to_string();
    let frame = json!({
        "type": msg::AGENT_QUERY,
        "reqId": req_id,
        "payload": verb.payload(),
    })
    .to_string();
    if ws.send(Message::text(frame)).await.is_err() {
        return Err("connection_lost");
    }

    let deadline = Instant::now() + QUERY_REPLY_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("timeout");
        }
        let Some(v) = next_json(&mut ws, remaining).await else {
            return Err("connection_lost");
        };
        match v.get("type").and_then(Value::as_str) {
            Some(t)
                if t == msg::AGENT_RESULT
                    && v.get("reqId").and_then(Value::as_str) == Some(req_id.as_str()) =>
            {
                return v.get("payload").cloned().ok_or("connection_lost");
            }
            Some(t) if t == msg::TOKEN_REVOKED => return Err("pairing_rejected"),
            _ => continue,
        }
    }
}

// ── entrypoint + output ─────────────────────────────────────────────────────

fn pairing_failure_sentinel(f: PairingFailure) -> &'static str {
    match f {
        PairingFailure::AppNotRunning => "app_not_running",
        PairingFailure::PairingRejected => "pairing_rejected",
        PairingFailure::ConnectionError => "connection_error",
    }
}

/// Print a synthesized CLI-level error (exit 2) and return that code. Never
/// echoes a path or a raw I/O error string — only fixed sentinels (see the
/// module doc's exit-code table).
fn emit_cli_error(resource: Option<&str>, sentinel: &str) -> i32 {
    println!(
        "{}",
        json!({ "ok": false, "resource": resource, "error": sentinel })
    );
    2
}

async fn run_verb(verb: Verb) -> i32 {
    let resource = verb.resource_name();

    let Some(pointer) = read_agent_pointer() else {
        return emit_cli_error(Some(resource), "app_not_running");
    };
    let Some(token) = read_pairing_token(&pointer.data_dir) else {
        return emit_cli_error(Some(resource), "app_not_running");
    };

    let ws = match connect_authenticated(&token).await {
        Ok(ws) => ws,
        Err(failure) => {
            return emit_cli_error(Some(resource), pairing_failure_sentinel(failure));
        }
    };

    match send_agent_query(ws, &verb).await {
        Ok(payload) => {
            let ok = payload.get("ok").and_then(Value::as_bool).unwrap_or(false);
            println!("{payload}");
            i32::from(!ok)
        }
        Err(sentinel) => emit_cli_error(Some(resource), sentinel),
    }
}

/// `ajh-tauri agent <verb>` entrypoint. `args` excludes the program name AND
/// the `agent` sentinel itself. Called from `lib::run_agent_cli_if_invoked`,
/// itself called from `main()` BELOW the native-host short-circuit and ABOVE
/// `ajh_tauri::run()` — see that function's doc for why the ordering matters.
/// Builds its OWN current-thread Tokio runtime (mirrors
/// [`super::native_host::run`]): this path runs before Tauri boots, so there
/// is no ambient reactor. Never panics out.
pub fn run(args: &[String]) -> i32 {
    crate::platform::windows_console::ensure_console_output();

    let verb = match parse_verb(args) {
        Ok(v) => v,
        Err(e) => {
            println!(
                "{}",
                json!({ "ok": false, "error": "usage", "detail": e.to_string() })
            );
            return 2;
        }
    };

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return emit_cli_error(Some(verb.resource_name()), "runtime_unavailable"),
    };
    rt.block_on(run_verb(verb))
}

#[cfg(test)]
mod tests {
    use super::*;
    // `advance_frame`/`ConnState`/`FrameDecision` are private to the parent
    // `extension_bridge` module — visible here because privacy in Rust
    // extends to every DESCENDANT module, not just direct children, so this
    // test module (a grandchild) can reach them exactly as
    // `extension_bridge::test` does one level up.
    use super::super::{advance_frame, BridgeState, ConnState, FrameDecision};

    // ── argv parsing ─────────────────────────────────────────────────────────

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn parses_best_matches_with_no_flags() {
        assert_eq!(
            parse_verb(&s(&["best-matches"])).unwrap(),
            Verb::BestMatches { limit: None }
        );
    }

    #[test]
    fn parses_best_matches_with_limit() {
        assert_eq!(
            parse_verb(&s(&["best-matches", "--limit", "5"])).unwrap(),
            Verb::BestMatches { limit: Some(5) }
        );
    }

    #[test]
    fn rejects_a_non_numeric_limit() {
        assert!(parse_verb(&s(&["best-matches", "--limit", "abc"])).is_err());
    }

    #[test]
    fn rejects_limit_missing_its_value() {
        assert!(parse_verb(&s(&["best-matches", "--limit"])).is_err());
    }

    #[test]
    fn parses_job_with_url() {
        assert_eq!(
            parse_verb(&s(&["job", "https://example.com/1"])).unwrap(),
            Verb::Job {
                url: "https://example.com/1".to_string()
            }
        );
    }

    #[test]
    fn rejects_job_without_a_url() {
        assert!(parse_verb(&s(&["job"])).is_err());
    }

    #[test]
    fn parses_the_three_no_arg_verbs() {
        assert_eq!(parse_verb(&s(&["profile"])).unwrap(), Verb::Profile);
        assert_eq!(parse_verb(&s(&["automations"])).unwrap(), Verb::Automations);
        assert_eq!(parse_verb(&s(&["schema"])).unwrap(), Verb::Schema);
    }

    #[test]
    fn rejects_an_unknown_verb() {
        assert!(parse_verb(&s(&["delete-everything"])).is_err());
    }

    #[test]
    fn rejects_a_missing_verb() {
        assert!(parse_verb(&s(&[])).is_err());
    }

    #[test]
    fn payload_carries_the_wire_resource_name() {
        assert_eq!(Verb::Schema.payload()["resource"], "schema");
        assert_eq!(
            Verb::Job {
                url: "https://x.example.com".to_string()
            }
            .payload()["url"],
            "https://x.example.com"
        );
        let with_limit = Verb::BestMatches { limit: Some(7) }.payload();
        assert_eq!(with_limit["limit"], 7);
        let without_limit = Verb::BestMatches { limit: None }.payload();
        assert!(without_limit.get("limit").is_none());
    }

    // ── pairing-failure classification (pure) ───────────────────────────────
    // Hand-written expected buckets, not derived from `classify_pairing_failure`
    // itself — mirrors the repo's standing lesson to pair a loop/derived check
    // with a literal.

    #[test]
    fn all_ports_absent_is_app_not_running() {
        assert_eq!(
            classify_pairing_failure(&[PortOutcome::NoUpgrade, PortOutcome::NoUpgrade]),
            PairingFailure::AppNotRunning
        );
        assert_eq!(classify_pairing_failure(&[]), PairingFailure::AppNotRunning);
    }

    #[test]
    fn every_reachable_port_rejecting_the_proof_is_pairing_rejected() {
        assert_eq!(
            classify_pairing_failure(&[PortOutcome::NoUpgrade, PortOutcome::ProofRejected]),
            PairingFailure::PairingRejected
        );
    }

    #[test]
    fn any_pre_auth_error_is_connection_error_not_pairing_rejected() {
        // Issue #1084 PR1's own decision: "a crash between challenge and auth
        // is not a pairing failure" — even alongside a genuine proof
        // rejection on another port, the mixed case must NOT be reported as
        // a wrong token.
        assert_eq!(
            classify_pairing_failure(&[PortOutcome::PreAuthError, PortOutcome::ProofRejected]),
            PairingFailure::ConnectionError
        );
        assert_eq!(
            classify_pairing_failure(&[PortOutcome::PreAuthError]),
            PairingFailure::ConnectionError
        );
    }

    // ── handshake wire-shape round trip against the REAL server state
    // machine (`super::advance_frame`) — no socket, no AppHandle needed:
    // `advance_hello`/`advance_auth` are pure functions of `&BridgeState`.
    // This is the proof the client's frame-building/parsing is wire-compatible
    // with the committed server half, not just internally self-consistent. ──

    #[test]
    fn handshake_round_trips_against_the_real_server_state_machine() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = BridgeState::load(dir.path());
        let token = state.token();

        let client_nonce = handshake::new_nonce();
        let hello = build_hello(&client_nonce);

        let decision = advance_frame(&state, &ConnState::AwaitingHello, &hello);
        let FrameDecision::Challenge { reply, next } = decision else {
            panic!("expected Challenge, got {decision:?}");
        };
        let challenge_json: Value = serde_json::from_str(&reply).unwrap();
        let server_nonce = parse_challenge(&challenge_json).expect("well-formed challenge");

        let proof = handshake::client_proof(&token, &server_nonce, &client_nonce);
        let auth = build_auth(&proof);

        let decision = advance_frame(&state, &next, &auth);
        let FrameDecision::AuthOk(reply) = decision else {
            panic!("expected AuthOk, got {decision:?}");
        };
        let auth_ok_json: Value = serde_json::from_str(&reply).unwrap();
        let server_proof = parse_auth_ok(&auth_ok_json).expect("well-formed auth.ok");

        assert!(
            handshake::verify_server_proof(&token, &server_nonce, &client_nonce, &server_proof),
            "the client's own verification must accept the real server's serverProof"
        );
    }

    #[test]
    fn handshake_round_trip_rejects_a_wrong_token() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = BridgeState::load(dir.path());

        let client_nonce = handshake::new_nonce();
        let hello = build_hello(&client_nonce);
        let decision = advance_frame(&state, &ConnState::AwaitingHello, &hello);
        let FrameDecision::Challenge { reply, next } = decision else {
            panic!("expected Challenge, got {decision:?}");
        };
        let server_nonce =
            parse_challenge(&serde_json::from_str(&reply).unwrap()).expect("well-formed challenge");

        // A wrong token — the CLI's persisted copy is stale.
        let wrong_proof =
            handshake::client_proof("not-the-real-token", &server_nonce, &client_nonce);
        let auth = build_auth(&wrong_proof);
        let decision = advance_frame(&state, &next, &auth);
        assert!(
            matches!(decision, FrameDecision::Unauthorized),
            "expected Unauthorized, got {decision:?}"
        );
    }

    // ── the SAME round trip, but over a REAL loopback socket, driving the
    // production `attempt_port` fn (not a reimplementation) against a
    // minimal server that itself calls the real `advance_frame` state
    // machine — the strongest available proof that the client's transport
    // code (WS upgrade, frame send/receive) interoperates with the actual
    // server, not just that the JSON shapes match in-process. ──

    #[tokio::test]
    async fn attempt_port_authenticates_over_a_real_socket_against_the_real_server() {
        use tokio::net::TcpListener;

        let dir = tempfile::TempDir::new().unwrap();
        let state = BridgeState::load(dir.path());
        let token = state.token();

        // Kernel-assigned ephemeral port (never collides with a real running
        // app or another test) — same hermetic pattern as
        // `import_tests::claim_busy_port`.
        let listener = TcpListener::bind(("127.0.0.1", 0u16)).await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            // No Origin check here (that gate is `handle_connection`'s own,
            // covered by `auth`'s tests) — everything past the WS upgrade is
            // the real per-frame `advance_frame` dispatch `handle_connection`
            // itself runs.
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let mut conn = ConnState::AwaitingHello;
            loop {
                let msg = ws.next().await.unwrap().unwrap();
                let text = match msg {
                    tokio_tungstenite::tungstenite::Message::Text(t) => t.to_string(),
                    other => panic!("expected a text frame, got {other:?}"),
                };
                match advance_frame(&state, &conn, &text) {
                    FrameDecision::Challenge { reply, next } => {
                        conn = next;
                        ws.send(tokio_tungstenite::tungstenite::Message::text(reply))
                            .await
                            .unwrap();
                    }
                    FrameDecision::AuthOk(reply) => {
                        ws.send(tokio_tungstenite::tungstenite::Message::text(reply))
                            .await
                            .unwrap();
                        break;
                    }
                    other => panic!("unexpected FrameDecision in test server: {other:?}"),
                }
            }
        });

        let result = attempt_port(port, &token).await;
        assert!(
            result.is_ok(),
            "attempt_port must authenticate against the real server over a real socket, got {:?}",
            result.err()
        );
        server.await.unwrap();
    }

    // ── pointer + token file reads (pure fs, no env mutation needed here —
    // the path itself is exercised by platform::config's own tests) ────────

    #[test]
    fn read_pairing_token_trims_and_rejects_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join(TOKEN_FILE), "  abc123  \n").unwrap();
        assert_eq!(
            read_pairing_token(dir.path().to_str().unwrap()),
            Some("abc123".to_string())
        );

        let empty_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(empty_dir.path().join(TOKEN_FILE), "   \n").unwrap();
        assert_eq!(read_pairing_token(empty_dir.path().to_str().unwrap()), None);

        let missing_dir = tempfile::TempDir::new().unwrap();
        assert_eq!(
            read_pairing_token(missing_dir.path().to_str().unwrap()),
            None
        );
    }
}
