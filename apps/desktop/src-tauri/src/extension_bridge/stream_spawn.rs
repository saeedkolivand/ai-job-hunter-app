//! Off-the-read-loop task spawning — split from `stream.rs` (R8 relief): `answer.assist`
//! streaming, `agent.query`, and `agent.call` all run on their OWN task so a multi-second
//! handler never blocks `handle_connection`'s `reader.next()`. See `stream`'s own module doc
//! for the whole streaming design and why this decoupling exists.

use std::sync::Arc;

use serde_json::Value;
use tauri::AppHandle;
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use super::assist_registry::AssistStreamRegistry;
use super::stream::ChannelFrameSink;
use crate::error::AppError;

/// Drive a streaming `answer.assist` request on its OWN task, decoupled from
/// the connection's read loop — see the module doc. The read loop's dispatch
/// for `FrameDecision::AnswerAssist` calls this and moves on immediately (no
/// reply is returned inline for the normal path); the terminal
/// `answer.assist.result` is sent from INSIDE the spawned task once
/// `handle_answer_assist` resolves, through the SAME channel as every
/// `assist.chunk`/`assist.done` it already sent, so per-`reqId` ordering
/// (chunks, then done, then result) holds.
///
/// [`AssistStreamRegistry::begin`] is called HERE, synchronously, on the read
/// loop's own thread — BEFORE `tokio::spawn` — rather than inside the spawned
/// task (where it used to live, in `resolve_answer_assist`). `tokio::spawn`
/// only SCHEDULES the task; it does not run it. Left inside the task, the
/// single-threaded read loop could immediately read the NEXT frame — an
/// `assist.cancel` for this very `reqId` — and dispatch it to
/// `AssistStreamRegistry::cancel` (also synchronous) before the spawned task
/// ever got scheduled to run its own `begin`. `cancel` would then find no
/// entry at all, silently drop the cancel, and the task would go on to run
/// `begin`→register→`job_start` regardless, billing a request the client
/// already gave up on. Calling `begin` here closes that scheduling gap: the
/// `Pending` marker exists before this function returns, so a same-connection
/// `assist.cancel` dispatched anywhere after this call is guaranteed to see
/// it. A duplicate `reqId` (one already `Pending`/`Running`/`CancelledEarly`
/// on this connection) is rejected right here with its own
/// `answer.assist.result` error reply — the task is never spawned at all.
pub(super) fn spawn_answer_assist(
    app: AppHandle,
    req_id: String,
    payload: Value,
    out_tx: UnboundedSender<Message>,
    registry: Arc<AssistStreamRegistry>,
) {
    let Some(r#gen) = begin_or_reject_duplicate(&registry, &req_id, &out_tx) else {
        return;
    };
    tokio::spawn(async move {
        let mut sink = ChannelFrameSink(out_tx.clone());
        let reply = super::answer_assist::handle_answer_assist(
            &app, &req_id, r#gen, &payload, &registry, &mut sink,
        )
        .await;
        let _ = out_tx.send(Message::text(reply));
    });
}

/// Drive an `agent.query` on its OWN task, decoupled from the connection's
/// read loop (finding #4, security review — the same reason
/// [`spawn_answer_assist`] exists above): `best-matches` can run
/// multi-second (see `agent_read`'s throttle doc), and awaiting it INLINE in
/// the read loop would stall `reader.next()` — including this connection's
/// own `token.revoked` observation, so an in-flight read could complete on
/// an already-revoked token. Unlike `answer.assist`, the reply here is a
/// single non-streamed `agent.result` frame — no chunking, no per-connection
/// registry — so this is a plain spawn-and-reply.
///
/// `cancel` is `handle_connection`'s own per-connection [`CancellationToken`]
/// (MAJOR fix — security review round 2), cancelled once at every teardown
/// path — a token revocation, but also a normal close, a read error, or a
/// stalled writer — the same single call site that already runs
/// `AssistStreamRegistry::cancel_all`. Without it, spawning this task off
/// the read loop closed the "stalls the loop" hole above but reopened a
/// narrower one: nothing ever told an in-flight query the connection it was
/// spawned for was gone, so it could still `out_tx.send` an `agent.result`
/// after this connection's own `token.revoked`/close frames were already
/// enqueued — a stale reply on a wire the caller has already been told to
/// stop trusting — and it kept `out_tx`'s clone (and so `run_writer`'s
/// reason to keep running) alive for as long as the query took, independent
/// of whether anyone was still listening.
///
/// **This is NOT [`spawn_answer_assist`]'s mechanism reused** — that
/// function's cancellation (`AssistStreamRegistry` + `job_cancel`) stops a
/// STREAMING network call promptly because `Completer::stream_complete`
/// checks `is_cancelled` at every chunk boundary; `handle_agent_query`'s
/// `best-matches` path is one `spawn_blocking` CPU pass with no such
/// checkpoint, so cancelling here cannot preempt compute already running on
/// its own thread — it only stops that compute's result from ever being
/// sent, and frees this task (and `out_tx`) immediately instead of only once
/// the compute finishes. Worth naming plainly: `spawn_answer_assist` itself
/// still unconditionally `out_tx.send`s its terminal reply after
/// cancellation too (only the underlying job stops early) — the SAME
/// stale-reply-after-teardown gap this fix closes for `agent.query`, left
/// open there. Not fixed here (out of this finding's scope); flagged for a
/// follow-up rather than silently copied into a second call site.
pub(super) fn spawn_agent_query(
    app: AppHandle,
    req_id: String,
    payload: Value,
    out_tx: UnboundedSender<Message>,
    cancel: CancellationToken,
    caller: super::CallerClass,
) {
    tokio::spawn(async move {
        let query = super::agent_read::handle_agent_query(&app, &req_id, &payload);
        if let Some(reply) = agent_query_or_cancelled(query, &cancel).await {
            // The extension's own smaller reply cap (PR1) — applied ONLY for that caller; the
            // CLI's reply is unchanged (see `agent_read::extension_capped_reply`'s doc).
            let reply = if caller == super::CallerClass::Extension {
                super::agent_read::extension_capped_reply(&req_id, &payload, reply)
            } else {
                reply
            };
            let _ = out_tx.send(Message::text(reply));
        }
        // `None`: the connection tore down before the query finished — see
        // this fn's own doc. Nothing left to do; `out_tx`'s clone this task
        // held is dropped right here instead of after the (possibly still
        // running) compute finishes.
    });
}

/// Race `query` against `cancel` firing first. `None` when `cancel` wins —
/// the caller must never act on a query that raced a torn-down connection;
/// `Some(query`'s own output`)` when the query wins, the normal case.
///
/// Generic over `Q` (rather than `handle_agent_query`'s own concrete future)
/// so this race's OUTCOME is directly unit-testable without a live
/// `AppHandle` (this crate has no `tauri::test` mock-app harness) — mirrors
/// [`next_step`]'s existing generic-over-futures pattern.
pub(super) async fn agent_query_or_cancelled<Q>(
    query: Q,
    cancel: &CancellationToken,
) -> Option<Q::Output>
where
    Q: std::future::Future,
{
    tokio::select! {
        () = cancel.cancelled() => None,
        reply = query => Some(reply),
    }
}

/// Drive an `agent.call` (ADR-038 §2, Phase 2) on its OWN task — the SAME
/// mechanism as [`spawn_agent_query`] just above, reused verbatim via
/// [`agent_query_or_cancelled`] rather than duplicated: a dispatched
/// `Effect::Read` command can do real network I/O
/// (`discovery_search_companies`, `boards_health`, `profile_import_from_url`,
/// `github_import_repos`) or `autopilot_best_matches`'s own uncapped
/// clustering pass, and awaiting any of those inline here would stall THIS
/// connection's `reader.next()` — including its own `token.revoked`
/// observation, the exact reasoning `spawn_agent_query`'s doc lays out in
/// full. `cancel` is the SAME per-connection [`CancellationToken`]
/// `spawn_agent_query` is given (not a second one) — both are "agent-CLI
/// background dispatch for this connection" and are cancelled at the
/// identical teardown site in `handle_connection`.
pub(super) fn spawn_agent_call(
    app: AppHandle,
    req_id: String,
    payload: Value,
    out_tx: UnboundedSender<Message>,
    cancel: CancellationToken,
    caller: super::CallerClass,
) {
    tokio::spawn(async move {
        let call = super::agent_call::handle_agent_call(&app, &req_id, &payload);
        if let Some(reply) = agent_query_or_cancelled(call, &cancel).await {
            // Same extension-only cap as `spawn_agent_query` — see that fn's doc.
            let reply = if caller == super::CallerClass::Extension {
                super::agent_call::extension_capped_reply(&req_id, &payload, reply)
            } else {
                reply
            };
            let _ = out_tx.send(Message::text(reply));
        }
    });
}

/// The synchronous half of [`spawn_answer_assist`] — factored out so it is
/// directly unit-testable WITHOUT a live `AppHandle` (this crate has no
/// `tauri::test` mock-app harness). A plain (non-`async`) function, so a
/// caller observing `Some(_)` returned — or `registry.contains(req_id)` true
/// right after — has proof `begin` ran on ITS OWN thread, not deferred into
/// whatever thread `tokio::spawn`'s task eventually runs on. Returns
/// `Some(gen)` — the generation `begin` minted for this reqId, which the
/// caller MUST thread all the way to `handle_answer_assist`'s end-of-request
/// `unregister_gen` call (see [`super::assist_registry::StreamEntry`]'s doc
/// for the reused-reqId clobber this generation closes) — when `req_id` was
/// free (the caller should go on to spawn the actual task); `None` when it
/// already named an active entry — in which case this function has ALREADY
/// enqueued the `DUPLICATE_REQUEST_MESSAGE` reply through `out_tx` itself, so
/// the caller has nothing left to do but return.
fn begin_or_reject_duplicate(
    registry: &AssistStreamRegistry,
    req_id: &str,
    out_tx: &UnboundedSender<Message>,
) -> Option<u64> {
    if let Some(r#gen) = registry.begin(req_id) {
        return Some(r#gen);
    }
    let reply = super::answer_assist::answer_assist_reply(
        req_id,
        Err(AppError::Validation(
            super::answer_assist::DUPLICATE_REQUEST_MESSAGE.to_string(),
        )),
    );
    let _ = out_tx.send(Message::text(reply));
    None
}

#[cfg(test)]
mod tests;
