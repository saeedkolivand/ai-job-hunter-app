//! Streamed-JSON-array frame scanning: `parse_gemini_parts`,
//! `GeminiScanner`/`parse_gemini_frames`, plus the model-classification
//! predicates that gate what the scanner/body builders do.

use serde_json::json;

use super::super::super::stream::StreamPiece;
use super::super::thinking::{gemini_is_v3_or_later, gemini_supports_thinking};
use super::super::wire::{parse_gemini_frames, parse_gemini_parts, GeminiScanner};

#[test]
fn thinking_gate_enables_only_known_models() {
    for m in [
        "gemini-2.5-pro",
        "gemini-2.5-flash",
        "gemini-2.0-flash-thinking",
        // Real Gemini 3 ids match neither "2.5" nor "thinking" on their
        // own — must be reached via the v3+ boundary, not the substrings.
        "gemini-3-pro-preview",
        "gemini-3-flash-preview",
        "gemini-3.6-flash",
    ] {
        assert!(gemini_supports_thinking(m), "{m} should enable thinking");
    }
    for m in ["gemini-1.5-pro", "gemini-1.5-flash", "gemini-2.0-flash"] {
        assert!(
            !gemini_supports_thinking(m),
            "{m} must not request thinkingConfig (it 400s)"
        );
    }
}

#[test]
fn parse_parts_splits_thought_from_answer() {
    let ev = json!({
        "candidates": [{
            "content": { "parts": [
                { "text": "reasoning…", "thought": true },
                { "text": "the answer" }
            ] }
        }]
    });
    assert_eq!(
        parse_gemini_parts(&ev),
        vec![(true, "reasoning…"), (false, "the answer")]
    );
}

#[test]
fn parse_parts_empty_without_candidates() {
    assert!(parse_gemini_parts(&json!({})).is_empty());
    assert!(parse_gemini_parts(&json!({ "candidates": [] })).is_empty());
}

#[test]
fn frames_parse_a_single_object_to_pieces() {
    // A self-contained object (no array wrapper) — the scanner finds it when
    // depth returns to 0 and the accumulated text starts with `{`.
    let obj = r#"{"candidates":[{"content":{"parts":[{"text":"reasoning","thought":true},{"text":"answer"}]}}]}"#;
    let mut state = GeminiScanner::default();
    let mut buf = String::from(obj);
    let pieces = parse_gemini_frames(&mut buf, &mut state);
    assert_eq!(
        pieces,
        vec![
            StreamPiece::thinking("reasoning"),
            StreamPiece::text("answer")
        ]
    );
    // The buffer is fully consumed and no partial object remains.
    assert!(buf.is_empty());
    assert!(state.pending.is_empty());
}

#[test]
fn frames_reassemble_object_split_across_chunks() {
    // An object delivered in two chunks is buffered in `state.pending` until
    // complete, then emitted exactly once.
    let mut state = GeminiScanner::default();
    let mut buf = String::from(r#"{"candidates":[{"content":{"parts":[{"text":"hel"#);
    assert!(parse_gemini_frames(&mut buf, &mut state).is_empty());
    assert!(!state.pending.is_empty());
    buf.push_str(r#"lo"}]}}]}"#);
    assert_eq!(
        parse_gemini_frames(&mut buf, &mut state),
        vec![StreamPiece::text("hello")]
    );
}

#[test]
fn frames_handle_braces_inside_strings() {
    // Braces inside a string value must not move the depth counter.
    let obj = r#"{"candidates":[{"content":{"parts":[{"text":"a } b { c"}]}}]}"#;
    let mut state = GeminiScanner::default();
    let mut buf = String::from(obj);
    assert_eq!(
        parse_gemini_frames(&mut buf, &mut state),
        vec![StreamPiece::text("a } b { c")]
    );
}

#[test]
fn frames_emit_a_usage_piece_when_usage_metadata_is_present() {
    let obj = r#"{"candidates":[{"content":{"parts":[{"text":"answer"}]}}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":5}}"#;
    let mut state = GeminiScanner::default();
    let mut buf = String::from(obj);
    let pieces = parse_gemini_frames(&mut buf, &mut state);
    assert_eq!(pieces.len(), 2);
    assert_eq!(pieces[0], StreamPiece::text("answer"));
    let usage_piece = &pieces[1];
    assert!(usage_piece.usage.is_some());
    let usage = usage_piece.usage.unwrap();
    assert_eq!(usage.input_tokens, 10);
    assert_eq!(usage.output_tokens, 5);
}

#[test]
fn frames_emit_both_objects_in_a_json_array_payload() {
    // A realistic streamed array (`[{…},{…}]`) split across two chunks: the
    // depth-0 framing (`[`, `,`, `]`, whitespace) must be dropped so the
    // `starts_with('{')` guard fires for the second object too. Both objects'
    // text deltas must be emitted in order.
    let mut state = GeminiScanner::default();
    let mut buf =
        String::from(r#"[{"candidates":[{"content":{"parts":[{"text":"Hello"}]}}]}, {"candi"#);
    let first = parse_gemini_frames(&mut buf, &mut state);
    assert_eq!(first, vec![StreamPiece::text("Hello")]);

    buf.push_str(r#"dates":[{"content":{"parts":[{"text":" world"}]}}]}]"#);
    let second = parse_gemini_frames(&mut buf, &mut state);
    assert_eq!(second, vec![StreamPiece::text(" world")]);
    assert!(buf.is_empty());
    assert!(state.pending.is_empty());
}

#[test]
fn v3_gate_recognizes_the_current_and_future_gemini_3_family() {
    for m in [
        "gemini-3-pro-preview",
        "gemini-3-flash-preview",
        "gemini-3.1-pro-preview",
        "gemini-3.5-flash",
        "gemini-3.6-flash",
        "models/gemini-3-pro-preview",
        "gemini-4-pro", // not-yet-released — must degrade forward, not backward
    ] {
        assert!(gemini_is_v3_or_later(m), "{m} should be v3+");
    }
    for m in [
        "gemini-1.5-pro",
        "gemini-1.5-flash",
        "gemini-2.0-flash",
        "gemini-2.5-pro",
        "gemini-2.5-flash",
        "not-a-gemini-model",
    ] {
        assert!(!gemini_is_v3_or_later(m), "{m} should not be v3+");
    }
}
