//! [`FrameDecision`] — the outcome of the per-frame handshake/dispatch decision — plus
//! [`ConnState`] and the handshake-advancement functions that PRODUCE one (R8 relief, PR4: this
//! module already existed for `FrameDecision`, split out of `mod.rs` in PR2; the PR4 Prep tab +
//! save-answers-on-submit additions pushed `mod.rs` back to the hard LOC cap with only 2 lines of
//! headroom, so the connection-state-machine block — `ConnState`, `advance_frame_from`,
//! `advance_frame`, `advance_hello`, `advance_auth`, `challenge_reply`, `auth_ok_reply`,
//! `update_required_reply` — moves here too, behaviourally identical). `pub(super)` throughout so
//! `mod.rs` (which still owns every CALL site — `handle_connection`'s dispatch, the `BridgeState`
//! token/`MAX_FRAME_BYTES`/`MAX_REQ_ID_BYTES` these functions read) keeps resolving every name
//! unqualified via its own `use self::frame::{...}` re-export, and `test.rs`/`caller_gate.rs`
//! (siblings, not descendants of this module) keep reaching them through THAT re-export exactly as
//! before — the same re-export shape `caller_gate`/`CallerClass` already established.

use serde_json::{json, Value};

use super::{
    caller_gate::advance_authenticated,
    handshake, msg,
    req_id_cap::{oversized_req_id_reply, MAX_REQ_ID_BYTES},
    BridgeState, CallerClass, MAX_FRAME_BYTES, PROTOCOL_VERSION,
};

/// Outcome of the per-frame handshake/dispatch decision, isolated from any
/// `AppHandle` so the size gate + handshake state machine are unit-testable. The
/// connection loop runs the (async, app-stateful) import only for
/// [`FrameDecision::Import`]; every other variant is resolved here from pure
/// inputs (+ the token off [`super::BridgeState`] for the constant-time proof check).
#[cfg_attr(test, derive(Debug))]
pub(super) enum FrameDecision {
    /// Frame exceeds [`super::MAX_FRAME_BYTES`] — close the socket without parsing.
    CloseOverCap,
    /// Not JSON, or an ignorable frame — drop silently, no reply, stay in state.
    Drop,
    /// The first frame was not a valid protocol-2 `hello` (a legacy `{type:'auth',
    /// token}` frame, a missing/older protocol): send this ready-to-send
    /// [`super::msg::UPDATE_REQUIRED`] reply, then CLOSE. Force cutover — no dual path.
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
    /// [`super::import_flow::handle_import`].
    Import { req_id: String, payload: Value },
    /// An authenticated `profile.get` to answer through [`super::handle_profile`]. Carries
    /// no payload — the reply is gated on the autofill opt-in, not on any input.
    Profile { req_id: String },
    /// An authenticated `applied.check` to answer through
    /// [`super::applied_check::handle_applied_check`]. Carries the payload verbatim so
    /// the handler can read `url`. Read-only by construction: resolved from the
    /// local `ApplicationStore` only — never the network.
    AppliedCheck { req_id: String, payload: Value },
    /// An authenticated `status.update` to answer through
    /// [`super::status_update::handle_status_update`]. Carries the payload verbatim so
    /// the handler can read `url` + `to`. The ONLY write this dispatch can
    /// route to besides `Import`;
    /// [`super::status_update::resolve_status_update`] is what actually restricts it
    /// to `saved → applied` on an exact match.
    StatusUpdate { req_id: String, payload: Value },
    /// An authenticated `autotrack.check` (Task #22) — a pure read of the
    /// auto-track opt-in off [`super::BridgeState`]. No payload; the loop answers it
    /// with `autotrack::autotrack_result_reply`.
    AutotrackCheck { req_id: String },
    /// An authenticated `autofill.check` (Task #30) — a pure read of the
    /// assisted-autofill opt-in off [`super::BridgeState`]. Mirrors
    /// [`FrameDecision::AutotrackCheck`] exactly. No payload; the loop
    /// answers it with `autofill_check::autofill_check_result_reply`.
    AutofillCheck { req_id: String },
    /// An authenticated `answers.save` to answer through
    /// [`super::answers_save::handle_answers_save`]. Carries the payload verbatim so
    /// the handler can read `url` + `answers`.
    AnswersSave { req_id: String, payload: Value },
    /// An authenticated `answers.suggest` to answer through
    /// [`super::answers_suggest::handle_answers_suggest`]. Carries the payload
    /// verbatim so the handler can read `questions`.
    AnswersSuggest { req_id: String, payload: Value },
    /// An authenticated `match.live` to answer through
    /// [`super::match_live::handle_match_live`]. Carries the payload verbatim so the
    /// handler can read `url` + `html`.
    MatchLive { req_id: String, payload: Value },
    /// An authenticated `answer.assist` to answer through
    /// [`super::answer_assist::handle_answer_assist`]. Carries the payload verbatim
    /// so the handler can read `question` + `url` + `searchWeb`.
    AnswerAssist { req_id: String, payload: Value },
    /// An authenticated `assist.cancel` — cancel the in-flight stream named
    /// by `req_id` on THIS connection's own
    /// [`super::stream::AssistStreamRegistry`]. No reply is ever sent for this frame.
    AssistCancel { req_id: String },
    /// An authenticated `agent.query` (issue #1084 PR 1) to answer through
    /// [`super::agent_read::handle_agent_query`]. Carries the payload verbatim so
    /// the handler can read `resource` (+ `url`/`limit`), and the resolved
    /// `caller` (PR1) so the dispatch loop knows whether to apply the
    /// extension's own smaller reply cap.
    AgentQuery {
        req_id: String,
        payload: Value,
        caller: CallerClass,
    },
    /// An authenticated `agent.call` (ADR-038 §2, Phase 2) to answer through
    /// [`super::agent_call::handle_agent_call`]. Carries the payload verbatim so the
    /// handler can read `namespace`/`command`/`input`, and the resolved
    /// `caller` (PR1) — same reasoning as [`FrameDecision::AgentQuery`].
    AgentCall {
        req_id: String,
        payload: Value,
        caller: CallerClass,
    },
    /// An authenticated `settings.get` (R7) — extension caller only, answered through
    /// [`super::settings::handle_settings_get`]. No payload.
    SettingsGet { req_id: String },
    /// An authenticated `settings.set` (R7) — extension caller only, answered through
    /// [`super::settings::handle_settings_set`]. Carries the payload verbatim so the handler can
    /// read `key`/`enabled`.
    SettingsSet { req_id: String, payload: Value },
    /// An authenticated `document.export` (PR2 — documents into ATS) to answer through
    /// [`super::document_export::handle_document_export`]. Extension caller only (gated in
    /// `caller_gate::advance_authenticated`, mirroring `SettingsGet`/`SettingsSet`); carries the
    /// payload verbatim so the handler can read `source`/`kind`/`format`/`templateId`/
    /// `letterLayoutId`/`atsMode`.
    DocumentExport { req_id: String, payload: Value },
    /// An authenticated `applied.check.batch` (PR3 — Check-fit on the page) to answer through
    /// [`super::applied_check_batch::handle_applied_check_batch`]. Carries the payload verbatim so
    /// the handler can read `urls`. Same caller posture as [`FrameDecision::AppliedCheck`]
    /// (unconditional — see `applied_check_batch`'s module doc) — the throttle admission is
    /// decided at the dispatch site, not here.
    AppliedCheckBatch { req_id: String, payload: Value },
}

/// Per-connection handshake state. A socket starts `AwaitingHello`; a valid
/// protocol-2 `hello` moves it to `AwaitingAuth` (holding the two fresh nonces);
/// a **verified** client proof moves it to `Authenticated`. Only in
/// `Authenticated` are `import.request` / `profile.get` frames honored — the
/// socket is session-authenticated, so those frames carry no token.
#[cfg_attr(test, derive(Debug, Clone, PartialEq, Eq))]
pub(super) enum ConnState {
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

/// The per-message handshake gate + dispatch routing (size cap → JSON parse →
/// state-machine step) — everything that does NOT need an `AppHandle`. Pure
/// aside from reading the pairing token off [`BridgeState`] for the
/// constant-time proof check; the loop performs the I/O and the app-stateful
/// import/profile work. See [`ConnState`] for the state transitions.
///
/// `caller` is THIS connection's own resolved `CallerClass`, resolved once
/// by `handle_connection` (finding #5, security review; extended in PR1) —
/// never re-derived here, since only the WS handshake ever sees the raw
/// header. Production (`handle_connection`) calls this directly; every
/// EXISTING test in this crate exercises extension-origin traffic and goes
/// through [`advance_frame`] below instead, so none of them had to learn a
/// new parameter.
pub(super) fn advance_frame_from(
    state: &BridgeState,
    conn: &ConnState,
    text: &str,
    caller: CallerClass,
) -> FrameDecision {
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

    // Bound BEFORE type dispatch, every verb + handshake state (see `req_id_cap`'s doc).
    if req_id.len() > MAX_REQ_ID_BYTES {
        return FrameDecision::Reply(oversized_req_id_reply());
    }

    match conn {
        ConnState::AwaitingHello => advance_hello(kind, &req_id, payload),
        ConnState::AwaitingAuth {
            server_nonce,
            client_nonce,
        } => advance_auth(state, kind, &req_id, payload, server_nonce, client_nonce),
        ConnState::Authenticated => advance_authenticated(state, kind, req_id, &envelope, caller),
    }
}

/// [`advance_frame_from`] with `caller: CallerClass::Other` — extension-origin
/// traffic that is NOT the paired extension's own caller class, the shape
/// every PRE-EXISTING test in this crate already exercises.
#[cfg(test)]
pub(super) fn advance_frame(state: &BridgeState, conn: &ConnState, text: &str) -> FrameDecision {
    advance_frame_from(state, conn, text, CallerClass::Other)
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
