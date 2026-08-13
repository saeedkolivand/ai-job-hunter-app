//! Unit tests for `stream.rs`, split into this sibling file (R8 line-budget
//! split — mirrors the `openai.rs` + `openai_tests.rs` precedent of moving
//! the test module itself out rather than production code).
//!
//! Wired via `#[path = "stream_tests.rs"] mod tests;` in `stream.rs` — that
//! keeps this a CHILD module of `stream` in the module tree (same as an
//! inline `#[cfg(test)] mod tests { ... }` block), so `use super::*;` below
//! still reaches every private item there, while this file's own filename
//! (ending `tests.rs`) excludes it from the architecture test's R8 LOC cap
//! (`tests/architecture.rs`'s `is_test` filename check) and from R3/R6's
//! non-test scans.

use super::*;
use std::cell::Cell;

#[test]
fn stream_piece_constructors_set_flags() {
    let t = StreamPiece::text("hi");
    assert_eq!(t.delta, "hi");
    assert!(!t.thinking);
    assert!(!t.done);

    let r = StreamPiece::thinking("reasoning");
    assert!(r.thinking);
    assert!(!r.done);

    let d = StreamPiece::done("");
    assert!(d.done);
    assert!(d.delta.is_empty());
}

/// Collect the sink actions `drive_stream` produces for a canned chunk list.
/// Each piece is identified by `(emit:delta/thinking, complete, cancelled, error)`.
/// `Complete` carries the final [`Usage`] AND the accumulated answer text
/// (non-thinking deltas only) that `finish` persists — see the usage- and
/// answer-tracking tests below.
#[derive(Debug, PartialEq)]
enum Act {
    Emit(String, bool),
    Complete(Usage, String),
    Cancelled(Usage),
    Error(String, Usage),
}

fn run(
    chunks: Vec<AppResult<Option<Vec<u8>>>>,
    cancel_after: Option<usize>,
    parse: impl FnMut(&mut String) -> Vec<StreamPiece>,
) -> Vec<Act> {
    let acts = std::cell::RefCell::new(Vec::new());
    let idx = Cell::new(0usize);
    let mut chunks = chunks.into_iter();
    let cancel_calls = Cell::new(0usize);

    let mut cancelled = || {
        let n = cancel_calls.get();
        cancel_calls.set(n + 1);
        cancel_after.map(|after| n >= after).unwrap_or(false)
    };

    let fut = drive_stream(
        &mut cancelled,
        || {
            let _ = idx.get();
            let next = chunks.next().unwrap_or(Ok(None));
            async move { next }
        },
        parse,
        |sink| {
            let act = match sink {
                StreamSink::Emit { delta, thinking } => Act::Emit(delta, thinking),
                StreamSink::Complete(usage, answer) => Act::Complete(usage, answer),
                StreamSink::Cancelled(usage) => Act::Cancelled(usage),
                StreamSink::Error(e, usage) => Act::Error(e.to_string(), usage),
            };
            acts.borrow_mut().push(act);
        },
    );
    // The future is synchronous (the fake chunk source resolves immediately).
    futures::executor::block_on(fut);
    acts.into_inner()
}

/// A trivial newline-delimited parser: each complete line becomes a text piece;
/// a line equal to `END` is the sentinel.
fn line_parser(buf: &mut String) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    while let Some(nl) = buf.find('\n') {
        let line = buf[..nl].trim().to_string();
        *buf = buf[nl + 1..].to_string();
        if line == "END" {
            out.push(StreamPiece::done(""));
        } else if !line.is_empty() {
            out.push(StreamPiece::text(line));
        }
    }
    out
}

#[test]
fn emits_pieces_then_completes_on_sentinel() {
    let acts = run(
        vec![
            Ok(Some(b"hello\nwor".to_vec())),
            Ok(Some(b"ld\nEND\n".to_vec())),
        ],
        None,
        line_parser,
    );
    assert_eq!(
        acts,
        vec![
            Act::Emit("hello".to_string(), false),
            Act::Emit("world".to_string(), false),
            Act::Complete(Usage::default(), "helloworld".to_string()),
        ]
    );
}

#[test]
fn a_multibyte_char_split_across_reads_is_not_corrupted() {
    // `response.chunk()` cuts the body at arbitrary byte offsets, so one
    // multi-byte char routinely straddles two reads. Decoding each read on
    // its own turned BOTH halves into U+FFFD, and the mojibake was persisted
    // as the finished document. The em dash here is E2 80 94, cut 1|2.
    let acts = run(
        vec![
            Ok(Some(vec![b'a', 0xE2])),
            Ok(Some(vec![0x80, 0x94, b'b', b'\n'])),
            Ok(None),
        ],
        None,
        line_parser,
    );
    assert_eq!(
        acts,
        vec![
            Act::Emit("a\u{2014}b".to_string(), false),
            Act::Complete(Usage::default(), "a\u{2014}b".to_string()),
        ]
    );
}

#[test]
fn a_multibyte_char_split_2_1_and_across_three_reads_is_not_corrupted() {
    // Same char cut 2|1, plus a 4-byte emoji (F0 9F 9A 80) dribbled one byte
    // per read — the carry must survive an arbitrary number of empty-yield
    // reads, not just one.
    let acts = run(
        vec![
            Ok(Some(vec![0xE2, 0x80])),
            Ok(Some(vec![0x94])),
            Ok(Some(vec![0xF0])),
            Ok(Some(vec![0x9F])),
            Ok(Some(vec![0x9A])),
            Ok(Some(vec![0x80, b'\n'])),
            Ok(None),
        ],
        None,
        line_parser,
    );
    assert_eq!(
        acts,
        vec![
            Act::Emit("\u{2014}\u{1F680}".to_string(), false),
            Act::Complete(Usage::default(), "\u{2014}\u{1F680}".to_string()),
        ]
    );
}

#[test]
fn genuinely_invalid_bytes_still_collapse_to_one_replacement_char() {
    // A corrupt transfer (never a provider) must not stall the loop: an
    // invalid sequence becomes exactly one U+FFFD and decoding continues.
    let acts = run(
        vec![Ok(Some(vec![b'a', 0xFF, b'b', b'\n'])), Ok(None)],
        None,
        line_parser,
    );
    assert_eq!(
        acts,
        vec![
            Act::Emit("a\u{FFFD}b".to_string(), false),
            Act::Complete(Usage::default(), "a\u{FFFD}b".to_string()),
        ]
    );
}

#[test]
fn an_incomplete_trailing_sequence_at_end_of_body_does_not_hang() {
    // The body ends mid-character: the held-back bytes are simply dropped and
    // the loop still completes exactly once.
    let acts = run(
        vec![Ok(Some(vec![b'a', b'\n', 0xE2])), Ok(None)],
        None,
        line_parser,
    );
    assert_eq!(
        acts,
        vec![
            Act::Emit("a".to_string(), false),
            Act::Complete(Usage::default(), "a".to_string()),
        ]
    );
}

#[test]
fn completes_once_on_end_of_body_without_sentinel() {
    // No `END` line — the loop still completes exactly once when the body ends.
    let acts = run(
        vec![Ok(Some(b"a\nb\n".to_vec())), Ok(None)],
        None,
        line_parser,
    );
    assert_eq!(
        acts,
        vec![
            Act::Emit("a".to_string(), false),
            Act::Emit("b".to_string(), false),
            Act::Complete(Usage::default(), "ab".to_string()),
        ]
    );
}

#[test]
fn cancellation_short_circuits_before_reading() {
    // Cancelled on the first check → no chunk is read, no completion
    // emitted, and no usage was ever seen (zero, not fabricated).
    let acts = run(
        vec![Ok(Some(b"hello\nEND\n".to_vec()))],
        Some(0),
        line_parser,
    );
    assert_eq!(acts, vec![Act::Cancelled(Usage::default())]);
}

#[test]
fn cancellation_mid_stream_stops_without_complete() {
    // First check passes (reads + emits), second check cancels before the next read.
    let acts = run(
        vec![Ok(Some(b"hello\n".to_vec())), Ok(Some(b"world\n".to_vec()))],
        Some(1),
        line_parser,
    );
    assert_eq!(
        acts,
        vec![
            Act::Emit("hello".to_string(), false),
            Act::Cancelled(Usage::default())
        ]
    );
}

#[test]
fn read_error_surfaces_and_stops() {
    let acts = run(
        vec![
            Ok(Some(b"a\n".to_vec())),
            Err(AppError::Message("boom".to_string())),
        ],
        None,
        line_parser,
    );
    assert_eq!(
        acts,
        vec![
            Act::Emit("a".to_string(), false),
            Act::Error("boom".to_string(), Usage::default()),
        ]
    );
}

#[test]
fn final_delta_on_sentinel_is_emitted_before_complete() {
    // A sentinel piece that also carries text emits the text, then completes.
    let parser = |buf: &mut String| -> Vec<StreamPiece> {
        let s = std::mem::take(buf);
        if s.is_empty() {
            vec![]
        } else {
            vec![StreamPiece::done(s)]
        }
    };
    let acts = run(vec![Ok(Some(b"tail".to_vec()))], None, parser);
    assert_eq!(
        acts,
        vec![
            Act::Emit("tail".to_string(), false),
            Act::Complete(Usage::default(), "tail".to_string())
        ]
    );
}

// ── Usage tracking: latest-wins + record-once-at-completion ────────────────

#[test]
fn a_later_usage_piece_overwrites_an_earlier_one_at_completion() {
    // Two usage-only pieces (no delta) arrive across two chunks, then the
    // sentinel — `Complete` must carry only the LAST usage seen, mirroring
    // Anthropic's incremental `message_start`/`message_delta` reporting and
    // Gemini/Ollama repeating a running total.
    let parser = |buf: &mut String| -> Vec<StreamPiece> {
        let mut out = Vec::new();
        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim().to_string();
            *buf = buf[nl + 1..].to_string();
            match line.as_str() {
                "USAGE1" => out.push(StreamPiece::usage(Usage {
                    input_tokens: 10,
                    output_tokens: 1,
                    thinking_tokens: None,
                })),
                "USAGE2" => out.push(StreamPiece::usage(Usage {
                    input_tokens: 10,
                    output_tokens: 99,
                    thinking_tokens: None,
                })),
                "END" => out.push(StreamPiece::done("")),
                _ => {}
            }
        }
        out
    };
    let acts = run(
        vec![
            Ok(Some(b"USAGE1\n".to_vec())),
            Ok(Some(b"USAGE2\nEND\n".to_vec())),
        ],
        None,
        parser,
    );
    assert_eq!(
        acts,
        vec![Act::Complete(
            Usage {
                input_tokens: 10,
                output_tokens: 99,
                thinking_tokens: None,
            },
            // Usage-only pieces carry no visible delta, so the persisted
            // answer is empty here.
            String::new(),
        )],
        "only the LAST usage piece must be recorded, not the first or a sum"
    );
}

#[test]
fn cancellation_after_a_usage_piece_still_carries_the_partial_usage() {
    // A usage piece arrives, then cancellation (e.g. `answer.assist`'s
    // live DRAFT_CAP calling `job_cancel`) — production now records
    // whatever REAL usage was already seen even on the `Cancelled` sink
    // (never through `Complete`/`finish`, which would also wrongly emit
    // a terminal `job_complete`), so a cost-capped generation is never
    // invisible to spend tracking.
    let parser = |buf: &mut String| -> Vec<StreamPiece> {
        let mut out = Vec::new();
        while let Some(nl) = buf.find('\n') {
            *buf = buf[nl + 1..].to_string();
            out.push(StreamPiece::usage(Usage {
                input_tokens: 50,
                output_tokens: 50,
                thinking_tokens: None,
            }));
        }
        out
    };
    let acts = run(
        vec![Ok(Some(b"USAGE\n".to_vec())), Ok(Some(b"USAGE\n".to_vec()))],
        Some(1),
        parser,
    );
    assert_eq!(
        acts,
        vec![Act::Cancelled(Usage {
            input_tokens: 50,
            output_tokens: 50,
            thinking_tokens: None,
        })],
        "cancellation must still carry the REAL usage already seen, never fabricated but never silently dropped either"
    );
}

#[test]
fn transport_error_after_a_usage_piece_still_carries_the_partial_usage() {
    // Same shape as the cancellation test above, but the stream fails
    // with a read error instead — production now records whatever REAL
    // usage was already seen on this path too (see `stream_response`'s
    // error branch), so a transport failure mid-stream no longer
    // undercounts spend the provider already reported.
    let parser = |buf: &mut String| -> Vec<StreamPiece> {
        let mut out = Vec::new();
        while let Some(nl) = buf.find('\n') {
            *buf = buf[nl + 1..].to_string();
            out.push(StreamPiece::usage(Usage {
                input_tokens: 50,
                output_tokens: 50,
                thinking_tokens: None,
            }));
        }
        out
    };
    let acts = run(
        vec![
            Ok(Some(b"USAGE\n".to_vec())),
            Err(AppError::Message("boom".to_string())),
        ],
        None,
        parser,
    );
    assert_eq!(
        acts,
        vec![Act::Error(
            "boom".to_string(),
            Usage {
                input_tokens: 50,
                output_tokens: 50,
                thinking_tokens: None,
            }
        )],
        "a transport error must still carry the REAL usage already seen, never fabricated \
         but never silently dropped either"
    );
}

// ── Persisted answer text (the poll-fallback contract) ─────────────────────
//
// `finish` persists the accumulated answer as `result.text` so a renderer
// that missed stream frames or the terminal `done` event recovers the
// finished document by polling `jobs_get`. These tests pin the two
// properties that make that safe: (1) only NON-thinking deltas contribute
// (reasoning is never persisted), and (2) inline `<think>…</think>` markup
// is stripped, so the poll fallback's longer-wins branch can never resolve
// reasoning markup into the final document.

#[test]
fn complete_carries_only_non_thinking_answer_text() {
    // A parser marking `T:`-prefixed lines as reasoning; everything else is
    // answer. The `Complete` sink (what `finish` persists) must carry ONLY
    // the answer deltas — reasoning is excluded, exactly as the renderer
    // routes provider-flagged `thinking` chunks away from its answer buffer.
    let parser = |buf: &mut String| -> Vec<StreamPiece> {
        let mut out = Vec::new();
        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim().to_string();
            *buf = buf[nl + 1..].to_string();
            if line == "END" {
                out.push(StreamPiece::done(""));
            } else if let Some(reason) = line.strip_prefix("T:") {
                out.push(StreamPiece::thinking(reason.to_string()));
            } else if !line.is_empty() {
                out.push(StreamPiece::text(line));
            }
        }
        out
    };
    let acts = run(
        vec![Ok(Some(
            b"Dear team,\nT:they want speed\nI apply.\nEND\n".to_vec(),
        ))],
        None,
        parser,
    );
    assert_eq!(
        acts,
        vec![
            Act::Emit("Dear team,".to_string(), false),
            Act::Emit("they want speed".to_string(), true),
            Act::Emit("I apply.".to_string(), false),
            Act::Complete(Usage::default(), "Dear team,I apply.".to_string()),
        ],
        "the persisted answer must exclude provider-flagged reasoning deltas"
    );
}

#[test]
fn a_stream_that_only_ever_emits_thinking_leaves_the_accumulated_answer_empty() {
    // HIGH (empty-completion job.completed bug): a reasoning model that runs
    // out of budget WHILE reasoning — or one whose provider never surfaces a
    // final channel at all — can legitimately reach the sentinel having
    // streamed real content, ALL of it thinking-flagged. `answer` (what
    // `finish` persists as `result.text`, see the tests above) must stay
    // empty in that case — this is the exact precondition `finish`'s
    // empty-answer branch exists to catch (see `stream.rs`'s `finish` doc):
    // a stream that "succeeded" at the transport level but produced nothing
    // usable must not be persisted as a completed job with `text: ""`.
    let parser = |buf: &mut String| -> Vec<StreamPiece> {
        let mut out = Vec::new();
        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim().to_string();
            *buf = buf[nl + 1..].to_string();
            if line == "END" {
                out.push(StreamPiece::done(""));
            } else if let Some(reason) = line.strip_prefix("T:") {
                out.push(StreamPiece::thinking(reason.to_string()));
            }
        }
        out
    };
    let acts = run(
        vec![Ok(Some(
            b"T:pondering the request at length\nEND\n".to_vec(),
        ))],
        None,
        parser,
    );
    assert_eq!(
        acts,
        vec![
            Act::Emit("pondering the request at length".to_string(), true),
            Act::Complete(Usage::default(), String::new()),
        ],
        "an all-thinking stream must complete with an EMPTY accumulated answer, \
         never a fabricated fallback"
    );
}

#[test]
fn persisted_answer_strips_inline_think_markup_it_never_leaks() {
    // A local reasoning model embeds <think>…</think> inline in a single
    // answer delta (thinking:false — the renderer's splitter, not the
    // provider, separates it). The loop accumulates the RAW delta...
    let parser = |buf: &mut String| -> Vec<StreamPiece> {
        let s = std::mem::take(buf);
        if s.is_empty() {
            vec![]
        } else {
            vec![StreamPiece::done(s)]
        }
    };
    let raw = "Dear team,<think>they want speed, be brief</think> I apply now.";
    let acts = run(vec![Ok(Some(raw.as_bytes().to_vec()))], None, parser);
    assert_eq!(
        acts,
        vec![
            Act::Emit(raw.to_string(), false),
            Act::Complete(Usage::default(), raw.to_string()),
        ],
        "the accumulated answer is the raw stream; stripping happens in `finish`"
    );
    // ...but `finish` persists the THINK-STRIPPED text, so reasoning markup
    // can never reach the final document via the poll fallback.
    let persisted = strip_think_blocks(raw);
    assert_eq!(persisted, "Dear team, I apply now.");
    assert!(
        !persisted.contains("<think>") && !persisted.contains("</think>"),
        "persisted text must never contain reasoning markup"
    );
}

#[test]
fn a_close_tag_split_across_frames_still_strips_clean() {
    // `</think>` arrives split across two frames. The renderer's STREAMING
    // splitter can mis-handle this, but the persisted answer accumulates the
    // whole stream first and strips in one pass — so the persisted text is
    // strictly equal-or-more-correct and never leaks markup.
    let passthrough = |buf: &mut String| -> Vec<StreamPiece> {
        let s = std::mem::take(buf);
        if s.is_empty() {
            vec![]
        } else {
            vec![StreamPiece::text(s)]
        }
    };
    let acts = run(
        vec![
            Ok(Some(b"a<think>b</thi".to_vec())),
            Ok(Some(b"nk>c".to_vec())),
            Ok(None),
        ],
        None,
        passthrough,
    );
    assert_eq!(
        acts,
        vec![
            Act::Emit("a<think>b</thi".to_string(), false),
            Act::Emit("nk>c".to_string(), false),
            Act::Complete(Usage::default(), "a<think>b</think>c".to_string()),
        ]
    );
    assert_eq!(
        strip_think_blocks("a<think>b</think>c"),
        "ac",
        "a </think> split across two stream frames still strips clean once the full \
         answer is accumulated"
    );
}

#[test]
fn strip_think_blocks_matches_the_renderer_splitter() {
    // Plain text is untouched.
    assert_eq!(strip_think_blocks("hello world"), "hello world");
    // A single block is removed, surrounding text kept.
    assert_eq!(
        strip_think_blocks("answer<think>reasoning</think>more"),
        "answermore"
    );
    // Multiple blocks.
    assert_eq!(
        strip_think_blocks("a<think>x</think>b<think>y</think>c"),
        "abc"
    );
    // A leading block.
    assert_eq!(strip_think_blocks("<think>r</think>visible"), "visible");
    // An empty block.
    assert_eq!(strip_think_blocks("a<think></think>b"), "ab");
    // An UNTERMINATED block discards everything from the tag onward — the
    // renderer's splitter drops an unterminated block at flush().
    assert_eq!(strip_think_blocks("keep<think>dropped forever"), "keep");
    // Whatever the input, the output can never contain reasoning markup.
    for s in [
        "answer<think>reasoning</think>more",
        "<think>r</think>visible",
        "keep<think>dropped forever",
    ] {
        let out = strip_think_blocks(s);
        assert!(
            !out.contains("<think>") && !out.contains("</think>"),
            "{s:?} leaked markup"
        );
    }
}

// ── empty_answer_message (MEDIUM — finish_reason threading) ────────────────
//
// `finish` and `cli_agent::emit_done` both route their empty-answer `Err`
// message through this one pure decision, so it is directly testable
// without the `AppHandle` this crate has no test harness for (see e.g.
// `openai_tests.rs`'s note on the same limitation).

#[test]
fn empty_answer_message_reports_the_length_truncation_distinctly() {
    assert_eq!(
        empty_answer_message(Some(StopReason::Length), ProviderId::OpenAi),
        EMPTY_ANSWER_LENGTH_MESSAGE,
        "finish_reason: length must get its own, more actionable message"
    );
}

/// Local Ollama is the ONE provider whose output cap the user can raise in
/// the app (`LocalModelLimits`, rendered only under the Ollama card), so it
/// gets the actionable wording. Every other provider must keep the generic
/// one — pointing them at a control they cannot see is worse than no advice,
/// which is why the original message dropped the pointer entirely.
///
/// This distinction only became reachable when local Ollama started
/// reporting `done_reason`; before that it never hit the `Length` arm.
#[test]
fn empty_answer_message_points_local_ollama_at_the_control_it_actually_has() {
    assert_eq!(
        empty_answer_message(Some(StopReason::Length), ProviderId::Ollama),
        EMPTY_ANSWER_LENGTH_LOCAL_MESSAGE
    );
    assert!(
        EMPTY_ANSWER_LENGTH_LOCAL_MESSAGE.contains("Max output tokens"),
        "must name the field as the UI labels it"
    );
    // The differential — no other provider may be sent to that field.
    for p in [
        ProviderId::OllamaCloud,
        ProviderId::OpenAi,
        ProviderId::OpenAiCompatible,
        ProviderId::Anthropic,
        ProviderId::Gemini,
    ] {
        assert_eq!(
            empty_answer_message(Some(StopReason::Length), p),
            EMPTY_ANSWER_LENGTH_MESSAGE,
            "{} has no adjustable output cap in the UI",
            p.as_str()
        );
    }
}

#[test]
fn empty_answer_message_falls_back_to_the_generic_message_otherwise() {
    // No signal at all (most providers/CLI agents) and a non-`Length`
    // signal both fall back to the SAME generic message — only `Length`
    // is distinct, per the report this closes.
    for reason in [None, Some(StopReason::End), Some(StopReason::Other)] {
        assert_eq!(
            empty_answer_message(reason, ProviderId::OpenAi),
            EMPTY_ANSWER_MESSAGE
        );
    }
}

// ── finish_outcome / truncation_notification (a truncated-but-non-empty
// completion must still complete, but with a non-fatal warning) ────────────
//
// `finish` used to consult `stop_reason` only on the EMPTY path — a stream
// that emitted real text and then reported `finish_reason: "length"` took
// the plain success path with no signal anywhere that the saved document
// might be cut off. `finish_outcome` is the pure decision `finish` now
// delegates to, directly testable without the `AppHandle` this crate has no
// test harness for (see the `empty_answer_message` note above).

/// The exact scenario from the report: real text streamed, then the
/// provider's `finish_reason` came back `length`. The completion must
/// still be treated as a SUCCESS (the partial text is real and worth
/// keeping) — never re-routed into the `Empty`/error arm — AND a warning
/// must be attached so the caller can surface it.
#[test]
fn a_truncated_non_empty_completion_still_completes_with_the_partial_text_and_warns() {
    let outcome = finish_outcome(
        "here is some partial output that got cut off",
        Some(StopReason::Length),
        ProviderId::OpenAi,
    );
    match outcome {
        FinishOutcome::Complete { text, warning } => {
            assert_eq!(text, "here is some partial output that got cut off");
            let warning = warning.expect("a non-empty Length completion must warn");
            assert_eq!(warning.kind, TRUNCATED_NOTIFICATION_KIND);
            assert!(
                warning.body.to_lowercase().contains("budget")
                    || warning.body.to_lowercase().contains("cut off"),
                "warning body must actually explain the truncation, got: {}",
                warning.body
            );
        }
        FinishOutcome::Empty { .. } => {
            panic!("a non-empty answer must never be routed through the Empty arm")
        }
    }
}

/// A normal completion (`stop_reason: End`, or none at all) must produce
/// NO warning — the whole point is that this is a targeted signal for the
/// truncation case, not noise on every generation.
#[test]
fn a_normal_completion_produces_no_warning() {
    for reason in [None, Some(StopReason::End), Some(StopReason::ToolUse)] {
        let outcome = finish_outcome("a complete answer", reason, ProviderId::OpenAi);
        match outcome {
            FinishOutcome::Complete { text, warning } => {
                assert_eq!(text, "a complete answer");
                assert!(
                    warning.is_none(),
                    "stop_reason {reason:?} must not produce a truncation warning"
                );
            }
            FinishOutcome::Empty { .. } => panic!("non-empty answer, must not be Empty"),
        }
    }
}

/// The EMPTY path is untouched by this change: `finish_reason: length` with
/// NO text at all still routes through the pre-existing empty-answer
/// message, never `truncation_notification` (that path already has its own
/// distinct, more actionable message — see `empty_answer_message`).
#[test]
fn an_empty_length_completion_still_uses_the_empty_answer_message_not_a_warning() {
    let outcome = finish_outcome("   ", Some(StopReason::Length), ProviderId::OpenAi);
    match outcome {
        FinishOutcome::Empty { message } => {
            assert_eq!(message, EMPTY_ANSWER_LENGTH_MESSAGE);
        }
        FinishOutcome::Complete { .. } => panic!("whitespace-only must be Empty"),
    }
}

#[test]
fn truncation_notification_is_none_for_every_non_length_stop_reason() {
    for reason in [
        None,
        Some(StopReason::End),
        Some(StopReason::ToolUse),
        Some(StopReason::Other),
    ] {
        assert!(truncation_notification(ProviderId::OpenAi, reason).is_none());
    }
}

#[test]
fn truncation_notification_points_local_ollama_at_the_control_it_actually_has() {
    let n =
        truncation_notification(ProviderId::Ollama, Some(StopReason::Length)).expect("must warn");
    assert_eq!(n.body, TRUNCATED_ANSWER_LOCAL_MESSAGE);
    assert!(n.body.contains("Max output tokens"));

    let n =
        truncation_notification(ProviderId::OpenAi, Some(StopReason::Length)).expect("must warn");
    assert_eq!(n.body, TRUNCATED_ANSWER_MESSAGE);
    assert!(
        !n.body.contains("Max output tokens"),
        "non-Ollama providers have no such Settings control"
    );
}
