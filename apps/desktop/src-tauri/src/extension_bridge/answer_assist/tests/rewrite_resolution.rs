//! `resolve_rewrite_instruction`, `assist_prompt_for_mode`, and
//! `validate_rewrite_fields` (all pure functions in the sibling
//! `answer_assist_parse.rs` — see the parent `tests` module doc for why
//! their tests stay here).

use serde_json::json;

use crate::extension_bridge::answer_assist_parse::{
    assist_prompt_for_mode, resolve_rewrite_instruction, validate_rewrite_fields,
};
use crate::extension_bridge::answer_rewrite::preset_instruction;

use super::super::{AssistMode, ANSWER_ASSIST_MAX_TOKENS, ANSWER_ASSIST_SYSTEM};

// ── assist_prompt_for_mode ───────────────────────────────────────────────

#[test]
fn assist_prompt_for_mode_selects_answer_assist_system_for_draft() {
    let (system, max_tokens) = assist_prompt_for_mode(AssistMode::Draft);
    assert_eq!(system, ANSWER_ASSIST_SYSTEM);
    assert_eq!(max_tokens, ANSWER_ASSIST_MAX_TOKENS);
}

#[test]
fn assist_prompt_for_mode_selects_rewrite_system_for_rewrite() {
    let (system, max_tokens) = assist_prompt_for_mode(AssistMode::Rewrite);
    assert_eq!(
        system,
        crate::extension_bridge::answer_rewrite::REWRITE_SYSTEM
    );
    // Same token cap as draft today — no in-app precedent to size a distinct
    // one for rewrite (see the function's own doc).
    assert_eq!(max_tokens, ANSWER_ASSIST_MAX_TOKENS);
    // The two modes must never select the SAME system prompt.
    assert_ne!(system, ANSWER_ASSIST_SYSTEM);
}

#[test]
fn resolve_rewrite_instruction_combines_a_recognized_preset_with_the_free_text() {
    // #1231 Half A — a preset chip pressed with a typed instruction must not
    // discard the typing: both intents survive, preset text first, the user's
    // own wording second.
    let resolved = resolve_rewrite_instruction(Some("shorten"), "keep the intro line").unwrap();
    assert_eq!(
        resolved,
        format!(
            "{} keep the intro line",
            preset_instruction("shorten").unwrap()
        )
    );
}

#[test]
fn resolve_rewrite_instruction_returns_a_recognized_preset_alone_when_no_free_text() {
    // No typed instruction — the preset chip's own wording verbatim, exactly
    // as before the combine change (server-authoritative, never a client copy).
    let resolved = resolve_rewrite_instruction(Some("shorten"), "").unwrap();
    assert_eq!(resolved, preset_instruction("shorten").unwrap());
}

#[test]
fn resolve_rewrite_instruction_falls_back_to_free_text_when_preset_is_unrecognized() {
    let resolved =
        resolve_rewrite_instruction(Some("not-a-real-preset"), "Make it shorter.").unwrap();
    assert_eq!(resolved, "Make it shorter.");
}

#[test]
fn resolve_rewrite_instruction_falls_back_to_free_text_when_no_preset_given() {
    let resolved = resolve_rewrite_instruction(None, "Make it shorter.").unwrap();
    assert_eq!(resolved, "Make it shorter.");
}

#[test]
fn resolve_rewrite_instruction_refuses_when_neither_preset_nor_instruction_is_usable() {
    let err = resolve_rewrite_instruction(None, "").unwrap_err();
    assert!(err
        .to_string()
        .contains("preset or instruction is required"));

    let err_unrecognized = resolve_rewrite_instruction(Some("bogus"), "").unwrap_err();
    assert!(err_unrecognized
        .to_string()
        .contains("preset or instruction is required"));
}

// ── validate_rewrite_fields ──────────────────────────────────────────────

#[test]
fn validate_rewrite_fields_rejects_an_empty_existing_answer() {
    let err = validate_rewrite_fields(&json!({ "mode": "rewrite", "existingAnswer": "   " }))
        .unwrap_err();
    assert!(err.to_string().contains("existingAnswer is required"));
}

#[test]
fn validate_rewrite_fields_rejects_a_missing_existing_answer() {
    let err = validate_rewrite_fields(&json!({ "mode": "rewrite" })).unwrap_err();
    assert!(err.to_string().contains("existingAnswer is required"));
}

#[test]
fn validate_rewrite_fields_rejects_neither_a_preset_nor_an_instruction() {
    let err = validate_rewrite_fields(&json!({
        "mode": "rewrite",
        "existingAnswer": "Because I like it."
    }))
    .unwrap_err();
    assert!(err
        .to_string()
        .contains("preset or instruction is required"));
}

#[test]
fn validate_rewrite_fields_resolves_a_recognized_preset() {
    let (existing_answer, instruction) = validate_rewrite_fields(&json!({
        "mode": "rewrite",
        "existingAnswer": "Because I like it.",
        "preset": "shorten"
    }))
    .unwrap();
    assert_eq!(existing_answer, "Because I like it.");
    assert_eq!(instruction, preset_instruction("shorten").unwrap());
}

#[test]
fn validate_rewrite_fields_falls_back_to_free_text_instruction() {
    let (existing_answer, instruction) = validate_rewrite_fields(&json!({
        "mode": "rewrite",
        "existingAnswer": "Because I like it.",
        "instruction": "Make it punchier."
    }))
    .unwrap();
    assert_eq!(existing_answer, "Because I like it.");
    assert_eq!(instruction, "Make it punchier.");
}
