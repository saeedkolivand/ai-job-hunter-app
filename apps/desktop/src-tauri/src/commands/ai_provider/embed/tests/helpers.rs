//! Pure helper tests: `truncate_chars` / `next_truncation_len` /
//! `is_context_length_error` / `split_into_chunks` / `bounded_split_cap` /
//! `l2_normalize`.

use super::super::*;

#[test]
fn truncate_chars_respects_char_boundaries() {
    let text = "héllo wörld"; // multi-byte chars
    assert_eq!(truncate_chars(text, 100), text);
    assert_eq!(truncate_chars(text, 1), "h");
    assert_eq!(truncate_chars(text, 2), "hé");
}

#[test]
fn next_truncation_len_halves_down_to_the_floor_then_gives_up() {
    assert_eq!(next_truncation_len(8000), Some(4000));
    assert_eq!(next_truncation_len(4000), Some(2000));
    assert_eq!(next_truncation_len(2000), Some(1000));
    assert_eq!(next_truncation_len(1000), Some(500));
    assert_eq!(next_truncation_len(500), None);
    assert_eq!(next_truncation_len(200), None);
}

#[test]
fn is_context_length_error_matches_known_provider_wordings() {
    // The exact Ollama wording seen in production (see app log).
    assert!(is_context_length_error(
        "Ollama 500 Internal Server Error: {\"error\":\"the input length exceeds the context length\"}"
    ));
    assert!(is_context_length_error(
        "openai: this model's maximum context length is 8192 tokens"
    ));
    // OpenAI's plain "too long" wording.
    assert!(is_context_length_error("openai: Input text is too long"));
    // Gemini's real over-length wording.
    assert!(is_context_length_error(
        "gemini: the input token count (12000) exceeds the maximum number of tokens allowed (8192)."
    ));
    assert!(is_context_length_error(
        "gemini: request too large — try a smaller resume/job ad."
    ));
    assert!(!is_context_length_error(
        "gemini: invalid or unauthorized API key."
    ));
    assert!(!is_context_length_error(
        "Ollama unreachable: connection refused"
    ));
    // 404 model-not-found must never be mistaken for a length overflow.
    assert!(!is_context_length_error(
        "gemini: model or endpoint not found — models/text-embedding-004 is not found"
    ));
}

// ── split_into_chunks / l2_normalize (pure helpers) ─────────────────────

#[test]
fn split_into_chunks_covers_the_whole_text_without_dropping_a_tail() {
    let text = "a".repeat(25);
    let chunks = split_into_chunks(&text, 10);
    assert_eq!(chunks, vec!["a".repeat(10), "a".repeat(10), "a".repeat(5)]);
    // Every char of the original text is present across the chunks.
    assert_eq!(chunks.concat().chars().count(), 25);
}

#[test]
fn split_into_chunks_is_a_single_chunk_when_under_the_cap() {
    assert_eq!(split_into_chunks("short", 8000), vec!["short"]);
}

#[test]
fn split_into_chunks_respects_char_boundaries() {
    let text = "é".repeat(7); // multi-byte char, 2 bytes each
    let chunks = split_into_chunks(&text, 3);
    assert_eq!(chunks, vec!["é".repeat(3), "é".repeat(3), "é".to_string()]);
}

#[test]
fn split_into_chunks_of_empty_text_is_one_empty_chunk() {
    // So the caller still makes its usual single provider call rather
    // than zero (preserves prior single-empty-call behavior).
    assert_eq!(split_into_chunks("", 8000), vec![""]);
}

#[test]
fn bounded_split_cap_is_a_no_op_within_the_chunk_limit() {
    // 24,000 chars at cap=8000 is 3 chunks — well under the 32 ceiling.
    assert_eq!(bounded_split_cap(24_000, 8000), 8000);
}

#[test]
fn bounded_split_cap_grows_the_cap_to_stay_within_the_limit_without_dropping_text() {
    // 2 MB at cap=8000 would need 250 chunks — far over the 32 ceiling.
    let total = 2_000_000;
    let cap = bounded_split_cap(total, 8000);
    let needed = total.div_ceil(cap);
    assert!(needed <= 32, "grown cap {cap} still needs {needed} chunks");
    // The grown cap must still cover the WHOLE document across at most
    // 32 chunks — growing the chunk size, never truncating the document.
    assert!(cap * 32 >= total);
}

#[test]
fn l2_normalize_scales_to_unit_length() {
    let mut v = vec![3.0, 4.0]; // 3-4-5 triangle
    l2_normalize(&mut v);
    assert!((v[0] - 0.6).abs() < 1e-9);
    assert!((v[1] - 0.8).abs() < 1e-9);
    let norm = (v[0] * v[0] + v[1] * v[1]).sqrt();
    assert!((norm - 1.0).abs() < 1e-9);
}

#[test]
fn l2_normalize_is_a_no_op_on_an_all_zero_vector() {
    let mut v = vec![0.0, 0.0];
    l2_normalize(&mut v);
    assert_eq!(v, vec![0.0, 0.0]);
}
