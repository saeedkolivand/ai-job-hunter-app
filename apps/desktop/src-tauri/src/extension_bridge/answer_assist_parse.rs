//! `answer.assist` payload parsing, clamping and validation — the pure,
//! `AppHandle`-free half of [`super::answer_assist`]. Nothing here touches
//! the network, the store or the registry: every function is a total
//! function of the incoming wire `Value`.
//!
//! Every value these parse is UNTRUSTED (page-derived or user-typed), so the
//! byte caps and the trims are the boundary, not a convenience.

use serde_json::Value;

use crate::error::{AppError, AppResult};

use super::answer_assist::{ANSWER_ASSIST_MAX_TOKENS, ANSWER_ASSIST_SYSTEM, MAX_INSTRUCTION_BYTES};

/// Clamp `s` to at most `max` BYTES, cutting on a UTF-8 char boundary so the result stays
/// valid UTF-8. Truncates, never rejects. Shared by every bridge verb that caps page text.
pub(super) fn clamp_bytes(mut s: String, max: usize) -> String {
    s.truncate(s.floor_char_boundary(max));
    s
}

/// Clamp `s` to at most `max` CHARS (never splits a multi-byte character) —
/// used for the model's own output, which `clamp_bytes`'s byte-count framing
/// is a poor fit for (a byte cap could cut a non-ASCII draft much shorter
/// than intended).
pub(super) fn clamp_chars(s: String, max: usize) -> String {
    if s.chars().count() <= max {
        return s;
    }
    s.chars().take(max).collect()
}

// ── Request parsing ──────────────────────────────────────────────────────────

pub(super) fn parse_question(payload: &Value) -> String {
    payload
        .get("question")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

pub(super) fn parse_url(payload: &Value) -> Option<String> {
    payload
        .get("url")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub(super) fn parse_search_web(payload: &Value) -> bool {
    payload
        .get("searchWeb")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Which of the two `answer.assist` prompt paths this request drives — see
/// the module doc's "Rewrite mode" section. Anything other than the literal
/// `"rewrite"` (including a missing/unknown `mode`) is `Draft` — back-compat
/// default, matching the extension's own `mode?: 'draft' | 'rewrite'`
/// optional field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AssistMode {
    Draft,
    Rewrite,
}

pub(super) fn parse_mode(payload: &Value) -> AssistMode {
    match payload.get("mode").and_then(|v| v.as_str()) {
        Some("rewrite") => AssistMode::Rewrite,
        _ => AssistMode::Draft,
    }
}

/// The field's CURRENT text to rewrite (rewrite mode only) — page/user-
/// derived and PII-adjacent (the user's own past answer); clamped at the
/// resolve boundary like every other untrusted field here, never persisted.
pub(super) fn parse_existing_answer(payload: &Value) -> String {
    payload
        .get("existingAnswer")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// The raw quick-action preset id string (rewrite mode only), when present —
/// validated (and resolved to its instruction) by
/// [`resolve_rewrite_instruction`], not here; this just extracts whatever
/// string the client sent, unrecognized or not.
pub(super) fn parse_preset(payload: &Value) -> Option<String> {
    payload
        .get("preset")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// The free-text instruction — user-typed and untrusted, in BOTH modes: the
/// rewrite-mode instruction and the draft-mode Regenerate instruction (see
/// [`parse_draft_instruction`] which adds the boundary clamp). Trimmed here,
/// fenced the same way `existingAnswer`/`question` are.
pub(super) fn parse_instruction(payload: &Value) -> String {
    payload
        .get("instruction")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

/// The DRAFT-mode instruction at the resolve boundary: [`parse_instruction`]'s
/// trim (the SAME parse the rewrite path validates through) bounded by
/// [`clamp_bytes`] to [`MAX_INSTRUCTION_BYTES`]. Unlike the rewrite path
/// there is no required-field refusal — `instruction` is OPTIONAL for a
/// draft, so an absent or malformed field degrades to "no instruction block"
/// rather than an error.
pub(super) fn parse_draft_instruction(payload: &Value) -> String {
    clamp_bytes(parse_instruction(payload), MAX_INSTRUCTION_BYTES)
}

/// Resolve the rewrite instruction to actually send: a recognized `preset`
/// COMBINED with a non-empty free-text `instruction` (`"{preset text} {free
/// text}"`) — since #1231 the extension sends both for a preset chip pressed
/// with a typed instruction in the box, so the preset must no longer
/// silently discard the user's typing (the chip names the coarse move, the
/// typed text the specifics). The preset map stays the server-side source of
/// truth for the preset's OWN wording — never a client copy. Falls back to
/// whichever single side is present, and refuses with a fixed sentinel when
/// neither yields any text.
pub(super) fn resolve_rewrite_instruction(
    preset: Option<&str>,
    instruction: &str,
) -> AppResult<String> {
    if let Some(text) = preset.and_then(super::answer_rewrite::preset_instruction) {
        return match instruction {
            "" => Ok(text.to_string()),
            free => Ok(format!("{text} {free}")),
        };
    }
    if instruction.is_empty() {
        return Err(AppError::Validation(
            "preset or instruction is required".to_string(),
        ));
    }
    Ok(instruction.to_string())
}

/// The system prompt + max-token cap [`resolve_answer_assist`] passes to
/// [`super::stream::compose_draft_stream`] for `mode` — draft always selects
/// [`ANSWER_ASSIST_SYSTEM`]/[`ANSWER_ASSIST_MAX_TOKENS`], rewrite always
/// selects [`super::answer_rewrite::REWRITE_SYSTEM`] (same token cap — no
/// in-app precedent to size a distinct one). A PURE function so this MODE →
/// PROMPT mapping is directly unit-testable even though
/// `resolve_answer_assist` itself cannot be driven end-to-end in this crate
/// (no `tauri::test` mock-app harness).
pub(super) fn assist_prompt_for_mode(mode: AssistMode) -> (&'static str, u32) {
    match mode {
        AssistMode::Draft => (ANSWER_ASSIST_SYSTEM, ANSWER_ASSIST_MAX_TOKENS),
        AssistMode::Rewrite => (
            super::answer_rewrite::REWRITE_SYSTEM,
            ANSWER_ASSIST_MAX_TOKENS,
        ),
    }
}

/// Validate rewrite mode's required fields — `existingAnswer` non-empty and
/// a usable preset-or-instruction (via [`resolve_rewrite_instruction`]) —
/// and return `(existing_answer, instruction)` on success. A PURE function:
/// it takes only `payload`, so it is structurally INCAPABLE of touching the
/// `ai_research` limiter — calling it before `resolve_answer_assist` ever
/// acquires that limiter closes the "malformed rewrite frame burns a
/// rate-window slot at zero provider spend" gap (`limits::Limiter` never
/// releases a slot early, so a rejection AFTER acquire would still cost one).
pub(super) fn validate_rewrite_fields(payload: &Value) -> AppResult<(String, String)> {
    let existing_answer = parse_existing_answer(payload);
    if existing_answer.trim().is_empty() {
        return Err(AppError::Validation(
            "existingAnswer is required".to_string(),
        ));
    }
    let preset = parse_preset(payload);
    let instruction = resolve_rewrite_instruction(preset.as_deref(), &parse_instruction(payload))?;
    Ok((existing_answer, instruction))
}
