//! Request-field parsing — exercises the sibling `answer_assist_parse.rs`
//! pure functions at THIS verb's own call site (they moved here, alongside
//! this verb's other tests, not into that file — see the parent `tests`
//! module doc).

use serde_json::json;

use crate::extension_bridge::answer_assist_parse::{
    clamp_bytes, clamp_chars, parse_draft_instruction, parse_existing_answer, parse_instruction,
    parse_mode, parse_preset, parse_question, parse_search_web, parse_url,
};

use super::super::budgets::{DRAFT_CAP, MAX_INSTRUCTION_BYTES, MAX_QUESTION_BYTES};
use super::super::AssistMode;

#[test]
fn parse_question_trims_and_defaults_to_empty() {
    assert_eq!(
        parse_question(&json!({ "question": "  Why this role?  " })),
        "Why this role?"
    );
    assert_eq!(parse_question(&json!({})), "");
    assert_eq!(parse_question(&json!({ "question": 42 })), "");
}

#[test]
fn parse_url_trims_drops_blank_and_defaults_to_none() {
    assert_eq!(
        parse_url(&json!({ "url": "  https://example.com/job/1  " })),
        Some("https://example.com/job/1".to_string())
    );
    assert_eq!(parse_url(&json!({ "url": "   " })), None);
    assert_eq!(parse_url(&json!({})), None);
}

#[test]
fn parse_search_web_defaults_to_false() {
    assert!(!parse_search_web(&json!({})));
    assert!(parse_search_web(&json!({ "searchWeb": true })));
    assert!(!parse_search_web(&json!({ "searchWeb": false })));
}

// ── rewrite-mode parsing (PR 11) ──────────────────────────────────────

#[test]
fn parse_mode_defaults_to_draft_for_missing_or_unknown_values() {
    assert_eq!(parse_mode(&json!({})), AssistMode::Draft);
    assert_eq!(parse_mode(&json!({ "mode": "draft" })), AssistMode::Draft);
    assert_eq!(parse_mode(&json!({ "mode": "bogus" })), AssistMode::Draft);
    assert_eq!(parse_mode(&json!({ "mode": 42 })), AssistMode::Draft);
}

#[test]
fn parse_mode_recognizes_rewrite() {
    assert_eq!(
        parse_mode(&json!({ "mode": "rewrite" })),
        AssistMode::Rewrite
    );
}

#[test]
fn parse_existing_answer_defaults_to_empty() {
    assert_eq!(
        parse_existing_answer(&json!({ "existingAnswer": "Because I love it." })),
        "Because I love it."
    );
    assert_eq!(parse_existing_answer(&json!({})), "");
    assert_eq!(parse_existing_answer(&json!({ "existingAnswer": 1 })), "");
}

#[test]
fn parse_preset_extracts_whatever_string_is_present_unvalidated() {
    assert_eq!(
        parse_preset(&json!({ "preset": "shorten" })),
        Some("shorten".to_string())
    );
    // Validation is `resolve_rewrite_instruction`'s job, not this parser's.
    assert_eq!(
        parse_preset(&json!({ "preset": "not-a-real-preset" })),
        Some("not-a-real-preset".to_string())
    );
    assert_eq!(parse_preset(&json!({})), None);
}

#[test]
fn parse_instruction_trims_and_defaults_to_empty() {
    assert_eq!(
        parse_instruction(&json!({ "instruction": "  Make it punchier.  " })),
        "Make it punchier."
    );
    assert_eq!(parse_instruction(&json!({})), "");
}

#[test]
fn parse_draft_instruction_trims_defaults_to_empty_and_bounds_at_the_resolve_boundary() {
    // The SAME parse the rewrite path validates through, plus the draft
    // boundary clamp — empty/malformed degrades to "no block", never an error.
    assert_eq!(parse_draft_instruction(&json!({})), "");
    assert_eq!(
        parse_draft_instruction(&json!({ "instruction": "  short  " })),
        "short"
    );
    // Byte cap holds for ASCII and multi-byte input alike (no char cut mid-UTF-8).
    let huge = "x".repeat(MAX_INSTRUCTION_BYTES + 200);
    assert_eq!(
        parse_draft_instruction(&json!({ "instruction": huge })),
        "x".repeat(MAX_INSTRUCTION_BYTES)
    );
    let huge_mb = "é".repeat((MAX_INSTRUCTION_BYTES / 2) + 100); // 2 bytes/char
    assert!(
        parse_draft_instruction(&json!({ "instruction": huge_mb })).len() <= MAX_INSTRUCTION_BYTES
    );
    assert!(std::str::from_utf8(
        parse_draft_instruction(&json!({ "instruction": huge_mb })).as_bytes()
    )
    .is_ok());
}

// ── clamp helpers ─────────────────────────────────────────────────────

#[test]
fn clamp_bytes_cuts_on_a_char_boundary() {
    let huge = "x".repeat(MAX_QUESTION_BYTES + 50);
    let clamped = clamp_bytes(huge, MAX_QUESTION_BYTES);
    assert_eq!(clamped.len(), MAX_QUESTION_BYTES);
    // 5 bytes lands inside the third 2-byte "é": the cut backs off to the boundary at 4.
    assert_eq!(clamp_bytes("é".repeat(10), 5), "éé");
}

#[test]
fn clamp_chars_counts_characters_not_bytes() {
    let huge = "é".repeat(DRAFT_CAP + 10); // 2 bytes/char in UTF-8
    let clamped = clamp_chars(huge, DRAFT_CAP);
    assert_eq!(clamped.chars().count(), DRAFT_CAP);
}
