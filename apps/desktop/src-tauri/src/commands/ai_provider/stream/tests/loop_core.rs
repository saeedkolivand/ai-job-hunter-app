//! `drive_stream`'s core control flow: sentinel/UTF-8-carry/cancel/error
//! handling.

use super::super::*;
use super::support::{line_parser, run, Act};

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
