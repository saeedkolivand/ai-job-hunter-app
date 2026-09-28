//! The per-frame APPLICATION dispatch — every `FrameDecision` variant except the handshake-only
//! ones (which mutate `handle_connection`'s own loop state and so stay inline there). Split from
//! `mod.rs` (R8 relief; pure code motion — every match arm body is unchanged, only its
//! `app`/`state`/`out_tx`/`assist_streams`/`agent_query_cancel` references are now through
//! this function's OWN parameters rather than `handle_connection`'s locals). See `connection`'s
//! own module doc for the full per-connection design.

use std::sync::Arc;

use serde_json::Value;
use tauri::AppHandle;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use super::assist_registry::AssistStreamRegistry;
use super::connection::export_reply_unless_revoked;
use super::frame::FrameDecision;
use super::handle_profile;
use super::{
    agent_call, agent_read, answers_save, answers_suggest, applied_check, applied_check_batch,
    autofill_check, autotrack, document_export, import_flow, match_live, settings, settings_set,
    status_update, stream, BridgeState,
};

/// Dispatch one authenticated, non-handshake `FrameDecision` and return its ready-to-send reply
/// (`None` for a spawned-off-the-loop verb, which sends its own reply later through `out_tx`).
/// `handle_connection`'s own match handles `CloseOverCap`/`Drop`/`Outdated`/`Unauthorized`/
/// `Challenge`/`AuthOk`/`Reply` inline (they mutate `conn`/`authenticated`/`counted_epoch` or
/// `break` the loop) and forwards every other variant here.
pub(super) async fn dispatch_frame(
    decision: FrameDecision,
    app: &AppHandle,
    state: &BridgeState,
    out_tx: &UnboundedSender<Message>,
    assist_streams: &Arc<AssistStreamRegistry>,
    agent_query_cancel: &CancellationToken,
) -> Option<String> {
    match decision {
        FrameDecision::CloseOverCap
        | FrameDecision::Drop
        | FrameDecision::Outdated(_)
        | FrameDecision::Unauthorized
        | FrameDecision::Challenge { .. }
        | FrameDecision::AuthOk(_)
        | FrameDecision::Reply(_) => unreachable!(
            "handshake-only FrameDecision variants are handled by handle_connection's own loop \
             before reaching dispatch_frame"
        ),
        FrameDecision::Import { req_id, payload } => {
            let outcome = import_flow::handle_import(app, payload).await;
            Some(import_flow::result_reply(&req_id, outcome))
        }
        FrameDecision::Profile { req_id } => Some(handle_profile(app, &req_id)),
        FrameDecision::AppliedCheck { req_id, payload } => {
            Some(applied_check::handle_applied_check(app, &req_id, &payload))
        }
        FrameDecision::StatusUpdate { req_id, payload } => {
            Some(status_update::handle_status_update(app, &req_id, &payload))
        }
        FrameDecision::AutotrackCheck { req_id } => Some(autotrack::autotrack_result_reply(
            &req_id,
            state.autotrack_enabled(),
        )),
        FrameDecision::AutofillCheck { req_id } => Some(
            autofill_check::autofill_check_result_reply(&req_id, state.autofill_enabled()),
        ),
        FrameDecision::AnswersSave { req_id, payload } => {
            Some(answers_save::handle_answers_save(app, &req_id, &payload))
        }
        FrameDecision::AnswersSuggest { req_id, payload } => Some(
            answers_suggest::handle_answers_suggest(app, &req_id, &payload),
        ),
        FrameDecision::MatchLive { req_id, payload } if state.try_acquire_match_live() => {
            Some(match_live::handle_match_live(app, &req_id, &payload).await)
        }
        FrameDecision::MatchLive { req_id, .. } => Some(match_live::throttled_reply(&req_id)),
        FrameDecision::AgentQuery {
            req_id,
            payload,
            caller,
        } if state.try_acquire_agent(agent_read::resource_name(&payload)) => {
            // Spawned (mirrors `AnswerAssist` below — the HIGH fix this
            // finding reuses): `best-matches` can run multi-second, and
            // awaiting it inline here would stall THIS loop's
            // `reader.next()` — including its own `token.revoked`
            // observation, so an in-flight read could complete on an
            // already-revoked token. See `stream::spawn_agent_query`.
            // `caller` (PR1) decides ONLY whether the extension's own
            // smaller reply cap applies — the CLI's own dispatch is
            // unchanged; see `agent_read::extension_capped_reply`.
            stream::spawn_agent_query(
                app.clone(),
                req_id,
                payload,
                out_tx.clone(),
                agent_query_cancel.clone(),
                caller,
            );
            None
        }
        FrameDecision::AgentQuery {
            req_id, payload, ..
        } => {
            let retry_after_ms = state.agent_retry_after_ms(agent_read::resource_name(&payload));
            Some(agent_read::throttled_reply(
                &req_id,
                &payload,
                retry_after_ms,
            ))
        }
        // ADR-038 §2 — same spawn-off-the-read-loop + shared-throttle
        // reasoning as AgentQuery above; see `stream::spawn_agent_call`
        // and `agent_call::throttle_key`'s own docs. `caller` (PR1) —
        // same reasoning as AgentQuery above.
        FrameDecision::AgentCall {
            req_id,
            payload,
            caller,
        } if state.try_acquire_agent(agent_call::throttle_key(
            payload.get("command").and_then(Value::as_str).unwrap_or(""),
        )) =>
        {
            stream::spawn_agent_call(
                app.clone(),
                req_id,
                payload,
                out_tx.clone(),
                agent_query_cancel.clone(),
                caller,
            );
            None
        }
        // `throttled_reply` (like `agent_call::origin_refused_reply`)
        // echoes the caller's own `reqId`/`namespace`/`command`, and this
        // reply goes STRAIGHT to the socket without passing through
        // `agent_call::enforce_frame_cap` — so those identifiers are
        // clamped inside `agent_call::refusal_reply`, which is the only
        // thing bounding this frame. See `REFUSAL_IDENT_CAP`.
        FrameDecision::AgentCall {
            req_id, payload, ..
        } => {
            let command = payload.get("command").and_then(Value::as_str).unwrap_or("");
            let retry_after_ms = state.agent_retry_after_ms(agent_call::throttle_key(command));
            Some(agent_call::throttled_reply(
                &req_id,
                &payload,
                retry_after_ms,
            ))
        }
        // `settings.get`/`settings.set` (R7) — extension caller only, gated in
        // `advance_authenticated`. `settings.get` is unthrottled (a pure read of the user's
        // own device-local settings); `settings.set` shares the per-pairing throttle
        // discipline every write verb on this bridge uses.
        FrameDecision::SettingsGet { req_id } => {
            Some(settings::handle_settings_get(&req_id, state))
        }
        FrameDecision::SettingsSet { req_id, payload } if state.try_acquire_settings_set() => Some(
            settings_set::handle_settings_set(app, &req_id, state, &payload),
        ),
        FrameDecision::SettingsSet { req_id, .. } => Some(settings::throttled_reply(&req_id)),
        // `document.export` (PR2 — documents into ATS): extension caller + Assisted-autofill
        // gate already checked in `caller_gate::advance_authenticated`; only the throttle is
        // decided here, same shape as `SettingsSet` just above. Awaited inline (not spawned
        // off the read loop) — a Typst compile is bounded (100-400ms per
        // `documents_export_document`'s own doc), the same class of cost `Import`/`MatchLive`
        // already await inline here. `export_reply_unless_revoked` guards the inline await:
        // this loop cannot poll `revoked_rx` while it is suspended here, so a rotation that
        // lands mid-compile must not still hand the finished document to the now-revoked
        // socket — see that function's doc.
        FrameDecision::DocumentExport { req_id, payload }
            if state.try_acquire_document_export() =>
        {
            export_reply_unless_revoked(
                state,
                document_export::handle_document_export(app, &req_id, &payload),
            )
            .await
        }
        FrameDecision::DocumentExport { req_id, .. } => Some(document_export::throttled_reply(
            &req_id,
            state.document_export_retry_after_ms(),
        )),
        // `applied.check.batch` (PR3): caller-unconditional like `AppliedCheck`; only the
        // throttle is decided here — same shape as `DocumentExport`/`SettingsSet` above.
        FrameDecision::AppliedCheckBatch { req_id, payload }
            if state.try_acquire_applied_check_batch() =>
        {
            Some(applied_check_batch::handle_applied_check_batch(
                app, &req_id, &payload,
            ))
        }
        FrameDecision::AppliedCheckBatch { req_id, .. } => {
            Some(applied_check_batch::throttled_reply(
                &req_id,
                state.applied_check_batch_retry_after_ms(),
            ))
        }
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
                std::sync::Arc::clone(assist_streams),
            );
            None
        }
        FrameDecision::AssistCancel { req_id } => {
            assist_streams.cancel(app, &req_id);
            None
        }
    }
}

#[cfg(test)]
mod tests;
