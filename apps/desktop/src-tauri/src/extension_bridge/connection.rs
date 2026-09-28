//! Per-connection accept + read loop — split from `mod.rs` (R8 relief). The handshake-only
//! `FrameDecision` arms (which mutate this loop's own `conn`/`authenticated`/`counted_epoch`
//! or `break`) stay inline here; every other verb is forwarded to `connection_dispatch`'s
//! [`super::connection_dispatch::dispatch_frame`]. See `mod.rs`'s own module doc for the full
//! security model this loop enforces.

use futures::StreamExt;
use serde_json::json;
use tauri::{AppHandle, Manager};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;

use super::connection_accept;
use super::connection_dispatch::dispatch_frame;
use super::frame::FrameDecision;
use super::frame_advance::{advance_frame_from, ConnState};
use super::{revoke, stream, BridgeState};
use crate::events::{emit_event, EXTENSION_BRIDGE_CHANGED};
use crate::observability::sanitize_reason;

/// Drive one accepted socket after [`connection_accept::accept_ws`]: the v2 mutual HMAC
/// challenge-response (`hello`→`challenge`→`auth`→`auth.ok`, verified **constant-time** via
/// [`handshake::verify_client_proof`] — the pairing token is never transmitted), then the
/// authenticated read loop. `connected` flips true ONLY once the client proof verifies (see
/// [`ConnState`]); a FAILED proof closes with **NO reply** (no oracle for a peer probing a
/// token); a legacy/outdated first frame gets `update.required` then closes (v1→v2 force
/// cutover, no dual-support path).
///
/// A multi-second streaming `answer.assist` handler runs on its OWN spawned task — see
/// [`stream`]'s module doc — never awaited inline, so this loop keeps polling `reader.next()`
/// (including a same-connection `assist.cancel`) while a stream is in flight. Every outbound
/// frame funnels through one channel into [`stream::run_writer`], the sole writer of the live
/// WS sink. This loop races THREE arms every iteration (via [`stream::next_step`]): the reader,
/// [`stream::run_writer`]'s own detached `JoinHandle` (so a `WRITE_STALL` timeout or send error
/// tears the connection down immediately instead of waiting on a next inbound frame that may
/// never arrive for a quiet/stalled peer), and this connection's
/// [`BridgeState::subscribe_revoke`] receiver (a token rotation must reach even a QUIET
/// connection — an authenticated one is sent [`msg::TOKEN_REVOKED`], an unauthenticated one
/// closes silently, no oracle).
pub(super) async fn handle_connection(app: AppHandle, stream: TcpStream) {
    let Some((ws, caller_class)) = connection_accept::accept_ws(stream).await else {
        return;
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
    // In-flight streaming `answer.assist` jobs for THIS connection only — see
    // `stream::AssistStreamRegistry`'s doc for why this is per-connection
    // rather than a field on the global `BridgeState`.
    let assist_streams = std::sync::Arc::new(stream::AssistStreamRegistry::default());
    // Cancels every in-flight `agent.query`/`agent.call` spawned for THIS connection
    // (MAJOR fix — security review round 2) — cancelled once, below, at the
    // SAME shared teardown site as `assist_streams.cancel_all`, so it covers
    // every way this loop can end (a token revocation, but also a normal
    // close, a read error, or a stalled writer), not just revocation. See
    // `stream::spawn_agent_query`'s doc for what this closes.
    let agent_query_cancel = tokio_util::sync::CancellationToken::new();

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
        let reply = match advance_frame_from(&state, &conn, &text, caller_class) {
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
            other => {
                dispatch_frame(
                    other,
                    &app,
                    &state,
                    &out_tx,
                    &assist_streams,
                    &agent_query_cancel,
                )
                .await
            }
        };
        if let Some(reply) = reply {
            if out_tx.send(Message::text(reply)).is_err() {
                break;
            }
        }
    }

    // The socket is gone — cancel every stream still registered for THIS connection (not just
    // an explicit `assist.cancel`'s target), else a disconnect mid-`answer.assist` leaves the
    // billable generation running for no consumer. Per-connection by construction (`stream`'s
    // module doc) — never a global `BridgeState` field.
    assist_streams.cancel_all(&app);
    // Same reasoning, for every in-flight `agent.query`/`agent.call` this connection spawned
    // (MAJOR fix — security review round 2) — see `stream::spawn_agent_query`'s doc.
    agent_query_cancel.cancel();
    // Only a socket that reached `Authenticated` (and so incremented the count) decrements it
    // here. After a REVOKE the count was already zeroed by `regenerate_token` and the epoch
    // moved on, so this decrement is SKIPPED (not merely saturating): a socket parked in a long
    // dispatch await may find a browser already re-paired on the new token by the time it gets
    // here, and giving back a count it no longer owns would take that live pairing 1→0. The
    // rotation path owns the notification instead.
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

/// Await `export` (in production, [`document_export::handle_document_export`]), then discard its
/// reply if this connection's pairing was revoked WHILE it was in flight, rather than enqueue a
/// résumé/cover-letter for a socket that is already gone. The read loop awaits `document.export`
/// inline (a real Typst compile), so it cannot poll `revoked_rx` until this returns; without this
/// guard a rotation landing mid-compile would still enqueue `document.result` ahead of the next
/// `NextStep::Revoked` handling. `state`'s rotation epoch is bumped inside the SAME lock hold
/// `regenerate_token` sends its revoke broadcast under, so ANY rotation reaching this connection
/// (subscribed at accept time, before the handshake) moves it — an epoch mismatch after the
/// await proves a revoke landed mid-export. Generic over `export` so the race is unit-testable
/// without a live socket/real Typst compile: a test future can rotate `state` itself mid-flight.
pub(super) async fn export_reply_unless_revoked(
    state: &BridgeState,
    export: impl std::future::Future<Output = String>,
) -> Option<String> {
    let epoch_before = state.rotation_epoch();
    let reply = export.await;
    (state.rotation_epoch() == epoch_before).then_some(reply)
}

#[cfg(test)]
mod tests;
