//! Shared fixtures for `stream`'s test topics: the `drive_stream` action
//! collector and a trivial newline-delimited parser.

use std::cell::Cell;

use super::super::*;

/// Collect the sink actions `drive_stream` produces for a canned chunk list.
/// Each piece is identified by `(emit:delta/thinking, complete, cancelled, error)`.
/// `Complete` carries the final [`Usage`] AND the accumulated answer text
/// (non-thinking deltas only) that `finish` persists — see the usage- and
/// answer-tracking tests below.
#[derive(Debug, PartialEq)]
pub(super) enum Act {
    Emit(String, bool),
    Complete(Usage, String),
    Cancelled(Usage),
    Error(String, Usage),
}

pub(super) fn run(
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
pub(super) fn line_parser(buf: &mut String) -> Vec<StreamPiece> {
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
