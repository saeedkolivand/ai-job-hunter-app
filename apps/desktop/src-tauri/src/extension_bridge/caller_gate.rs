//! Caller-class gate — `CallerClass` (a handshake-`Origin` label) plus the per-verb dispatch
//! matrix [`advance_authenticated`] that gates on it. Split out of `mod.rs` (R8 relief, PR1 —
//! extension read tier) — same pattern as the `autotrack.rs`/`status_update.rs`/`settings.rs`
//! splits: this pair is cohesive enough (the label + the ONE function that reads it) to own its
//! own module without changing any behavior. `pub(super)` throughout: visible everywhere in
//! `extension_bridge` (mod.rs re-exports both names so the rest of the tree keeps referring to
//! them unqualified — see `mod.rs`'s `use self::caller_gate::...`), never further.

use serde_json::Value;

use crate::error::AppError;

use super::{
    agent_call, agent_read, auth, document_export, import_flow, msg, settings, BridgeState,
    FrameDecision,
};

/// The caller class a handshake `Origin` resolves to (PR1, extension read tier — extends the
/// bare `is_agent_cli` bool finding #5's security review introduced). Resolved ONCE by
/// [`handle_connection`](super::handle_connection), the same point that used to resolve
/// `is_agent_cli` alone, and threaded through `advance_frame_from`/[`advance_authenticated`] to
/// route `agent.query`/`agent.call` to the correct tier and the new `settings.get`/`settings.set`
/// verbs to their allowed callers.
///
/// A LABEL, not a boundary — same caveat as [`auth::AGENT_CLI_ORIGIN`]'s own doc: the real
/// authentication is the v2 mutual HMAC handshake. This stops a non-colluding case (a future
/// extension bug, a compromised update) from reaching a surface it was never meant to.
///
/// A frame relayed through the native-messaging host ([`super::native_host`] — the documented
/// Firefox HTTPS-Only-Mode fallback) resolves to [`CallerClass::Extension`]: the host forwards the
/// SAME paired extension's frames 1:1 and has no origin of its own, so [`auth::is_extension_origin`]
/// carves out only the CLI sentinel, not the relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CallerClass {
    /// Handshake `Origin` matched [`auth::AGENT_CLI_ORIGIN`].
    Cli,
    /// Handshake `Origin` matched [`auth::is_extension_origin`] — the paired browser extension.
    Extension,
    /// Neither — every `agent.query`/`agent.call`/`settings.*` frame is refused.
    Other,
}

impl CallerClass {
    /// Resolve from a handshake `Origin`, checked in the same order
    /// [`auth::is_allowed_origin`] itself checks (dev override folds into the extension case, via
    /// [`auth::is_extension_origin`]). Pure — no I/O, directly unit-testable.
    pub(super) fn resolve(origin: &str, dev_origins: &[String]) -> Self {
        let origin = origin.trim();
        if origin == auth::AGENT_CLI_ORIGIN {
            CallerClass::Cli
        } else if auth::is_extension_origin(origin, dev_origins) {
            CallerClass::Extension
        } else {
            CallerClass::Other
        }
    }
}

/// Post-auth dispatch: the socket is session-authenticated, so frames carry no
/// token. Routes `import.request` / `profile.get` / `applied.check` /
/// `status.update` / `answers.save` / `answers.suggest` / `match.live` /
/// `answer.assist` / `assist.cancel` unconditionally; `caller` (a
/// [`CallerClass`], resolved once at handshake time — finding #5, security
/// review; extended in PR1) gates `agent.query` / `agent.call` (CLI
/// unconditionally, the extension Read-only + Autofill-gated + reply-capped,
/// see `msg::AGENT_QUERY`'s doc) and `settings.get` / `settings.set`
/// (extension only). An unknown type gets an `import.result` error reply
/// (never a panic). `state` is read PURELY here (the Autofill opt-in +
/// `agent_call`'s policy table are both in-memory/`'static` reads, no I/O).
pub(super) fn advance_authenticated(
    state: &BridgeState,
    kind: &str,
    req_id: String,
    envelope: &Value,
    caller: CallerClass,
) -> FrameDecision {
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
        // The read-only agent surface (issue #1084 PR 1; extension read tier, PR1).
        // `caller` is a spoofable label, not a boundary (the HMAC handshake
        // is); it stops a non-colluding case (a future extension bug, a
        // compromised update) from reaching a tier it was never meant to.
        msg::AGENT_QUERY => match caller {
            CallerClass::Other => FrameDecision::Reply(agent_read::origin_refused_reply(
                &req_id,
                envelope.get("payload").unwrap_or(&Value::Null),
            )),
            CallerClass::Cli => {
                // CLI behaviour is byte-for-byte unchanged — the existing tests are the proof.
                let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
                FrameDecision::AgentQuery {
                    req_id,
                    payload,
                    caller,
                }
            }
            CallerClass::Extension => {
                let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
                if state.autofill_enabled() {
                    FrameDecision::AgentQuery {
                        req_id,
                        payload,
                        caller,
                    }
                } else {
                    FrameDecision::Reply(agent_read::extension_gate_reply(&req_id, &payload))
                }
            }
        },
        // ADR-038 §2's generic tier (extension read tier, PR1 decision 1: Read-only for the
        // extension — checked PURELY off the policy table, before any I/O, via
        // `agent_call::extension_may_dispatch`).
        msg::AGENT_CALL => match caller {
            CallerClass::Other => FrameDecision::Reply(agent_call::origin_refused_reply(
                &req_id,
                envelope.get("payload").unwrap_or(&Value::Null),
            )),
            CallerClass::Cli => {
                let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
                FrameDecision::AgentCall {
                    req_id,
                    payload,
                    caller,
                }
            }
            CallerClass::Extension => {
                let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
                if !state.autofill_enabled() {
                    FrameDecision::Reply(agent_call::extension_gate_reply(&req_id, &payload))
                } else if agent_call::extension_may_dispatch(&payload) {
                    FrameDecision::AgentCall {
                        req_id,
                        payload,
                        caller,
                    }
                } else {
                    FrameDecision::Reply(agent_call::effect_not_allowed_reply(&req_id, &payload))
                }
            }
        },
        // `settings.get`/`settings.set` (R7) — extension caller ONLY, regardless of the Autofill
        // gate (this is how the user turns it on). Refused for the CLI (it already has the
        // Reversible rows for these opt-ins via `agent.call`) and any other caller.
        msg::SETTINGS_GET => match caller {
            CallerClass::Extension => FrameDecision::SettingsGet { req_id },
            CallerClass::Cli | CallerClass::Other => {
                FrameDecision::Reply(settings::origin_refused_reply(&req_id))
            }
        },
        msg::SETTINGS_SET => match caller {
            CallerClass::Extension => {
                let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
                FrameDecision::SettingsSet { req_id, payload }
            }
            CallerClass::Cli | CallerClass::Other => {
                FrameDecision::Reply(settings::origin_refused_reply(&req_id))
            }
        },
        // `document.export` (PR2 — documents into ATS) — extension caller ONLY (unlike
        // `agent.query`/`agent.call`, the CLI never reaches this verb: it already has
        // `documents:documents_export_document` via `agent.call`). Assisted-autofill checked HERE,
        // same shape as `AGENT_QUERY`'s own extension arm — exporting a résumé/cover-letter out of
        // the app is the same consent class as handing it to a form field.
        msg::DOCUMENT_EXPORT => match caller {
            CallerClass::Extension => {
                let payload = envelope.get("payload").cloned().unwrap_or(Value::Null);
                if state.autofill_enabled() {
                    FrameDecision::DocumentExport { req_id, payload }
                } else {
                    FrameDecision::Reply(document_export::extension_gate_reply(&req_id))
                }
            }
            CallerClass::Cli | CallerClass::Other => {
                FrameDecision::Reply(document_export::origin_refused_reply(&req_id))
            }
        },
        // Unknown message types — acknowledged as an error, never panic.
        other => FrameDecision::Reply(import_flow::result_reply(
            &req_id,
            Err(AppError::Validation(format!(
                "unknown message type '{other}'"
            ))),
        )),
    }
}
