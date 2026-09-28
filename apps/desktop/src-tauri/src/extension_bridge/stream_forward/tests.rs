use super::*;

fn stream_chunk(delta: &str, done: bool, thinking: Option<bool>) -> AiStreamChunk {
    AiStreamChunk {
        job_id: "job-1".to_string(),
        delta: delta.to_string(),
        done,
        error: None,
        thinking,
    }
}

#[test]
fn forwardable_delta_forwards_a_plain_text_delta() {
    let chunk = stream_chunk("Because I ", false, None);
    assert_eq!(forwardable_delta(&chunk), Some("Because I "));
}

#[test]
fn forwardable_delta_skips_the_terminal_done_piece() {
    let chunk = stream_chunk("", true, None);
    assert_eq!(forwardable_delta(&chunk), None);
}

#[test]
fn forwardable_delta_skips_a_thinking_piece() {
    // A reasoning/thinking delta must never leak into the popup's
    // streaming preview — only the visible answer streams.
    let chunk = stream_chunk("pondering…", false, Some(true));
    assert_eq!(forwardable_delta(&chunk), None);
}

#[test]
fn forwardable_delta_skips_an_empty_delta() {
    let chunk = stream_chunk("", false, Some(false));
    assert_eq!(forwardable_delta(&chunk), None);
}

// ── forward_chunk (MEDIUM fix: live DRAFT_CAP enforcement; HIGH fix:
// dead-sink detection) ───────────────────────────────────────────────────

#[derive(Default)]
struct RecordingSink {
    sent: Vec<String>,
}

#[async_trait::async_trait]
impl FrameSink for RecordingSink {
    async fn send_frame(&mut self, text: String) -> bool {
        self.sent.push(text);
        true
    }
}

/// A sink whose transport is already gone — `send_frame` always reports
/// `false`, mirroring a disconnected client's outbound channel.
struct DeadSink;

#[async_trait::async_trait]
impl FrameSink for DeadSink {
    async fn send_frame(&mut self, _text: String) -> bool {
        false
    }
}

#[tokio::test]
async fn forward_chunk_stops_growing_accumulated_once_the_draft_cap_is_reached() {
    let mut sink = RecordingSink::default();
    let mut accumulated = String::new();

    // A single delta that exactly fills the cap.
    let cap = super::super::answer_assist::DRAFT_CAP;
    let first = stream_chunk(&"a".repeat(cap), false, None);
    let capped = forward_chunk(&first, "req-1", &mut sink, &mut accumulated, 0).await;
    assert_eq!(capped, ForwardOutcome::CapReached);
    assert_eq!(accumulated.chars().count(), cap);

    // A second delta arriving after the cap must never grow the buffer
    // or send another frame.
    let second = stream_chunk("more text", false, None);
    let capped_again = forward_chunk(&second, "req-1", &mut sink, &mut accumulated, 0).await;
    assert_eq!(capped_again, ForwardOutcome::CapReached);
    assert_eq!(
        accumulated.chars().count(),
        cap,
        "must never exceed the cap"
    );
    assert_eq!(
        sink.sent.len(),
        1,
        "the second delta must never be forwarded on the wire"
    );
}

/// A delta that would cross the cap is cut at the boundary, not dropped
/// whole. `cap_base = 0` is a single-attempt request (or attempt 1 of a
/// retried one); the REBASED window a retry gets is driven through this
/// same function by `answer_assist`'s
/// `compose_with_length_retry_*_draft_cap` tests.
#[tokio::test]
async fn forward_chunk_clamps_a_delta_that_would_cross_the_cap_mid_chunk() {
    let mut sink = RecordingSink::default();
    let cap = super::super::answer_assist::DRAFT_CAP;
    let mut accumulated = "x".repeat(cap - 5);

    // 10 chars incoming, only 5 fit before the cap.
    let chunk = stream_chunk("0123456789", false, None);
    let capped = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated, 0).await;

    assert_eq!(capped, ForwardOutcome::CapReached);
    assert_eq!(accumulated.chars().count(), cap);
    assert_eq!(
        sink.sent.last().unwrap(),
        &assist_chunk_frame("req-1", "01234")
    );
}

/// A retry's window starts where its OWN text starts (`compose_attempts`
/// snapshots it), so a first attempt that forwarded
/// inline-`<think>` prose right up to the cap cannot leave the retry a stub:
/// same buffer, fresh budget.
///
/// Mutation checks (both executed): count the whole buffer instead of the
/// slice past `cap_base` (the pre-fix shape) and the first pair fails
/// (`CapReached`, and a 5-char frame instead of the whole 10-char delta);
/// ignore what the attempt already spent (`spent` returns 0 whenever
/// `cap_base > 0`) and the second pair fails — the attempt forwards `cap + 10`.
#[tokio::test]
async fn forward_chunk_gives_each_attempt_a_full_cap_past_cap_base() {
    let mut sink = RecordingSink::default();
    let cap = super::super::answer_assist::DRAFT_CAP;
    // Attempt 1 spent all but 5 chars of a cap; the retry's window starts here.
    let mut accumulated = "x".repeat(cap - 5);
    let cap_base = accumulated.len();

    let chunk = stream_chunk("0123456789", false, None);
    let outcome = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated, cap_base).await;

    assert_eq!(
        outcome,
        ForwardOutcome::Continue,
        "10 chars is nowhere near the retry's own cap"
    );
    assert_eq!(
        sink.sent.last().unwrap(),
        &assist_chunk_frame("req-1", "0123456789"),
        "the retry's delta must reach the client whole"
    );

    // …and that window is still ONE cap, accumulated across the attempt's own
    // deltas: a second delta that would cross it is clamped, not waved through.
    let flood = stream_chunk(&"z".repeat(cap), false, None);
    let outcome = forward_chunk(&flood, "req-1", &mut sink, &mut accumulated, cap_base).await;

    assert_eq!(outcome, ForwardOutcome::CapReached);
    assert_eq!(
        accumulated.chars().count() - (cap - 5),
        cap,
        "the attempt forwarded exactly its own cap, never more"
    );
}

#[tokio::test]
async fn forward_chunk_reports_uncapped_while_under_the_limit() {
    let mut sink = RecordingSink::default();
    let mut accumulated = String::new();
    let chunk = stream_chunk("short delta", false, None);
    let capped = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated, 0).await;
    assert_eq!(capped, ForwardOutcome::Continue);
    assert_eq!(accumulated, "short delta");
    assert_eq!(sink.sent, vec![assist_chunk_frame("req-1", "short delta")]);
}

#[tokio::test]
async fn forward_chunk_reports_sink_gone_when_send_frame_returns_false() {
    let mut sink = DeadSink;
    let mut accumulated = String::new();
    let chunk = stream_chunk("hello", false, None);
    let outcome = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated, 0).await;
    assert_eq!(outcome, ForwardOutcome::SinkGone);
    assert_eq!(
        accumulated, "hello",
        "the delta is still accumulated locally even though the wire send failed"
    );
}

#[tokio::test]
async fn forward_chunk_never_reports_sink_gone_once_already_capped() {
    // Once the cap is reached, forward_chunk short-circuits before ever
    // touching the sink again — a dead sink discovered only AFTER the
    // cap must never surface, since there's nothing left to send.
    let mut sink = DeadSink;
    let cap = super::super::answer_assist::DRAFT_CAP;
    let mut accumulated = "x".repeat(cap);
    let chunk = stream_chunk("more", false, None);
    let outcome = forward_chunk(&chunk, "req-1", &mut sink, &mut accumulated, 0).await;
    assert_eq!(outcome, ForwardOutcome::CapReached);
}

#[test]
fn assist_chunk_frame_carries_the_delta_under_the_reqs_id() {
    let frame = assist_chunk_frame("req-9", "Because I ");
    let v: Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(v["type"], msg::ASSIST_CHUNK);
    assert_eq!(v["reqId"], "req-9");
    assert_eq!(v["payload"]["delta"], "Because I ");
}

#[test]
fn assist_done_frame_carries_no_payload() {
    let frame = assist_done_frame("req-9");
    let v: Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(v["type"], msg::ASSIST_DONE);
    assert_eq!(v["reqId"], "req-9");
    assert!(v["payload"].is_null());
}
