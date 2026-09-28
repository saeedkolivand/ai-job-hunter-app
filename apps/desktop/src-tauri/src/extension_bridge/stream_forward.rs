//! Live cap-clamped delta forwarding for a streaming `answer.assist` — split from `stream.rs`
//! (R8 relief). [`forward_chunk`] is [`super::stream_compose::compose_draft_stream`]'s per-delta
//! call; see `stream`'s own module doc for the whole streaming design.

use serde_json::{json, Value};

use super::msg;
use super::stream::FrameSink;
use crate::events::AiStreamChunk;

/// Whether `chunk` (an `ai:stream` event for the job [`compose_draft_stream`]
/// is driving) carries visible answer text to forward as an `assist.chunk`
/// frame — `None` for the terminal `done` piece, a reasoning/`thinking`
/// piece (the popup streams only the visible answer, never chain-of-thought),
/// or an already-empty delta. Pure — directly unit-tested without a live
/// `AppHandle`/event.
pub(super) fn forwardable_delta(chunk: &AiStreamChunk) -> Option<&str> {
    if chunk.done || chunk.thinking == Some(true) || chunk.delta.is_empty() {
        None
    } else {
        Some(chunk.delta.as_str())
    }
}

/// Outcome of forwarding one delta through [`forward_chunk`] — tells
/// [`compose_draft_stream`] whether (and why) to stop the generation early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ForwardOutcome {
    /// Keep going — under the cap, sink still alive.
    Continue,
    /// [`super::answer_assist::DRAFT_CAP`] was reached (sink still alive) —
    /// stop forwarding; the caller cancels the job immediately to bound cost
    /// and latency, the same path an `assist.cancel` drives.
    CapReached,
    /// `sink.send_frame` reported the transport is gone (returned `false`)
    /// — no consumer left; the caller should cancel the job immediately,
    /// not wait for the cap or a natural finish.
    SinkGone,
}

/// Forward one delta (if any, per [`forwardable_delta`]) through `sink`,
/// clamped so `accumulated` never grows more than
/// [`super::answer_assist::DRAFT_CAP`] chars PAST `cap_base` — the LIVE,
/// mid-stream sibling of `resolve_answer_assist`'s own terminal `clamp_chars`
/// safety net. See [`ForwardOutcome`] for what each return value tells the
/// caller to do.
///
/// `cap_base` is where the current attempt's window starts (`0` for a
/// single-attempt caller — see [`compose_draft_stream`]). Not a
/// boundary of the buffer it indexes ⇒ the whole buffer is counted instead —
/// the safe direction, since counting MORE only makes the cap bite sooner.
pub(super) async fn forward_chunk(
    chunk: &AiStreamChunk,
    req_id: &str,
    sink: &mut dyn FrameSink,
    accumulated: &mut String,
    cap_base: usize,
) -> ForwardOutcome {
    let cap = super::answer_assist::DRAFT_CAP;
    let spent = |acc: &str| acc.get(cap_base..).unwrap_or(acc).chars().count();
    let Some(delta) = forwardable_delta(chunk) else {
        return if spent(accumulated) >= cap {
            ForwardOutcome::CapReached
        } else {
            ForwardOutcome::Continue
        };
    };
    let remaining = cap.saturating_sub(spent(accumulated));
    if remaining == 0 {
        return ForwardOutcome::CapReached;
    }
    let piece: std::borrow::Cow<'_, str> = if delta.chars().count() > remaining {
        delta.chars().take(remaining).collect::<String>().into()
    } else {
        delta.into()
    };
    if piece.is_empty() {
        return ForwardOutcome::Continue;
    }
    accumulated.push_str(&piece);
    if !sink.send_frame(assist_chunk_frame(req_id, &piece)).await {
        return ForwardOutcome::SinkGone;
    }
    if spent(accumulated) >= cap {
        ForwardOutcome::CapReached
    } else {
        ForwardOutcome::Continue
    }
}

/// Build an `assist.chunk { delta }` frame.
fn assist_chunk_frame(req_id: &str, delta: &str) -> String {
    json!({
        "type": msg::ASSIST_CHUNK,
        "reqId": req_id,
        "payload": { "delta": delta },
    })
    .to_string()
}

/// Build the terminal, no-payload `assist.done` frame for `req_id`.
/// `pub(super)` so `answer_assist`'s fake compose round can emit the REAL
/// frame — the "exactly one `assist.done` per request" assertion is then
/// about the production frame, not about a re-typed copy of it.
pub(super) fn assist_done_frame(req_id: &str) -> String {
    json!({
        "type": msg::ASSIST_DONE,
        "reqId": req_id,
        "payload": Value::Null,
    })
    .to_string()
}

#[cfg(test)]
mod tests;
