//! [`FrameDecision`] — the outcome of the per-frame handshake/dispatch decision. Split out of
//! `mod.rs` (R8 relief, PR2 — documents-into-ATS's `document.export` addition pushed that module
//! to the hard LOC cap) — same pattern as `caller_gate.rs`/`autofill_profile.rs`/`settings.rs`:
//! behaviourally identical, only the file it lives in moved. `pub(super)` so `mod.rs` (which still
//! owns every reference to this type — the `handle_connection` match, `advance_frame_from`'s
//! return type, `advance_authenticated`'s return type) and `caller_gate.rs` (a SIBLING module, not
//! a descendant of this one) both keep resolving `FrameDecision`/`super::FrameDecision`
//! unqualified, via `mod.rs`'s own `use self::frame::FrameDecision;` re-export — the exact
//! re-export shape `caller_gate`/`CallerClass` already established.

use serde_json::Value;

use super::CallerClass;

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
    Challenge {
        reply: String,
        next: super::ConnState,
    },
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
}
