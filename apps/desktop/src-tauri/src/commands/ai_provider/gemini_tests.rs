//! Unit tests for `gemini.rs`, split into this sibling file (R8 line-budget
//! split — mirrors the `anthropic.rs` + `anthropic_tests.rs` precedent of
//! moving the test module itself out rather than production code).
//!
//! Wired via `#[path = "gemini_tests.rs"] mod tests;` in `gemini.rs` — that
//! keeps this a CHILD module of `gemini` in the module tree (same as an
//! inline `#[cfg(test)] mod tests { ... }` block), so the imports below
//! still reach every private item there, while this file's own filename
//! (ending `tests.rs`) excludes it from the architecture test's R8 LOC cap
//! (`tests/architecture.rs`'s `is_test` filename check) and from R3/R6's
//! non-test scans.

use super::{
    build_chat_stream_body, build_embed_body, gemini_effective_temperature, gemini_effort_levels,
    gemini_is_v3_or_later, gemini_supports_thinking, join_parts_text, parse_gemini_embed_usage,
    parse_gemini_frames, parse_gemini_parts, parse_gemini_turn, parse_gemini_usage,
    validate_gemini_key, AiProvider, GeminiClient, GeminiScanner, StreamPiece,
    EMBED_OUTPUT_DIMENSIONALITY,
};
use crate::commands::ai_provider::{AiGenerateRequest, StopReason, ToolCall};
use crate::error::AppError;
use crate::ipc_contracts::ai::AiGenerateRequestMessage;
use serde_json::json;

fn base_request() -> AiGenerateRequest {
    AiGenerateRequest {
        model: "gemini-1.5-flash".to_string(),
        messages: vec![AiGenerateRequestMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
        }],
        locale: "en".to_string(),
        temperature: Some(0.8),
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        repeat_penalty: None,
        max_tokens: None,
        context_window: None,
        effort: None,
    }
}

#[test]
fn chat_stream_body_serializes_sampling_params_when_set() {
    let mut req = base_request();
    req.top_p = Some(0.95);
    req.frequency_penalty = Some(0.3);
    req.presence_penalty = Some(0.2);
    let body = build_chat_stream_body(&req);
    let config = &body["generationConfig"];
    assert_eq!(config["topP"], json!(0.95));
    assert_eq!(config["frequencyPenalty"], json!(0.3));
    assert_eq!(config["presencePenalty"], json!(0.2));
}

#[test]
fn gemini_effective_temperature_omits_only_for_v3_with_no_explicit_value() {
    // Explicit value ALWAYS wins, on every model — never overridden.
    assert_eq!(
        gemini_effective_temperature("gemini-3.6-flash", Some(0.3), 0.7),
        Some(0.3)
    );
    assert_eq!(
        gemini_effective_temperature("gemini-1.5-flash", Some(0.3), 0.7),
        Some(0.3)
    );
    // No explicit value, pre-v3: keeps the caller's fallback (unchanged
    // behavior — Google's don't-touch-1.0 guidance is scoped to Gemini 3+).
    assert_eq!(
        gemini_effective_temperature("gemini-1.5-flash", None, 0.7),
        Some(0.7)
    );
    // No explicit value, v3+: omit entirely — `ai.google.dev/gemini-api/docs/gemini-3`
    // (fetched 2026-08-04) warns a below-1.0 value risks looping/degraded
    // performance on complex reasoning tasks; never invent one.
    assert_eq!(
        gemini_effective_temperature("gemini-3.6-flash", None, 0.7),
        None
    );
}

#[test]
fn chat_stream_body_omits_temperature_for_a_v3_model_with_no_explicit_value() {
    let mut req = base_request();
    req.model = "gemini-3.6-flash".to_string();
    req.temperature = None;
    let body = build_chat_stream_body(&req);
    assert!(
        body["generationConfig"].get("temperature").is_none(),
        "must not invent a temperature for a v3+ model — let the API apply its own 1.0"
    );
}

#[test]
fn chat_stream_body_sends_an_explicit_temperature_even_on_a_v3_model() {
    let mut req = base_request();
    req.model = "gemini-3.6-flash".to_string();
    req.temperature = Some(0.3);
    let body = build_chat_stream_body(&req);
    assert_eq!(
        body["generationConfig"]["temperature"],
        json!(0.3),
        "a deliberate user value must still be honored on a v3+ model"
    );
}

#[test]
fn chat_stream_body_keeps_the_default_temperature_for_a_pre_v3_model_with_no_explicit_value() {
    let mut req = base_request();
    req.model = "gemini-1.5-flash".to_string();
    req.temperature = None;
    let body = build_chat_stream_body(&req);
    assert_eq!(
        body["generationConfig"]["temperature"],
        json!(0.7),
        "unchanged behavior for pre-v3 models"
    );
}

#[test]
fn chat_stream_body_omits_sampling_params_when_none() {
    let body = build_chat_stream_body(&base_request());
    let config = &body["generationConfig"];
    assert!(config.get("topP").is_none());
    assert!(config.get("frequencyPenalty").is_none());
    assert!(config.get("presencePenalty").is_none());
}

#[test]
fn blank_or_missing_key_is_rejected_with_unauthorized() {
    // A missing key, an empty string, and whitespace-only must all fail fast with
    // the same unauthorized message `friendly_api_error` maps a real 401/403 to —
    // never sending an empty `x-goog-api-key` header for a wasted round-trip.
    for stored in [None, Some(String::new()), Some("   \n\t".to_string())] {
        match validate_gemini_key(stored) {
            Err(AppError::Config(msg)) => {
                assert_eq!(msg, "gemini: invalid or unauthorized API key.")
            }
            other => panic!("expected unauthorized Config error, got {other:?}"),
        }
    }
}

#[test]
fn present_key_passes_through_untrimmed() {
    // A real key is returned verbatim (surrounding content preserved, only blank
    // rejected) so the request uses exactly what the user stored.
    assert_eq!(
        validate_gemini_key(Some("AIza-secret".to_string())).unwrap(),
        "AIza-secret"
    );
}

#[test]
fn default_embedding_model_is_not_the_retired_text_embedding_004() {
    // text-embedding-004 was retired by Google (shutdown Jan 14, 2026) —
    // the exact "model or endpoint not found" error this app was seeing.
    let model = GeminiClient.default_embedding_model().unwrap();
    assert_ne!(model, "text-embedding-004");
    assert_eq!(model, "gemini-embedding-2");
}

#[test]
fn embed_body_requests_the_reduced_output_dimensionality() {
    // Without this, gemini-embedding-2 defaults to 3072 dims (4x the
    // retired text-embedding-004's 768), quadrupling stored-vector size
    // for no accuracy benefit this app uses. Must be NESTED inside
    // `embedContentConfig` (camelCase) — the top-level `outputDimensionality`
    // field is deprecated per the live REST reference and may be silently
    // ignored.
    let body = build_embed_body("gemini-embedding-2", "hello");
    assert_eq!(
        body["embedContentConfig"]["outputDimensionality"],
        json!(EMBED_OUTPUT_DIMENSIONALITY)
    );
    assert_eq!(EMBED_OUTPUT_DIMENSIONALITY, 768);
    // Never at the deprecated top-level location.
    assert!(body.get("output_dimensionality").is_none());
    assert!(body.get("outputDimensionality").is_none());
    assert_eq!(body["model"], json!("models/gemini-embedding-2"));
    assert_eq!(body["content"]["parts"][0]["text"], json!("hello"));
}

#[test]
fn embedding_cap_is_within_the_documented_token_limit_range() {
    // Drift-pinning only, NOT proof of token safety on its own (that would
    // need a real tokenizer, which this crate doesn't have) — it just
    // pins the char cap to a sane range relative to gemini-embedding-2's
    // documented 8,192-token limit, so a future edit can't silently set
    // it absurdly high or low. The real per-language safety net is
    // `embed_chunk_adaptive`'s halve-and-retry on an actual provider
    // context-length error, which this test does not exercise.
    let cap = GeminiClient.max_embedding_input_chars();
    assert!(cap <= 8192, "char cap {cap} can exceed 8192 tokens");
    assert!(cap >= 4_000, "cap {cap} truncates too aggressively");
}

#[test]
fn join_parts_text_concatenates_first_candidate_parts() {
    let data = json!({
        "candidates": [{
            "content": { "parts": [{ "text": "Acme is " }, { "text": "a widget maker." }] },
            "groundingMetadata": { "webSearchQueries": ["Acme"] }
        }]
    });
    assert_eq!(join_parts_text(&data), "Acme is a widget maker.");
    assert_eq!(join_parts_text(&json!({})), "");
    assert_eq!(join_parts_text(&json!({ "candidates": [] })), "");
}

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
fn parse_usage_reads_prompt_and_candidates_token_counts() {
    let data = json!({ "usageMetadata": { "promptTokenCount": 55, "candidatesTokenCount": 22, "totalTokenCount": 77 } });
    let usage = parse_gemini_usage(&data).expect("usage present");
    assert_eq!(usage.input_tokens, 55);
    assert_eq!(usage.output_tokens, 22);
}

#[test]
fn parse_usage_is_none_when_absent() {
    assert!(parse_gemini_usage(&json!({})).is_none());
}

#[test]
fn parse_embed_usage_reads_prompt_token_count_when_present() {
    let data = json!({ "usageMetadata": { "promptTokenCount": 7 } });
    let usage = parse_gemini_embed_usage(&data);
    assert_eq!(usage.input_tokens, 7);
    assert_eq!(usage.output_tokens, 0);
}

#[test]
fn parse_embed_usage_zero_when_absent() {
    // Gemini's embedContent response typically carries no usageMetadata —
    // must degrade to zero, never fabricate a token count.
    let usage = parse_gemini_embed_usage(&json!({ "embedding": { "values": [0.1] } }));
    assert_eq!(usage.input_tokens, 0);
    assert_eq!(usage.output_tokens, 0);
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
fn parse_turn_extracts_function_calls_alongside_text() {
    // Gemini reports finishReason "STOP" even when it emits a functionCall — the
    // call's presence, not the finishReason, is the "wants tools back" signal.
    let data = json!({
        "candidates": [{
            "content": { "parts": [
                { "text": "Looking up the company." },
                { "functionCall": { "name": "research_company", "args": { "company": "Acme" } } }
            ] },
            "finishReason": "STOP"
        }]
    });
    let turn = parse_gemini_turn(&data);
    assert_eq!(turn.text, "Looking up the company.");
    assert_eq!(turn.stop, StopReason::ToolUse);
    assert_eq!(
        turn.tool_calls,
        vec![ToolCall {
            id: "research_company-1".to_string(),
            name: "research_company".to_string(),
            args: json!({ "company": "Acme" }),
        }]
    );
}

#[test]
fn parse_turn_plain_answer_maps_stop_reason() {
    let data = json!({
        "candidates": [{
            "content": { "parts": [{ "text": "Final answer." }] },
            "finishReason": "STOP"
        }]
    });
    let turn = parse_gemini_turn(&data);
    assert_eq!(turn.text, "Final answer.");
    assert!(turn.tool_calls.is_empty());
    assert_eq!(turn.stop, StopReason::End);

    let truncated = json!({
        "candidates": [{ "content": { "parts": [{ "text": "..." }] }, "finishReason": "MAX_TOKENS" }]
    });
    assert_eq!(parse_gemini_turn(&truncated).stop, StopReason::Length);
}

#[test]
fn parse_turn_malformed_function_call_maps_to_length_not_tool_use() {
    // A tool call truncated by the output-token limit comes back with
    // `finishReason: "MALFORMED_FUNCTION_CALL"` (NOT `MAX_TOKENS`) — it must
    // route through the same non-executable/truncated path as `MAX_TOKENS`, so
    // the (possibly half-serialized) args never reach a tool handler.
    let data = json!({
        "candidates": [{
            "content": { "parts": [
                { "functionCall": { "name": "research_company", "args": { "company": "Ac" } } }
            ] },
            "finishReason": "MALFORMED_FUNCTION_CALL"
        }]
    });
    let turn = parse_gemini_turn(&data);
    assert_eq!(turn.stop, StopReason::Length);
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

#[test]
fn capabilities_supports_reasoning_mirrors_the_v3_gate() {
    assert!(
        GeminiClient
            .capabilities("gemini-3-pro-preview")
            .supports_reasoning
    );
    assert!(
        !GeminiClient
            .capabilities("gemini-2.5-pro")
            .supports_reasoning
    );
}

#[test]
fn chat_stream_body_sends_thinking_level_for_a_v3_model_with_effort_set() {
    // gemini-3.1-pro-preview — LIVE, Preview status
    // (`ai.google.dev/gemini-api/docs/models`, checked 2026-08-04).
    let mut req = base_request();
    req.model = "gemini-3.1-pro-preview".to_string();
    req.effort = Some("low".to_string());
    let body = build_chat_stream_body(&req);
    assert_eq!(
        body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
        json!("LOW")
    );
}

#[test]
fn chat_stream_body_omits_thinking_level_invalid_for_the_current_model_tier() {
    // The reported model-switch scenario: `effort` is stored PER PROVIDER
    // (`preferences-store.ts`), not per model, and nothing clears it on a
    // model switch. "medium" is valid for gemini-3.1-pro-preview but NOT
    // for gemini-3.1-flash-lite-image (minimal/high only — LIVE, Stable
    // status, `ai.google.dev/gemini-api/docs/models`, checked
    // 2026-08-04) — both are Gemini 3+, so gating on
    // `gemini_is_v3_or_later` alone would ship an invalid level and 400.
    // Must omit `thinkingLevel` entirely rather than send a level the
    // CURRENT model rejects.
    let mut req = base_request();
    req.model = "gemini-3.1-flash-lite-image".to_string();
    req.effort = Some("medium".to_string());
    let body = build_chat_stream_body(&req);
    assert!(
        body["generationConfig"]["thinkingConfig"]
            .get("thinkingLevel")
            .is_none(),
        "medium is invalid for gemini-3.1-flash-lite-image (minimal/high only) — must not be sent"
    );
}

#[test]
fn chat_stream_body_omits_thinking_level_for_a_pre_v3_model_even_with_effort_set() {
    // `thinkingLevel` is documented "Recommended for Gemini 3 or later
    // models. Use with earlier models results in an error" on THIS
    // endpoint's own REST reference — a pre-v3 model (2.5 and earlier)
    // must never get thinkingLevel/thinkingBudget.
    let mut req = base_request();
    req.model = "gemini-2.5-pro".to_string();
    req.effort = Some("low".to_string());
    let body = build_chat_stream_body(&req);
    assert!(body["generationConfig"]["thinkingConfig"]
        .get("thinkingLevel")
        .is_none());
}

#[test]
fn chat_stream_body_omits_thinking_config_for_a_non_thinking_model_with_no_effort() {
    let mut req = base_request();
    // Pre-v3, non-2.5, non-"*-thinking-*" — genuinely no thinking support.
    req.model = "gemini-2.0-flash".to_string();
    let body = build_chat_stream_body(&req);
    assert!(body["generationConfig"].get("thinkingConfig").is_none());
}

#[test]
fn chat_stream_body_keeps_include_thoughts_alongside_thinking_level() {
    // A real Gemini 3 id matches BOTH the (now-widened) "thinking" display
    // gate and the v3 effort gate — both fields must land in the SAME
    // thinkingConfig object, not one clobbering the other. "high" is the
    // one level every row in `gemini_effort_levels`'s table accepts, so
    // it's valid for gemini-3.1-pro-preview specifically too.
    // gemini-3.1-pro-preview — LIVE, Preview status
    // (`ai.google.dev/gemini-api/docs/models`, checked 2026-08-04).
    let mut req = base_request();
    req.model = "gemini-3.1-pro-preview".to_string();
    req.effort = Some("high".to_string());
    let body = build_chat_stream_body(&req);
    let tc = &body["generationConfig"]["thinkingConfig"];
    assert_eq!(tc["includeThoughts"], json!(true));
    assert_eq!(tc["thinkingLevel"], json!("HIGH"));
}

#[test]
fn effort_levels_are_looked_up_per_model_tier_not_per_provider() {
    // gemini-3-pro-preview is SHUT DOWN (checked 2026-08-04) — this
    // assertion is intentional, not stale: it locks in the row's kept
    // historical value (see the doc comment on `gemini_effort_levels`),
    // not a claim the model is selectable.
    assert_eq!(
        gemini_effort_levels("gemini-3-pro-preview"),
        vec!["low", "high"]
    );
    assert_eq!(
        gemini_effort_levels("gemini-3.1-pro-preview"),
        vec!["low", "medium", "high"]
    );
    assert_eq!(
        gemini_effort_levels("gemini-3.1-flash-lite-image"),
        vec!["minimal", "high"]
    );
    assert_eq!(
        gemini_effort_levels("gemini-3-flash-preview"),
        vec!["minimal", "low", "medium", "high"]
    );
    assert_eq!(
        gemini_effort_levels("gemini-3.6-flash"),
        vec!["minimal", "low", "medium", "high"]
    );
    // `gemini-3.1-flash-lite` (the TEXT model, distinct from `-image`) has no
    // row in the live thinking table — this locks in the safe universal
    // fallback so a future "fix" doesn't silently guess it belongs in the
    // full-level branch (see the doc comment above `gemini_effort_levels`).
    assert_eq!(gemini_effort_levels("gemini-3.1-flash-lite"), vec!["high"]);
    // An unrecognized future v3+ id falls back to the one universally-safe
    // level, never a guess that could 400.
    assert_eq!(gemini_effort_levels("gemini-4-pro"), vec!["high"]);
    // Pre-v3 models get no levels at all.
    assert!(gemini_effort_levels("gemini-2.5-pro").is_empty());
    assert!(gemini_effort_levels("gemini-1.5-flash").is_empty());
}

#[test]
fn capabilities_effort_levels_matches_the_free_function() {
    assert_eq!(
        GeminiClient.effort_levels("gemini-3-pro-preview"),
        gemini_effort_levels("gemini-3-pro-preview")
    );
}
