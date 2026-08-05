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
    join_responses_text, parse_model_list, parse_openai_delta, parse_openai_embed_usage,
    parse_openai_frames, parse_openai_turn, parse_openai_usage, resolve_openai_key,
    scrub_url_secret, should_list_model, OpenAiClient,
};
use crate::commands::ai_provider::{
    AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, StopReason, TokenParam, ToolCall,
};
use crate::error::AppError;
use crate::ipc_contracts::ai::AiGenerateRequestMessage;
use serde_json::{json, Value};

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

use wiremock::matchers::{header, method, path, query_param};
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

// ── list_models_transport (same wiremock pattern as web_search_transport) ──────

#[tokio::test]
async fn list_models_transport_errors_on_http_500() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let err = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect_err("a non-success status must reject, never silently degrade to empty");
    // Routed through `friendly_api_error`, not a bare `AppError::Provider` —
    // 500 classifies as `Network` (retriable), distinct from a 401 (`Config`,
    // bad key) or a 429 (`Network`, rate limit). This IS the PR's premise:
    // once the curated fallback is gone, this classification is the user's
    // only explanation.
    assert!(matches!(err, AppError::Network(_)));
}

#[tokio::test]
async fn list_models_transport_classifies_401_as_a_config_error_not_a_bare_provider_error() {
    // `test_key`'s status branch (not independently testable here — it needs
    // a live `AppHandle` this crate has no mock harness for) constructs its
    // error via the identical `friendly_api_error(self.id, status,
    // &body_text)` call on the same `list_models_request` transport, so this
    // covers both by construction.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let err = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect_err("a 401 must reject");
    assert!(matches!(err, AppError::Config(_)));
}

#[tokio::test]
async fn list_models_transport_ok_with_the_native_openai_filter_applied() {
    let server = MockServer::start().await;
    let payload = json!({
        "data": [{ "id": "gpt-4o" }, { "id": "text-embedding-3-small" }]
    });
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payload))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let models = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect("ok");
    assert_eq!(models, vec![json!({ "name": "gpt-4o" })]);
}

#[tokio::test]
async fn list_models_transport_ok_empty_on_a_genuinely_empty_catalogue() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [] })))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let models = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect("ok");
    assert_eq!(models, Vec::<Value>::new());
}

#[tokio::test]
async fn list_models_transport_errors_on_a_non_json_200_body() {
    // The captive-portal case: a 200 whose body is HTML, not JSON — a
    // self-hosted gateway behind a broken proxy/auth wall can return this.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>captive portal</html>"))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let err = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect_err("a non-JSON 200 body must reject, never silently degrade to empty");
    assert!(matches!(err, AppError::Provider(_)));
}

#[tokio::test]
async fn list_models_transport_errors_on_a_bare_array_200_body() {
    // Well-formed JSON, but not the `{ "data": [...] }` envelope — a deployment
    // that returns the array directly rather than wrapping it.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{ "id": "gpt-4o" }])))
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let err = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect_err("a bare-array body must reject, not silently return an empty list");
    assert!(matches!(err, AppError::Provider(_)));
}

#[tokio::test]
async fn list_models_transport_succeeds_with_no_key_for_a_keyless_deployment() {
    // A keyless `OpenAiCompatible` deployment (LM Studio, vLLM, …) must still
    // be able to list — and must never send an empty `Authorization: Bearer`
    // header (some gateways reject a malformed header rather than ignoring it).
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(|req: &wiremock::Request| !req.headers.contains_key("authorization"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "data": [{ "id": "local-model" }] })),
        )
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAiCompatible, Some(server.uri()));
    let models = client
        .list_models_transport(None)
        .await
        .expect("a keyless request must still reach the mock (no Authorization header required)");
    assert_eq!(models, vec![json!({ "name": "local-model" })]);
}

#[tokio::test]
async fn list_models_transport_sends_the_bearer_header_when_a_key_is_present() {
    // The `Some(key)` sibling of the keyless test above — every other
    // `list_models_transport` mock in this file never REQUIRES an
    // `authorization` header, so a regression that dropped `bearer_auth`
    // entirely (e.g. accidentally hardcoding `None`) would keep every one of
    // them green. This one requires the header to be present with the exact
    // value, closing that gap.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header("authorization", "Bearer dummy-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "data": [{ "id": "gpt-4o" }] })),
        )
        .mount(&server)
        .await;

    let client = OpenAiClient::new(ProviderId::OpenAi, Some(server.uri()));
    let models = client
        .list_models_transport(Some("dummy-key"))
        .await
        .expect("a keyed request must send the bearer header the mock requires");
    assert_eq!(models, vec![json!({ "name": "gpt-4o" })]);
}

// ── endpoint_url / scrub_url_secret (query-string-auth gateway safety) ─────────
//
// Some OpenAI-compatible gateways (Cloudflare AI Gateway, several self-hosted
// proxies) authenticate via the base URL's own query string, e.g.
// `https://gw.example.com/v1?api-key=SECRET`. Plain string concatenation
// (`format!("{base}/{path}")`) sends that case to the WRONG path (the query
// value gets `/path` appended to it) and, because reqwest's own error
// `Display` embeds the request URL verbatim, echoes the secret into any
// error text built from it.

#[test]
fn endpoint_url_preserves_a_query_carrying_base_url() {
    let client = OpenAiClient::new(
        ProviderId::OpenAiCompatible,
        Some("https://gw.example.com/v1?api-key=SECRET123".to_string()),
    );
    let url = client.endpoint_url("models").unwrap();
    assert_eq!(
        url.as_str(),
        "https://gw.example.com/v1/models?api-key=SECRET123"
    );
}

#[test]
fn endpoint_url_resolves_the_same_path_regardless_of_a_trailing_slash_on_the_base() {
    let no_slash = OpenAiClient::new(
        ProviderId::OpenAiCompatible,
        Some("https://gw.example.com/v1?api-key=SECRET123".to_string()),
    )
    .endpoint_url("models")
    .unwrap();
    let with_slash = OpenAiClient::new(
        ProviderId::OpenAiCompatible,
        Some("https://gw.example.com/v1/?api-key=SECRET123".to_string()),
    )
    .endpoint_url("models")
    .unwrap();
    assert_eq!(no_slash, with_slash);
}

#[test]
fn endpoint_url_joins_a_multi_segment_path() {
    let client = OpenAiClient::new(ProviderId::OpenAi, None);
    let url = client.endpoint_url("chat/completions").unwrap();
    assert_eq!(url.as_str(), "https://api.openai.com/v1/chat/completions");
}

#[test]
fn endpoint_url_preserves_a_fragment() {
    let client = OpenAiClient::new(
        ProviderId::OpenAiCompatible,
        Some("https://gw.example.com/v1#frag".to_string()),
    );
    let url = client.endpoint_url("models").unwrap();
    assert_eq!(url.as_str(), "https://gw.example.com/v1/models#frag");
}

#[test]
fn endpoint_url_errors_on_an_unparseable_base_url() {
    let client = OpenAiClient::new(ProviderId::OpenAiCompatible, Some("not a url".to_string()));
    assert!(matches!(
        client.endpoint_url("models"),
        Err(AppError::Config(_))
    ));
}

#[tokio::test]
async fn scrub_url_secret_removes_the_query_but_keeps_scheme_host_path() {
    // A genuine transport failure (not a synthetic error — `reqwest::Error`
    // has no public constructor) against an unreachable loopback port, so the
    // resulting error carries a real, reqwest-attached URL.
    // Built through `net::http::shared()`, not `reqwest::Client::new()` — R5
    // confines client construction to `net/http.rs`, and this test is more
    // faithful for it: the error being scrubbed comes from the same client
    // the production path uses.
    let url = reqwest::Url::parse("http://127.0.0.1:1/v1/models?api-key=SECRET123").unwrap();
    let err = crate::net::http::shared()
        .get(url)
        .timeout(std::time::Duration::from_millis(500))
        .send()
        .await
        .expect_err("port 1 must be unreachable");
    let text = scrub_url_secret(err).to_string();
    assert!(
        !text.contains("SECRET123"),
        "query secret must not survive scrubbing: {text}"
    );
    assert!(
        text.contains("127.0.0.1:1/v1/models"),
        "scheme/host/path must stay visible for diagnosing a wrong-path bug: {text}"
    );
}

#[tokio::test]
async fn list_models_transport_reaches_the_correct_path_with_a_query_carrying_base_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(query_param("api-key", "SECRET123"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "data": [{ "id": "gw-model" }] })),
        )
        .mount(&server)
        .await;

    let base = format!("{}/v1?api-key=SECRET123", server.uri());
    let client = OpenAiClient::new(ProviderId::OpenAiCompatible, Some(base));
    let models = client
        .list_models_transport(None)
        .await
        .expect("a query-carrying base URL must resolve to the correct path with the query intact");
    assert_eq!(models, vec![json!({ "name": "gw-model" })]);
}

#[tokio::test]
async fn list_models_transport_never_leaks_the_query_secret_on_a_network_failure() {
    let client = OpenAiClient::new(
        ProviderId::OpenAiCompatible,
        Some("http://127.0.0.1:1/v1?api-key=SECRET123".to_string()),
    );
    let err = client
        .list_models_transport(None)
        .await
        .expect_err("port 1 must be unreachable");
    let text = err.to_string();
    assert!(
        !text.contains("SECRET123"),
        "the resolved-but-unreachable query-carrying URL must not leak its secret: {text}"
    );
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

#[test]
fn chat_stream_body_omits_reasoning_effort_outside_the_verified_level_set() {
    // OpenAI's real `reasoning_effort` enum has grown to 7 values (see
    // `OPENAI_EFFORT_LEVELS`'s doc comment) but this adapter only exposes the
    // 3 verified-universal ones — a stale/unrecognized value (e.g. carried
    // over from a DIFFERENT provider's richer picker, since `effort` is
    // stored per provider, not per model) must never be sent through just
    // because `caps.supports_reasoning` is true.
    let mut req = base_request();
    req.model = "o3-mini".to_string();
    req.effort = Some("xhigh".to_string());
    let caps = ModelCapabilities {
        supports_reasoning: true,
        ..chat_caps(false)
    };
    let body = build_chat_stream_body(&req, caps);
    assert!(
        body.get("reasoning_effort").is_none(),
        "xhigh is outside OPENAI_EFFORT_LEVELS — must not be sent"
    );
}

#[test]
fn effort_levels_returns_the_verified_universal_set_for_every_reasoning_model() {
    assert_eq!(
        OpenAiClient::new(ProviderId::OpenAi, None).effort_levels("o3-mini"),
        vec!["low", "medium", "high"]
    );
    assert_eq!(
        OpenAiClient::new(ProviderId::OpenAi, None).effort_levels("gpt-5.6"),
        vec!["low", "medium", "high"]
    );
    assert!(OpenAiClient::new(ProviderId::OpenAi, None)
        .effort_levels("gpt-4o")
        .is_empty());
}

// ── list_models ──────────────────────────────────────────────────────────────

#[test]
fn resolve_openai_key_errors_on_missing_or_blank_key_for_native_openai() {
    assert!(matches!(
        resolve_openai_key(ProviderId::OpenAi, None),
        Err(AppError::Config(_))
    ));
    assert!(matches!(
        resolve_openai_key(ProviderId::OpenAi, Some("  ".to_string())),
        Err(AppError::Config(_))
    ));
}

#[test]
fn resolve_openai_key_errors_on_missing_key_for_ollama_cloud() {
    // Only `OpenAiCompatible` gets the keyless exemption — Ollama Cloud is a
    // hosted cloud service (via the same composed client) and still requires
    // its account key.
    assert!(matches!(
        resolve_openai_key(ProviderId::OllamaCloud, None),
        Err(AppError::Config(_))
    ));
}

#[test]
fn resolve_openai_key_accepts_a_real_key() {
    assert_eq!(
        resolve_openai_key(ProviderId::OpenAi, Some("sk-real".to_string()))
            .unwrap()
            .as_deref(),
        Some("sk-real")
    );
}

#[test]
fn resolve_openai_key_trims_the_returned_key_not_just_the_checked_one() {
    // A pasted key with a trailing space/newline must reach `bearer_auth`
    // TRIMMED — checking `k.trim().is_empty()` but returning the padded `k`
    // is the exact bug: a trailing space just 401s, an embedded `\n` makes
    // the header value invalid and the request never builds at all.
    assert_eq!(
        resolve_openai_key(ProviderId::OpenAi, Some(" sk-real \n".to_string()))
            .unwrap()
            .as_deref(),
        Some("sk-real")
    );
}

#[test]
fn resolve_openai_key_allows_a_missing_or_blank_key_for_openai_compatible() {
    // A keyless self-hosted deployment (LM Studio, vLLM, …) is explicitly
    // supported — generation already works with no key
    // (`chat_stream`/`chat_with_tools` default a missing key to `""`), so
    // listing/testing must not hard-error just because no key is stored.
    assert_eq!(
        resolve_openai_key(ProviderId::OpenAiCompatible, None).unwrap(),
        None
    );
    assert_eq!(
        resolve_openai_key(ProviderId::OpenAiCompatible, Some("   ".to_string())).unwrap(),
        None
    );
}

#[test]
fn resolve_openai_key_still_prefers_a_real_key_for_openai_compatible_when_present() {
    assert_eq!(
        resolve_openai_key(ProviderId::OpenAiCompatible, Some("real-key".to_string()))
            .unwrap()
            .as_deref(),
        Some("real-key")
    );
}

#[test]
fn parse_model_list_applies_the_native_openai_chat_filter() {
    let body = json!({
        "data": [{ "id": "gpt-4o" }, { "id": "text-embedding-3-small" }]
    });
    let names: Vec<String> = parse_model_list(ProviderId::OpenAi, &body)
        .unwrap()
        .into_iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["gpt-4o"]);
}

#[test]
fn parse_model_list_passes_through_unfiltered_for_openai_compatible() {
    let body = json!({ "data": [{ "id": "gpt-oss:120b" }] });
    let names: Vec<String> = parse_model_list(ProviderId::OpenAiCompatible, &body)
        .unwrap()
        .into_iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["gpt-oss:120b"]);
}

#[test]
fn parse_model_list_normalizes_created_epoch_seconds_to_millis() {
    // OpenAI's `created` is unix-epoch SECONDS — 1704067200 is the well-known
    // 2024-01-01T00:00:00Z reference point; the normalized `createdAt` must
    // be that value in MILLISECONDS, this codebase's `createdAt` convention.
    let body = json!({
        "data": [{ "id": "gpt-4o", "created": 1_704_067_200i64 }]
    });
    let page = parse_model_list(ProviderId::OpenAi, &body).unwrap();
    assert_eq!(
        page,
        vec![json!({ "name": "gpt-4o", "createdAt": 1_704_067_200_000i64 })]
    );
}

#[test]
fn parse_model_list_omits_created_at_rather_than_overflow_on_a_pathological_value() {
    // `created` is provider-controlled — a bare `secs * 1000` can overflow
    // `i64` (panics in debug, silently wraps in release). Must degrade to
    // "omit the field", never fabricate a wrapped/garbage timestamp.
    let body = json!({
        "data": [{ "id": "gpt-4o", "created": i64::MAX }]
    });
    let page = parse_model_list(ProviderId::OpenAi, &body).unwrap();
    assert_eq!(page, vec![json!({ "name": "gpt-4o" })]);
}

#[test]
fn parse_model_list_omits_optional_fields_the_provider_does_not_return_and_keeps_name_unchanged() {
    // `name` must stay byte-identical to the pre-widening shape — a stored
    // model preference matches against it. OpenAI's `/v1/models` never
    // returns `displayName`/`contextLength` at all.
    let body = json!({ "data": [{ "id": "gpt-4o" }] });
    let page = parse_model_list(ProviderId::OpenAi, &body).unwrap();
    assert_eq!(page, vec![json!({ "name": "gpt-4o" })]);
}

#[test]
fn parse_model_list_ok_empty_on_genuinely_empty_catalogue() {
    let body = json!({ "data": [] });
    assert_eq!(
        parse_model_list(ProviderId::OpenAi, &body).unwrap(),
        Vec::<Value>::new()
    );
}

#[test]
fn parse_model_list_errors_when_data_field_is_missing() {
    let body = json!({ "unexpected": "shape" });
    assert!(matches!(
        parse_model_list(ProviderId::OpenAi, &body),
        Err(AppError::Provider(_))
    ));
}
