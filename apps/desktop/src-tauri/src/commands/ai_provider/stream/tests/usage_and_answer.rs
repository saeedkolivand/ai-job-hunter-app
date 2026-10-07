//! Usage tracking (latest-wins + record-once-at-completion) and the
//! persisted-answer/think-strip contract.

use super::super::*;
use super::support::{run, Act};

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
                    timings: None,
                })),
                "USAGE2" => out.push(StreamPiece::usage(Usage {
                    input_tokens: 10,
                    output_tokens: 99,
                    thinking_tokens: None,
                    timings: None,
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
                timings: None,
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
                timings: None,
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
            timings: None,
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
                timings: None,
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
                timings: None,
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
