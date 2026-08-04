//! Unit tests for `openai.rs`, split into this sibling file (R8 line-budget
//! split — mirrors the `anthropic.rs` + `anthropic_tests.rs` precedent of
//! moving the test module itself out rather than production code).
//!
//! Wired via `#[path = "openai_tests.rs"] mod tests;` in `openai.rs` — that
//! keeps this a CHILD module of `openai` in the module tree (same as an
//! inline `#[cfg(test)] mod tests { ... }` block), so the imports below
//! still reach every private item there, while this file's own filename
//! (ending `tests.rs`) excludes it from the architecture test's R8 LOC cap
//! (`tests/architecture.rs`'s `is_test` filename check) and from R3/R6's
//! non-test scans.

use super::{
    build_chat_stream_body, is_gpt5_or_later_reasoning_family, is_reasoning_model,
    join_responses_text, parse_openai_delta, parse_openai_embed_usage, parse_openai_frames,
    parse_openai_turn, parse_openai_usage, should_list_model, OpenAiClient,
};
use crate::commands::ai_provider::{
    AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, StopReason, TokenParam, ToolCall,
};
use crate::ipc_contracts::ai::AiGenerateRequestMessage;
use serde_json::json;

fn base_request() -> AiGenerateRequest {
    AiGenerateRequest {
        model: "gpt-4o".to_string(),
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

fn chat_caps(supports_temperature: bool) -> ModelCapabilities {
    ModelCapabilities {
        supports_temperature,
        supports_system_role: true,
        supports_streaming: true,
        supports_reasoning: !supports_temperature,
        supports_tools: true,
        supports_json_mode: true,
        supports_embeddings: true,
        supports_web_search: false,
        token_param: TokenParam::MaxTokens,
    }
}

#[test]
fn chat_stream_body_always_requests_streamed_usage() {
    // AI-spend visibility depends on this flag being sent on every OpenAI
    // Chat Completions stream (native, OpenAI-compatible, and Ollama Cloud).
    let body = build_chat_stream_body(&base_request(), chat_caps(true));
    assert_eq!(body["stream_options"], json!({ "include_usage": true }));
}

#[test]
fn parse_usage_extracts_real_token_counts() {
    let data = json!({ "usage": { "prompt_tokens": 42, "completion_tokens": 17 } });
    let usage = parse_openai_usage(&data).expect("usage present");
    assert_eq!(usage.input_tokens, 42);
    assert_eq!(usage.output_tokens, 17);
}

#[test]
fn parse_usage_is_none_when_absent() {
    // Every streamed chunk except the final one has no `usage` field.
    assert!(parse_openai_usage(&json!({ "choices": [] })).is_none());
    assert!(parse_openai_usage(&json!({})).is_none());
}

#[test]
fn parse_embed_usage_prefers_prompt_tokens() {
    let data = json!({ "usage": { "prompt_tokens": 12, "total_tokens": 12 } });
    let usage = parse_openai_embed_usage(&data);
    assert_eq!(usage.input_tokens, 12);
    assert_eq!(usage.output_tokens, 0, "an embed call has no output tokens");
}

#[test]
fn parse_embed_usage_falls_back_to_total_tokens() {
    // Some OpenAI-compatible embed servers send only `total_tokens`.
    let data = json!({ "usage": { "total_tokens": 9 } });
    assert_eq!(parse_openai_embed_usage(&data).input_tokens, 9);
}

#[test]
fn parse_embed_usage_zero_when_absent() {
    let usage = parse_openai_embed_usage(&json!({}));
    assert_eq!(usage.input_tokens, 0);
    assert_eq!(usage.output_tokens, 0);
}

#[test]
fn chat_stream_body_serializes_sampling_params_when_set() {
    let mut req = base_request();
    req.top_p = Some(0.95);
    req.frequency_penalty = Some(0.3);
    req.presence_penalty = Some(0.2);
    let body = build_chat_stream_body(&req, chat_caps(true));
    assert_eq!(body["top_p"], json!(0.95));
    assert_eq!(body["frequency_penalty"], json!(0.3));
    assert_eq!(body["presence_penalty"], json!(0.2));
}

#[test]
fn chat_stream_body_omits_sampling_params_when_none() {
    let body = build_chat_stream_body(&base_request(), chat_caps(true));
    assert!(body.get("top_p").is_none());
    assert!(body.get("frequency_penalty").is_none());
    assert!(body.get("presence_penalty").is_none());
}

#[test]
fn chat_stream_body_skips_sampling_params_on_reasoning_models() {
    // o-series models reject `temperature` entirely — sampling knobs must be
    // skipped alongside it, never sent to a model that 400s on them.
    let mut req = base_request();
    req.top_p = Some(0.95);
    req.frequency_penalty = Some(0.3);
    req.presence_penalty = Some(0.2);
    let body = build_chat_stream_body(&req, chat_caps(false));
    assert!(body.get("temperature").is_none());
    assert!(body.get("top_p").is_none());
    assert!(body.get("frequency_penalty").is_none());
    assert!(body.get("presence_penalty").is_none());
}

#[test]
fn embedding_cap_is_token_safe_for_every_language() {
    // text-embedding-3-* hard-error past 8191 tokens. Token-dense scripts (CJK)
    // run ≈1 char/token, so the char cap must itself stay under 8191 — otherwise
    // a full-cap CJK input exceeds the token limit and the request FAILS.
    let cap = OpenAiClient::new(ProviderId::OpenAi, None).max_embedding_input_chars();
    assert!(
        cap <= 8191,
        "char cap {cap} can exceed 8191 tokens for ~1-char/token languages"
    );
    // Sanity: still a useful amount of text (not collapsed to near-zero).
    assert!(cap >= 4_000, "cap {cap} truncates too aggressively");
}

#[test]
fn list_filter_only_restricts_native_openai() {
    // Native OpenAI exposes a large non-chat catalog — keep only chat families.
    assert!(should_list_model(ProviderId::OpenAi, "gpt-4o"));
    assert!(should_list_model(ProviderId::OpenAi, "o3-mini"));
    assert!(should_list_model(ProviderId::OpenAi, "chatgpt-4o-latest"));
    for non_chat in ["text-embedding-3-small", "dall-e-3", "whisper-1", "tts-1"] {
        assert!(
            !should_list_model(ProviderId::OpenAi, non_chat),
            "{non_chat} should be filtered out for native OpenAI"
        );
    }

    // Ollama Cloud + generic OpenAI-compatible servers return their own
    // curated catalog under arbitrary names — never filter those, so the
    // full Ollama Cloud list (not just gpt-oss:*) reaches the picker.
    for id in [
        "gpt-oss:120b",
        "qwen3-coder:480b",
        "deepseek-v3.1:671b",
        "kimi-k2:1t",
        "glm-4.6",
    ] {
        assert!(should_list_model(ProviderId::OllamaCloud, id), "{id}");
        assert!(should_list_model(ProviderId::OpenAiCompatible, id), "{id}");
    }
}

#[test]
fn join_responses_text_takes_message_items_only() {
    // The Responses `output` array interleaves the web_search_call with the
    // final assistant message.
    let data = json!({
        "output": [
            { "type": "web_search_call", "id": "ws_1", "status": "completed" },
            { "type": "message", "role": "assistant", "content": [
                { "type": "output_text", "text": "Acme is a ", "annotations": [] },
                { "type": "output_text", "text": "widget maker.", "annotations": [] }
            ]}
        ]
    });
    assert_eq!(join_responses_text(&data), "Acme is a widget maker.");
    assert_eq!(join_responses_text(&json!({})), "");
    assert_eq!(join_responses_text(&json!({ "output": [] })), "");
}

#[test]
fn detects_o_series_including_future_models() {
    for m in ["o1", "o1-mini", "o3", "o3-mini", "o4-mini", "o5", "o9-pro"] {
        assert!(is_reasoning_model(m), "{m} should be a reasoning model");
    }
    for m in [
        "gpt-4o",
        "gpt-4o-mini",
        "gpt-3.5-turbo",
        "omni",
        "chatgpt-4o",
    ] {
        assert!(
            !is_reasoning_model(m),
            "{m} should not be a reasoning model"
        );
    }
}

#[test]
fn parse_delta_splits_reasoning_from_content() {
    // DeepSeek-R1 / vLLM style: reasoning on `reasoning_content`.
    let ev = json!({ "choices": [{ "delta": { "reasoning_content": "let me think" } }] });
    assert_eq!(parse_openai_delta(&ev), ("let me think", ""));

    // OpenRouter style: reasoning on `reasoning`.
    let ev = json!({ "choices": [{ "delta": { "reasoning": "pondering" } }] });
    assert_eq!(parse_openai_delta(&ev), ("pondering", ""));

    // Normal answer content.
    let ev = json!({ "choices": [{ "delta": { "content": "the answer" } }] });
    assert_eq!(parse_openai_delta(&ev), ("", "the answer"));
}

#[test]
fn parse_delta_empty_when_no_choices_or_fields() {
    assert_eq!(parse_openai_delta(&json!({})), ("", ""));
    assert_eq!(
        parse_openai_delta(&json!({ "choices": [{ "delta": {} }] })),
        ("", "")
    );
}

#[test]
fn parse_frames_splits_sse_lines_into_pieces() {
    use super::StreamPiece;
    // Two complete data lines (reasoning then content) + a partial trailing line.
    let mut buf = String::from(
        "data: {\"choices\":[{\"delta\":{\"reasoning\":\"think\"}}]}\n\
         data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\
         data: {\"choices\":[{\"delta\":{\"con",
    );
    let pieces = parse_openai_frames(&mut buf);
    assert_eq!(
        pieces,
        vec![StreamPiece::thinking("think"), StreamPiece::text("hello")]
    );
    // The incomplete final line is left buffered for the next chunk.
    assert!(buf.starts_with("data: {\"choices\""));
    assert!(!buf.contains('\n'));
}

#[test]
fn parse_frames_emits_done_sentinel_on_done_marker() {
    use super::StreamPiece;
    let mut buf = String::from(
        "data: {\"choices\":[{\"delta\":{\"content\":\"last\"}}]}\n\
         data: [DONE]\n",
    );
    let pieces = parse_openai_frames(&mut buf);
    assert_eq!(
        pieces,
        vec![StreamPiece::text("last"), StreamPiece::done("")]
    );
}

#[test]
fn parse_frames_skips_non_data_and_unparseable_lines() {
    // Comment/keepalive lines and malformed JSON are ignored, not errors.
    let mut buf = String::from(": keepalive\ndata: not-json\n\n");
    assert!(parse_openai_frames(&mut buf).is_empty());
}

#[test]
fn parse_turn_decodes_tool_calls_with_stringified_arguments() {
    // Chat Completions puts function args in a JSON *string* — it must be decoded.
    let data = json!({
        "choices": [{
            "message": {
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": { "name": "match_resume", "arguments": "{\"resumeId\":\"r1\",\"jobId\":\"j1\"}" }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    });
    let turn = parse_openai_turn(&data);
    assert_eq!(turn.text, "");
    assert_eq!(turn.stop, StopReason::ToolUse);
    assert_eq!(
        turn.tool_calls,
        vec![ToolCall {
            id: "call_1".to_string(),
            name: "match_resume".to_string(),
            args: json!({ "resumeId": "r1", "jobId": "j1" }),
        }]
    );
}

#[test]
fn parse_turn_plain_answer_has_no_tool_calls() {
    let data = json!({
        "choices": [{ "message": { "content": "Here is the answer." }, "finish_reason": "stop" }]
    });
    let turn = parse_openai_turn(&data);
    assert_eq!(turn.text, "Here is the answer.");
    assert!(turn.tool_calls.is_empty());
    assert_eq!(turn.stop, StopReason::End);
}

#[test]
fn parse_turn_malformed_arguments_degrade_to_empty_object() {
    // A truncated/invalid arguments string must not error the whole turn.
    let data = json!({
        "choices": [{
            "message": { "tool_calls": [{ "id": "c", "function": { "name": "f", "arguments": "{not json" } }] },
            "finish_reason": "tool_calls"
        }]
    });
    let turn = parse_openai_turn(&data);
    assert_eq!(turn.tool_calls.len(), 1);
    assert_eq!(turn.tool_calls[0].args, json!({}));
}

// ── web_search_transport (wiremock against `crate::net::http::shared()`,
// mirroring the pattern in `retry.rs`'s `retry_loop_tests`) ────────────────

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn web_search_transport_degrades_to_empty_on_http_500() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let text = client
        .web_search_transport("dummy-key", "gpt-4o", "system", "user")
        .await
        .expect("never an error, only degrades to empty");
    assert_eq!(text, "");
}

#[tokio::test]
async fn web_search_transport_degrades_to_empty_on_non_json_200() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let text = client
        .web_search_transport("dummy-key", "gpt-4o", "system", "user")
        .await
        .expect("never an error, only degrades to empty");
    assert_eq!(text, "");
}

#[tokio::test]
async fn web_search_transport_extracts_text_from_a_realistic_responses_payload() {
    let server = MockServer::start().await;
    let payload = json!({
        "output": [
            { "type": "web_search_call", "id": "ws_1", "status": "completed" },
            { "type": "message", "role": "assistant", "content": [
                { "type": "output_text", "text": "Acme is a ", "annotations": [] },
                { "type": "output_text", "text": "widget maker.", "annotations": [] }
            ]}
        ]
    });
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let text = client
        .web_search_transport("dummy-key", "gpt-4o", "system", "user")
        .await
        .expect("ok");
    assert_eq!(text, "Acme is a widget maker.");
}

#[test]
fn supports_web_search_gate_only_allows_native_openai() {
    // Regression guard against silently dropping the provider gate in a
    // future refactor: a non-OpenAI id must never reach `/responses` (a
    // generic OpenAI-compatible gateway can't be assumed to support the
    // native `web_search` tool). `web_search_complete` itself can't be
    // driven end to end here — it needs a live `AppHandle`, and this crate
    // has no `tauri::test` mock-app harness (see its doc comment, and the
    // same note on `salary_research::SalaryResearch::enrich`) — so this
    // exercises the pure gate predicate it's built on before any HTTP call.
    assert!(OpenAiClient::new(ProviderId::OpenAi, None).supports_web_search());
    for other in [
        ProviderId::OpenAiCompatible,
        ProviderId::OllamaCloud,
        ProviderId::Ollama,
        ProviderId::Anthropic,
        ProviderId::Gemini,
    ] {
        assert!(
            !OpenAiClient::new(other, None).supports_web_search(),
            "{other:?} must not pass the web_search gate"
        );
    }
}

/// `ModelCapabilities::supports_web_search` (what `ai_research_answer` gates
/// the daily-budget charge on) must mirror the private gate predicate above —
/// this is the field a caller actually reads.
#[test]
fn capabilities_supports_web_search_mirrors_the_gate() {
    assert!(
        OpenAiClient::new(ProviderId::OpenAi, None)
            .capabilities("gpt-4o")
            .supports_web_search
    );
    assert!(
        !OpenAiClient::new(ProviderId::OpenAiCompatible, None)
            .capabilities("some-model")
            .supports_web_search
    );
}

#[test]
fn reasoning_effort_gate_differs_by_provider_id_and_model_catalog() {
    // Native OpenAI: the legacy o-series AND the current gpt-5.x line.
    assert!(OpenAiClient::new(ProviderId::OpenAi, None).supports_reasoning_effort("o3-mini"));
    assert!(!OpenAiClient::new(ProviderId::OpenAi, None).supports_reasoning_effort("gpt-4o"));
    for m in [
        "gpt-5",
        "gpt-5-mini",
        "gpt-5.4",
        "gpt-5.5",
        "gpt-5.6",
        "gpt-5.6-sol",
    ] {
        assert!(
            OpenAiClient::new(ProviderId::OpenAi, None).supports_reasoning_effort(m),
            "{m} should accept reasoning_effort"
        );
    }
    // The `-chat-latest` variant of each gpt-5.x generation is the
    // non-reasoning conversational sibling — must stay excluded.
    for m in ["gpt-5-chat-latest", "gpt-5.1-chat-latest"] {
        assert!(
            !OpenAiClient::new(ProviderId::OpenAi, None).supports_reasoning_effort(m),
            "{m} must not accept reasoning_effort"
        );
    }
    // Ollama Cloud: the Ollama thinking-family catalog, NOT the o-series/gpt-5
    // rule — gpt-oss doesn't match either, and qwen3-coder must stay excluded.
    assert!(
        OpenAiClient::new(ProviderId::OllamaCloud, None).supports_reasoning_effort("gpt-oss:120b")
    );
    assert!(!OpenAiClient::new(ProviderId::OllamaCloud, None)
        .supports_reasoning_effort("qwen3-coder:480b"));
    // A generic OpenAI-compatible gateway is an unknown catalog — never guessed.
    assert!(
        !OpenAiClient::new(ProviderId::OpenAiCompatible, None).supports_reasoning_effort("o3-mini")
    );
}

#[test]
fn gpt5_family_gate_excludes_earlier_majors_and_the_chat_latest_siblings() {
    for m in [
        "gpt-4o",
        "gpt-4o-mini",
        "gpt-4-turbo",
        "gpt-3.5-turbo",
        "gpt-5-chat-latest",
        "gpt-5.1-chat-latest",
        "gpt-5.2-chat-latest",
        "gpt-5.3-chat-latest",
        "not-a-gpt-model",
    ] {
        assert!(
            !is_gpt5_or_later_reasoning_family(m),
            "{m} should not be gpt-5+ reasoning"
        );
    }
    for m in [
        "gpt-5",
        "gpt-5-mini",
        "gpt-5-nano",
        "gpt-5.1",
        "gpt-5.1-codex",
        "gpt-5.4",
        "gpt-5.6-sol",
        "gpt-5.6-terra",
        "gpt-5.6-luna",
        "gpt-6", // not-yet-released — must degrade forward, not backward
    ] {
        assert!(
            is_gpt5_or_later_reasoning_family(m),
            "{m} should be gpt-5+ reasoning"
        );
    }
}

#[test]
fn effort_levels_mirror_the_reasoning_gate() {
    assert_eq!(
        OpenAiClient::new(ProviderId::OpenAi, None).effort_levels("gpt-5.6"),
        vec!["low", "medium", "high"]
    );
    assert!(OpenAiClient::new(ProviderId::OpenAi, None)
        .effort_levels("gpt-4o")
        .is_empty());
}

#[test]
fn chat_stream_body_sends_reasoning_effort_for_a_reasoning_capable_model() {
    let mut req = base_request();
    req.model = "o3-mini".to_string();
    req.effort = Some("high".to_string());
    let caps = ModelCapabilities {
        supports_reasoning: true,
        ..chat_caps(false)
    };
    let body = build_chat_stream_body(&req, caps);
    assert_eq!(body["reasoning_effort"], json!("high"));
}

#[test]
fn chat_stream_body_omits_reasoning_effort_for_a_non_reasoning_model() {
    let mut req = base_request();
    req.model = "gpt-4o".to_string();
    req.effort = Some("high".to_string());
    let caps = ModelCapabilities {
        supports_reasoning: false,
        ..chat_caps(true)
    };
    let body = build_chat_stream_body(&req, caps);
    assert!(body.get("reasoning_effort").is_none());
}

#[test]
fn chat_stream_body_omits_reasoning_effort_when_not_set() {
    let req = base_request();
    let caps = ModelCapabilities {
        supports_reasoning: true,
        ..chat_caps(true)
    };
    let body = build_chat_stream_body(&req, caps);
    assert!(body.get("reasoning_effort").is_none());
}
