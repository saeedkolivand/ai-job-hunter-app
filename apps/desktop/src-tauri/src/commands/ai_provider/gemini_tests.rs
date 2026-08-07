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
    gemini_is_v3_or_later, gemini_omits_sampling_params, gemini_supports_thinking, join_parts_text,
    parse_gemini_embed_usage, parse_gemini_frames, parse_gemini_parts, parse_gemini_turn,
    parse_gemini_usage, parse_model_page, resolve_intent, validate_gemini_key, AiProvider,
    GeminiClient, GeminiScanner, Intent, SamplingProfile, StreamPiece, DETERMINISTIC_TEMPERATURE,
    EMBED_OUTPUT_DIMENSIONALITY, PROSE_FREQUENCY_PENALTY, PROSE_GROUNDED_TEMPERATURE,
    PROSE_PRESENCE_PENALTY, PROSE_TEMPERATURE, PROSE_TOP_P,
};
use crate::commands::ai_provider::{AiGenerateRequest, StopReason, ToolCall};
use crate::error::AppError;
use crate::ipc_contracts::ai::AiGenerateRequestMessage;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A minimal raw-socket HTTP/1.1 server that writes each response's status
/// line + headers immediately, then sleeps `body_delay` BEFORE writing that
/// response's body. `wiremock::ResponseTemplate::set_delay` cannot build
/// this: its delay elapses entirely BEFORE any bytes (headers included)
/// reach the socket, so it only ever exercises `req.send()`'s own timeout —
/// never a `resp.text()`/`resp.json()` blocked on a body that's slow to
/// arrive AFTER `send()` already resolved, which is the actual gap a
/// cumulative deadline has to cover. Serves `bodies` in order, one per
/// accepted connection (`Connection: close`, so each page fetch opens a new
/// one) — `bodies[i].1` is that page's body-write delay.
async fn slow_body_server(bodies: Vec<(String, Duration)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        for (body, body_delay) in bodies {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            // Drain the request up to the blank line ending its headers —
            // contents don't matter, only connection ORDER distinguishes pages.
            let mut buf = [0u8; 4096];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") => break,
                    Ok(_) => continue,
                }
            }
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(headers.as_bytes()).await;
            let _ = socket.flush().await;
            if !body_delay.is_zero() {
                tokio::time::sleep(body_delay).await;
            }
            let _ = socket.write_all(body.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });
    format!("http://{addr}")
}

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
        intent: None,
    }
}

/// Mirrors exactly what `GeminiClient::chat_stream` does: resolve this
/// adapter's own profile for `req.model` + `req.intent`, then merge with the
/// request's explicit numeric overrides.
fn sampling_for(req: &AiGenerateRequest) -> SamplingProfile {
    GeminiClient
        .sampling_profile(&req.model, resolve_intent(req))
        .resolve(req)
}

#[test]
fn chat_stream_body_serializes_sampling_params_when_set() {
    let mut req = base_request();
    req.top_p = Some(0.95);
    req.frequency_penalty = Some(0.3);
    req.presence_penalty = Some(0.2);
    let body = build_chat_stream_body(&req, sampling_for(&req));
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
    let body = build_chat_stream_body(&req, sampling_for(&req));
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
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["generationConfig"]["temperature"],
        json!(0.3),
        "a deliberate user value must still be honored on a v3+ model"
    );
}

#[test]
fn chat_stream_body_uses_the_deterministic_target_for_a_pre_v3_model_with_no_explicit_value() {
    // No `req.intent` set → `Intent::Default`, which resolves to the SAME
    // numbers as `Intent::Deterministic` on an accepting model (see
    // `Intent`'s own doc comment, `commands/ai_provider/mod.rs`) — this
    // reproduces the pre-fix renderer's own hardcoded default (`0.3` for the
    // majority of deterministic surfaces), NOT the adapter's old standalone
    // `0.7` fallback (that fallback's job is now done by this profile).
    let mut req = base_request();
    req.model = "gemini-1.5-flash".to_string();
    req.temperature = None;
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["generationConfig"]["temperature"],
        json!(DETERMINISTIC_TEMPERATURE)
    );
}

#[test]
fn chat_stream_body_uses_the_deterministic_target_for_a_non_gemini_prefixed_pre_v3_model() {
    // `gemma-3-27b-it` is what a REAL non-`gemini-`-prefixed pre-v3 id looks
    // like on this provider (`parse_model_page` surfaces it into the model
    // picker exactly like a `gemini-*` id — see `gemini_is_pre_v3`'s doc
    // comment). It must reach the wire with the SAME deterministic
    // temperature as a `gemini-`-prefixed pre-v3 model, not silently drop to
    // neutral.
    let mut req = base_request();
    req.model = "gemma-3-27b-it".to_string();
    req.temperature = None;
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["generationConfig"]["temperature"],
        json!(DETERMINISTIC_TEMPERATURE)
    );
}

#[test]
fn gemini_omits_sampling_params_matches_the_v3_gate() {
    // ONE predicate decides temperature AND topP (and topK, if this file
    // ever wires one up) — pinned directly so the two parameters can't
    // drift apart the way `topP` silently did before this fix.
    assert!(gemini_omits_sampling_params("gemini-3.6-flash"));
    assert!(gemini_omits_sampling_params("gemini-3-pro-preview"));
    assert!(!gemini_omits_sampling_params("gemini-1.5-flash"));
    assert!(!gemini_omits_sampling_params("gemini-2.5-pro"));
}

#[test]
fn chat_stream_body_omits_top_p_for_a_v3_model_even_when_explicit() {
    // Unlike `temperature`, `topP` has no "deliberate user intent" to
    // preserve — it's the renderer's own anti-detection knob (useless on a
    // model that ignores it), so a v3+ model omits it unconditionally, even
    // when the caller set one.
    let mut req = base_request();
    req.model = "gemini-3.6-flash".to_string();
    req.top_p = Some(0.95);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body["generationConfig"].get("topP").is_none(),
        "topP must never reach a v3+ model, even when explicitly set"
    );
}

#[test]
fn chat_stream_body_sends_top_p_for_a_pre_v3_model_when_explicit() {
    let mut req = base_request();
    req.model = "gemini-1.5-flash".to_string();
    req.top_p = Some(0.95);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["generationConfig"]["topP"],
        json!(0.95),
        "unchanged behavior for pre-v3 models"
    );
}

#[test]
fn chat_stream_body_omits_sampling_params_when_none() {
    let req = base_request();
    let body = build_chat_stream_body(&req, sampling_for(&req));
    let config = &body["generationConfig"];
    assert!(config.get("topP").is_none());
    assert!(config.get("frequencyPenalty").is_none());
    assert!(config.get("presencePenalty").is_none());
}

#[test]
fn sampling_profile_is_fully_neutral_on_a_v3_model_for_every_intent() {
    // Hard constraint: a Gemini 3+ model receives NO temperature and NO
    // top_p, even under `Intent::Deterministic` — Google's deprecation
    // notice is unconditional, not intent-scoped.
    for intent in [
        Intent::Deterministic,
        Intent::Prose,
        Intent::ProseGrounded,
        Intent::Default,
    ] {
        let profile = GeminiClient.sampling_profile("gemini-3.6-flash", intent);
        assert_eq!(
            profile,
            SamplingProfile::default(),
            "{intent:?} must stay neutral on a v3+ model"
        );
    }
}

#[test]
fn sampling_profile_declares_real_values_per_intent_on_a_recognized_pre_v3_model() {
    // "gemini-1.5-flash" fails `gemini_is_v3_or_later`, so it is pre-v3 —
    // it declares REAL values reproducing this app's pre-fix shipped numbers,
    // the same [`DETERMINISTIC_TEMPERATURE`]/[`PROSE_TEMPERATURE`]/
    // [`PROSE_GROUNDED_TEMPERATURE`] targets every other accepting adapter
    // uses (this app's pre-fix renderer sent identical numbers to Gemini as
    // every other cloud provider).
    let model = "gemini-1.5-flash";
    assert_eq!(
        GeminiClient.sampling_profile(model, Intent::Deterministic),
        SamplingProfile {
            temperature: Some(DETERMINISTIC_TEMPERATURE),
            ..SamplingProfile::default()
        }
    );
    assert_eq!(
        GeminiClient.sampling_profile(model, Intent::Prose),
        SamplingProfile {
            temperature: Some(PROSE_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            frequency_penalty: Some(PROSE_FREQUENCY_PENALTY),
            presence_penalty: Some(PROSE_PRESENCE_PENALTY),
            ..SamplingProfile::default()
        }
    );
    assert_eq!(
        GeminiClient.sampling_profile(model, Intent::ProseGrounded),
        SamplingProfile {
            temperature: Some(PROSE_GROUNDED_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            frequency_penalty: Some(PROSE_FREQUENCY_PENALTY),
            ..SamplingProfile::default()
        },
        "prose_grounded must never send presence_penalty"
    );
    // `Default` (no declared intent) resolves the same as `Deterministic`.
    assert_eq!(
        GeminiClient.sampling_profile(model, Intent::Default),
        GeminiClient.sampling_profile(model, Intent::Deterministic)
    );
}

#[test]
fn sampling_profile_declares_real_values_for_a_non_gemini_prefixed_pre_v3_model() {
    // `parse_model_page` filters Google's `/v1beta/models` listing only on
    // the `models/` wrapper prefix, not on family, so a `gemma-*` id reaches
    // this app's model picker exactly like a `gemini-*` one and is a real
    // Google model served by the same endpoint — it must get the SAME
    // deterministic profile as a `gemini-`-prefixed pre-v3 model, not fall
    // back to neutral. (A previous version of `gemini_is_pre_v3` required
    // the literal `gemini-` prefix and silently dropped these to neutral —
    // see that function's own doc comment for why that was wrong and
    // reverted.)
    let model = "gemma-3-27b-it";
    assert_eq!(
        GeminiClient.sampling_profile(model, Intent::Deterministic),
        SamplingProfile {
            temperature: Some(DETERMINISTIC_TEMPERATURE),
            ..SamplingProfile::default()
        }
    );
}

#[test]
fn gemini_effective_temperature_uses_fallback_for_a_non_gemini_prefixed_model() {
    // `gemini_effective_temperature`'s own gate stays on the wider
    // `gemini_omits_sampling_params` (v3+ only, via `gemini_is_v3_or_later`)
    // — a non-"gemini-"-prefixed id that isn't v3+ still gets its caller's
    // `fallback` here. `GeminiClient::sampling_profile`'s own gate
    // (`gemini_is_pre_v3`) is now the exact same v3 boundary, so the two no
    // longer diverge for any model id.
    assert_eq!(
        gemini_effective_temperature("totally-unrecognized-model-id", None, 0.7),
        Some(0.7)
    );
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
fn present_key_is_returned_unchanged_when_already_clean() {
    assert_eq!(
        validate_gemini_key(Some("AIza-secret".to_string())).unwrap(),
        "AIza-secret"
    );
}

#[test]
fn validate_gemini_key_trims_the_returned_key_not_just_the_checked_one() {
    // A pasted key with a trailing space/newline must reach the
    // `x-goog-api-key` header TRIMMED — checking `k.trim().is_empty()` but
    // returning the padded `k` is the exact bug: a trailing space just
    // 401s, an embedded `\n` makes the header value invalid and the request
    // never builds at all.
    assert_eq!(
        validate_gemini_key(Some(" AIza-secret \n".to_string())).unwrap(),
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
    let body = build_chat_stream_body(&req, sampling_for(&req));
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
    let body = build_chat_stream_body(&req, sampling_for(&req));
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
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(body["generationConfig"]["thinkingConfig"]
        .get("thinkingLevel")
        .is_none());
}

#[test]
fn chat_stream_body_omits_thinking_config_for_a_non_thinking_model_with_no_effort() {
    let mut req = base_request();
    // Pre-v3, non-2.5, non-"*-thinking-*" — genuinely no thinking support.
    req.model = "gemini-2.0-flash".to_string();
    let body = build_chat_stream_body(&req, sampling_for(&req));
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
    let body = build_chat_stream_body(&req, sampling_for(&req));
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

// ── list_models ──────────────────────────────────────────────────────────────

#[test]
fn parse_model_page_strips_the_models_prefix() {
    let body = json!({
        "models": [
            { "name": "models/gemini-3-pro" },
            { "name": "models/gemini-2.5-flash" },
            { "name": "tunedModels/not-a-real-model" },
        ]
    });
    let (page, cursor) = parse_model_page(&body).unwrap();
    let names: Vec<String> = page
        .into_iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["gemini-3-pro", "gemini-2.5-flash"]);
    assert_eq!(cursor, None);
}

#[test]
fn parse_model_page_does_not_filter_out_preview_ids() {
    // Regression guard for the `/v1` -> `/v1beta` switch: `/v1beta` is the ONLY
    // version that lists `-preview`/experimental models (e.g. the curated
    // Pro-tier default in `provider-meta.ts` is a `-preview` id) — the parser
    // must never re-introduce a filter that drops them.
    let body = json!({
        "models": [{ "name": "models/gemini-3.1-pro-preview" }]
    });
    let (page, _) = parse_model_page(&body).unwrap();
    assert_eq!(page, vec![json!({ "name": "gemini-3.1-pro-preview" })]);
}

#[test]
fn parse_model_page_populates_display_name_and_context_length_when_present() {
    let body = json!({
        "models": [{
            "name": "models/gemini-3-pro",
            "displayName": "Gemini 3 Pro",
            "inputTokenLimit": 1_000_000,
        }]
    });
    let (page, _) = parse_model_page(&body).unwrap();
    assert_eq!(
        page,
        vec![json!({
            "name": "gemini-3-pro",
            "displayName": "Gemini 3 Pro",
            "contextLength": 1_000_000,
        })]
    );
}

#[test]
fn parse_model_page_never_populates_created_at_gemini_does_not_return_one() {
    // Gemini's `/v1beta/models` reports no creation timestamp at all —
    // `createdAt` must be ABSENT from the entry, never defaulted to some
    // sentinel, even when every other optional field IS present.
    let body = json!({
        "models": [{
            "name": "models/gemini-3-pro",
            "displayName": "Gemini 3 Pro",
            "inputTokenLimit": 1_000_000,
        }]
    });
    let (page, _) = parse_model_page(&body).unwrap();
    assert!(page[0].get("createdAt").is_none());
}

#[test]
fn parse_model_page_omits_optional_fields_the_provider_does_not_return_and_keeps_name_unchanged() {
    // `name` must stay byte-identical to the pre-widening shape — a stored
    // model preference matches against it.
    let body = json!({ "models": [{ "name": "models/gemini-2.5-flash" }] });
    let (page, _) = parse_model_page(&body).unwrap();
    assert_eq!(page, vec![json!({ "name": "gemini-2.5-flash" })]);
}

#[test]
fn parse_model_page_ok_empty_on_genuinely_empty_catalogue() {
    let body = json!({ "models": [] });
    let (page, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(page, Vec::<Value>::new());
    assert_eq!(cursor, None);
}

#[test]
fn parse_model_page_errors_when_models_field_is_missing() {
    let body = json!({ "unexpected": "shape" });
    assert!(matches!(
        parse_model_page(&body),
        Err(AppError::Provider(_))
    ));
}

#[test]
fn parse_model_page_carries_a_non_empty_next_page_token() {
    let body = json!({
        "models": [{ "name": "models/gemini-2.5-flash" }],
        "nextPageToken": "page-2-token",
    });
    let (_, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(cursor, Some("page-2-token".to_string()));
}

#[test]
fn parse_model_page_treats_an_empty_next_page_token_as_the_last_page() {
    let body = json!({
        "models": [{ "name": "models/gemini-2.5-flash" }],
        "nextPageToken": "",
    });
    let (_, cursor) = parse_model_page(&body).unwrap();
    assert_eq!(cursor, None);
}

// `advance_cursor`/`PaginationStep`/`pagination_step` are shared, generic
// helpers now — see `ai_provider::mod`'s test module for their coverage.
// Duplicating them here per-adapter is exactly the "a rule implemented
// twice that silently stops agreeing" defect class this codebase keeps
// paying for; one copy, one set of tests.

// ── list_models_transport (wiremock — the pagination HTTP loop itself,
// not just pagination_step's pure decision) ─────────────────────────────────

#[tokio::test]
async fn list_models_transport_propagates_the_cursor_into_the_next_requests_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-page1-token",
        })))
        .mount(&server)
        .await;
    // Only matches when `pageToken` carries EXACTLY page 1's
    // `nextPageToken` — proves the token is wired from the parsed response
    // into the next request's query, not just decided in the abstract by
    // `pagination_step`.
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param("pageToken", "gemini-page1-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-2.5-flash" }],
        })))
        .mount(&server)
        .await;

    let models = GeminiClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect("both pages must be fetched, the token propagated between them");
    assert_eq!(
        models,
        vec![
            json!({ "name": "gemini-3-pro" }),
            json!({ "name": "gemini-2.5-flash" }),
        ]
    );
}

#[tokio::test]
async fn list_models_transport_propagates_a_mid_pagination_status_error_instead_of_the_pages_already_collected(
) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-page1-token",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param("pageToken", "gemini-page1-token"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = GeminiClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err(
            "a failure on page 2 must reject the whole fetch, never return page 1's models alone",
        );
    // 500 classifies as Network via `friendly_api_error` — not the point of
    // this test, but asserted so a future change that silently swallows
    // page 2's error can't slip through.
    assert!(matches!(err, AppError::Network(_)));
}

#[tokio::test]
async fn list_models_transport_errors_when_page_two_is_malformed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-page1-token",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param("pageToken", "gemini-page1-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "unexpected": "shape" })))
        .mount(&server)
        .await;

    let err = GeminiClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err("a malformed page 2 body must reject the whole fetch");
    assert!(matches!(err, AppError::Provider(_)));
}

#[tokio::test]
async fn list_models_transport_preserves_provider_context_on_a_non_json_page_two_body() {
    // Regression pin: the `.await??` rewrite in 70738c63 dropped the
    // "{name}: parse: " context and flipped this from `Provider` to `Parse`
    // (`error.rs`'s `From<serde_json::Error>`) — a change to the renderer-
    // visible error the sweep never intended. Unlike the "malformed shape"
    // test above (valid JSON, wrong fields — caught by `parse_model_page`),
    // this body isn't JSON at all, so it fails inside `read_json_capped`
    // itself, exercising the exact line that regressed.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-page1-token",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param("pageToken", "gemini-page1-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json at all"))
        .mount(&server)
        .await;

    let err = GeminiClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err("a non-JSON page 2 body must reject the whole fetch");
    assert!(matches!(err, AppError::Provider(_)));
    assert_eq!(
        err.to_string(),
        "gemini: parse: response body did not match the expected schema"
    );
}

#[tokio::test]
async fn list_models_transport_errors_when_the_provider_reports_a_stalled_token() {
    // A `nextPageToken` present but identical across two pages: the
    // provider claims more data exists but gives no way to reach it. The
    // exact regression this whole finding is about — silently treating this
    // as `Done` and returning the one page collected as `Ok` — must not
    // happen at the transport level either, not just in the pure
    // `pagination_step` unit tests.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-stuck",
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .and(query_param("pageToken", "gemini-stuck"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "models/gemini-3-pro" }],
            "nextPageToken": "gemini-stuck",
        })))
        .mount(&server)
        .await;

    let err = GeminiClient
        .list_models_transport(&server.uri(), "dummy-key", Duration::from_secs(30))
        .await
        .expect_err("a stalled token must reject, never silently return the partial catalogue");
    assert!(matches!(err, AppError::Provider(_)));
}

#[tokio::test]
async fn list_models_transport_errors_when_the_cumulative_deadline_fires_across_multiple_pages() {
    // Page 1 responds instantly (well within budget) and reports a token.
    // Page 2 writes its HEADERS instantly too — `send()` resolves fast — but
    // stalls its BODY well past the REMAINING budget. A deadline that only
    // wraps `send()` (the pre-fix bug) would let this succeed late instead
    // of erroring; only wrapping the body reads too catches it. Verified
    // (before applying that fix) that this test genuinely fails against the
    // unfixed transport — it doesn't error at all, it just returns `Ok`
    // after the full body delay, which is the exact silent-non-enforcement
    // bug this test exists to catch.
    let page1 = json!({
        "models": [{ "name": "models/gemini-3-pro" }],
        "nextPageToken": "gemini-page1-token",
    })
    .to_string();
    let page2 = json!({ "models": [{ "name": "models/gemini-2.5-flash" }] }).to_string();
    let base = slow_body_server(vec![
        (page1, Duration::ZERO),
        (page2, Duration::from_millis(100)),
    ])
    .await;

    let err = GeminiClient
        .list_models_transport(&base, "dummy-key", Duration::from_millis(40))
        .await
        .expect_err("page 2's body delay must exceed the REMAINING cumulative budget after page 1");
    assert!(matches!(err, AppError::Network(_)));
}
