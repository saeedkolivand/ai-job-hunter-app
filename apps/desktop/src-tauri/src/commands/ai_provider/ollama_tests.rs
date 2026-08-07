//! Unit tests for `ollama.rs`, split into this sibling file (R8 line-budget
//! split — mirrors the `anthropic.rs` + `anthropic_tests.rs` precedent of
//! moving the test module itself out rather than production code; done
//! proactively here — `ollama.rs` was at 1371/1400 LOC, 29 lines from the
//! hard cap, after this round's fixes).
//!
//! Wired via `#[path = "ollama_tests.rs"] mod tests;` in `ollama.rs` — that
//! keeps this a CHILD module of `ollama` in the module tree (same as an
//! inline `#[cfg(test)] mod tests { ... }` block), so the imports below
//! still reach every private item there, while this file's own filename
//! (ending `tests.rs`) excludes it from the architecture test's R8 LOC cap
//! (`tests/architecture.rs`'s `is_test` filename check) and from R3/R6's
//! non-test scans.

use super::{
    build_chat_stream_body, build_ollama_embed_body, normalize_show,
    ollama_family_supports_thinking, ollama_supports_tools, parse_model_list, parse_ollama_frames,
    parse_ollama_turn, parse_ollama_usage, parse_web_search, Intent, OllamaClient, SamplingProfile,
    StreamPiece,
};
use crate::commands::ai_provider::{AiGenerateRequest, AiProvider, StopReason, ToolCall};
use crate::error::AppError;
use crate::ipc_contracts::ai::AiGenerateRequestMessage;
use serde_json::{json, Value};

fn base_request() -> AiGenerateRequest {
    AiGenerateRequest {
        model: "llama3.1:8b".to_string(),
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

#[test]
fn sampling_profile_is_neutral_for_every_intent_and_an_unknown_model() {
    // Local Ollama always defers to the model's own Modelfile — never a
    // per-(model, intent) app default.
    for model in ["llama3.1:8b", "some-unrecognized-future-model"] {
        for intent in [Intent::Deterministic, Intent::Prose, Intent::Default] {
            assert_eq!(
                OllamaClient.sampling_profile(model, intent),
                SamplingProfile::default()
            );
        }
    }
}

#[test]
fn chat_stream_body_serializes_top_p_and_repeat_penalty_when_set() {
    let mut req = base_request();
    req.top_p = Some(0.95);
    req.repeat_penalty = Some(1.15);
    let body = build_chat_stream_body(&req);
    assert_eq!(body["options"]["top_p"], json!(0.95));
    assert_eq!(body["options"]["repeat_penalty"], json!(1.15));
    // frequency_penalty is never remapped into Ollama's repeat_penalty field.
    assert!(body["options"].get("frequency_penalty").is_none());
}

#[test]
fn chat_stream_body_omits_sampling_options_when_none() {
    let body = build_chat_stream_body(&base_request());
    assert!(body["options"].get("top_p").is_none());
    assert!(body["options"].get("repeat_penalty").is_none());
}

#[test]
fn embed_body_sends_a_grown_chunk_in_full_never_silently_truncated() {
    // Regression guard for the reintroduced silent-truncation defect:
    // `bounded_split_cap`'s growth path (`embed.rs`) deliberately sizes a
    // chunk PAST the nominal 8000-char cap for a document that would
    // otherwise need more than `MAX_CHUNKS_PER_DOCUMENT` chunks — a real
    // example is a 300,000-char document / 32 chunks = 9,375 chars each.
    // `embed_with` used to `chars().take(8000)` here and silently drop
    // the last ~1,375 chars of every such chunk with no error. The body
    // builder must send whatever it's given, in full, regardless of
    // length — total chars sent must equal the input length.
    let text = "a".repeat(9_375);
    let body = build_ollama_embed_body("nomic-embed-text", &text);
    assert_eq!(
        body["prompt"].as_str().unwrap().chars().count(),
        9_375,
        "the full grown chunk must reach the provider, never silently truncated"
    );
}

#[test]
fn parse_ollama_frames_splits_thinking_and_content() {
    let mut buf = String::from(
        "{\"message\":{\"thinking\":\"hmm\"},\"done\":false}\n\
         {\"message\":{\"content\":\"hello\"},\"done\":false}\n",
    );
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(
        pieces,
        vec![StreamPiece::thinking("hmm"), StreamPiece::text("hello")]
    );
    assert!(buf.is_empty());
}

#[test]
fn parse_ollama_frames_done_carries_final_content() {
    // The `done:true` object becomes a sentinel carrying its final content.
    let mut buf = String::from("{\"message\":{\"content\":\"end\"},\"done\":true}\n");
    assert_eq!(
        parse_ollama_frames(&mut buf),
        vec![StreamPiece::done("end")]
    );
}

#[test]
fn parse_ollama_frames_done_carries_real_token_usage() {
    let mut buf = String::from(
        "{\"message\":{\"content\":\"end\"},\"done\":true,\"prompt_eval_count\":123,\"eval_count\":45}\n",
    );
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(pieces.len(), 1);
    let usage = pieces[0].usage.expect("done piece must carry usage");
    assert_eq!(usage.input_tokens, 123);
    assert_eq!(usage.output_tokens, 45);
}

/// The streamed sentinel must carry `done_reason` too, not just usage. Without
/// it `stream::finish` can't tell "the local model burned its whole budget
/// reasoning and never answered" from "the provider silently returned nothing" —
/// the exact ambiguity that made an empty generation unexplainable.
#[test]
fn parse_ollama_frames_done_carries_the_stop_reason() {
    let mut buf = String::from(
        "{\"message\":{\"content\":\"\"},\"done\":true,\"done_reason\":\"length\"}
",
    );
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].stop_reason, Some(StopReason::Length));
}

#[test]
fn parse_ollama_frames_done_reports_a_clean_stop_as_end_not_length() {
    // The differential: a normal completion must NOT be reported as truncated,
    // or every finished generation would claim it ran out of budget.
    let mut buf = String::from(
        "{\"message\":{\"content\":\"hi\"},\"done\":true,\"done_reason\":\"stop\"}
",
    );
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(pieces[0].stop_reason, Some(StopReason::End));
}

#[test]
fn parse_ollama_frames_done_without_a_reason_reports_none() {
    // Older/leaner Ollama builds omit the field; absent must stay `None` rather
    // than being invented as a clean end.
    let mut buf = String::from(
        "{\"message\":{\"content\":\"hi\"},\"done\":true}
",
    );
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(pieces[0].stop_reason, None);
}

#[test]
fn parse_usage_reads_prompt_eval_and_eval_counts() {
    let data = json!({ "prompt_eval_count": 10, "eval_count": 20 });
    let usage = parse_ollama_usage(&data).expect("usage present");
    assert_eq!(usage.input_tokens, 10);
    assert_eq!(usage.output_tokens, 20);
}

#[test]
fn parse_usage_is_none_when_absent() {
    assert!(parse_ollama_usage(&json!({})).is_none());
}

#[test]
fn parse_ollama_frames_buffers_partial_line() {
    // A partial trailing JSON line is left for the next chunk.
    let mut buf = String::from("{\"message\":{\"content\":\"hi\"},\"done\":false}\n{\"mess");
    let pieces = parse_ollama_frames(&mut buf);
    assert_eq!(pieces, vec![StreamPiece::text("hi")]);
    assert_eq!(buf, "{\"mess");
}

#[test]
fn parse_ollama_frames_skips_blank_and_unparseable_lines() {
    let mut buf = String::from("\nnot-json\n");
    assert!(parse_ollama_frames(&mut buf).is_empty());
}

#[test]
fn parse_web_search_maps_results_and_caps_limit() {
    let body = json!({
        "results": [
            { "title": "Acme — Wikipedia", "url": "https://w/a", "content": "Acme makes widgets." },
            { "title": "Acme careers", "url": "https://a/c", "content": "Series B." },
            { "title": "extra", "url": "https://x", "content": "ignored by limit" },
        ]
    });
    let out = parse_web_search(&body, 2);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].title, "Acme — Wikipedia");
    assert_eq!(out[0].snippet, "Acme makes widgets.");
    assert_eq!(out[1].url, "https://a/c");
}

#[test]
fn parse_web_search_tolerates_missing_fields_and_no_results() {
    assert!(parse_web_search(&json!({}), 5).is_empty());
    let out = parse_web_search(&json!({ "results": [{}] }), 5);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].title, "");
    assert_eq!(out[0].snippet, "");
}

#[test]
fn normalize_extracts_context_and_details_by_architecture() {
    // `context_length` is keyed by architecture — scan for the suffix, not a
    // hardcoded `llama.` prefix, so qwen2/phi3/etc. all work unchanged.
    let data = json!({
        "model_info": { "qwen2.context_length": 32768, "qwen2.embedding_length": 3584 },
        "details": { "parameter_size": "7.6B", "quantization_level": "Q4_K_M", "family": "qwen2" }
    });
    let out = normalize_show(&data);
    assert_eq!(out["contextLength"], json!(32768));
    assert_eq!(out["parameterSize"], json!("7.6B"));
    assert_eq!(out["quantization"], json!("Q4_K_M"));
    assert_eq!(out["family"], json!("qwen2"));
}

#[test]
fn normalize_omits_missing_fields() {
    let data = json!({
        "model_info": { "llama.context_length": 8192 },
        "details": { "parameter_size": "8B" }
    });
    let out = normalize_show(&data);
    assert_eq!(out["contextLength"], json!(8192));
    assert_eq!(out["parameterSize"], json!("8B"));
    // Absent fields are omitted (not null), so the TS optional schema accepts it.
    assert!(out.get("quantization").is_none());
    assert!(out.get("family").is_none());
}

#[test]
fn normalize_returns_null_when_nothing_usable() {
    assert!(normalize_show(&json!({})).is_null());
    assert!(normalize_show(&json!({ "model_info": {}, "details": {} })).is_null());
}

#[test]
fn tool_support_gate_is_conservative() {
    for m in [
        "llama3.1:8b",
        "llama3.3:70b",
        "qwen2.5:7b",
        "mistral-nemo",
        "command-r-plus",
    ] {
        assert!(ollama_supports_tools(m), "{m} should advertise tools");
    }
    // Unknown / non-tool families default off so the turn degrades safely.
    for m in [
        "llama2",
        "phi3",
        "gemma2",
        "nomic-embed-text",
        "deepseek-coder",
    ] {
        assert!(!ollama_supports_tools(m), "{m} must default to no tools");
    }
}

#[test]
fn parse_turn_reads_object_arguments_and_content() {
    // Ollama returns arguments as an already-decoded object (NOT a JSON string).
    let data = json!({
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{ "function": { "name": "match_resume", "arguments": { "resumeId": "r1", "jobId": "j1" } } }]
        },
        "done": true,
        "done_reason": "stop"
    });
    let turn = parse_ollama_turn(&data);
    assert_eq!(turn.stop, StopReason::ToolUse);
    assert_eq!(
        turn.tool_calls,
        vec![ToolCall {
            id: "match_resume-0".to_string(),
            name: "match_resume".to_string(),
            args: json!({ "resumeId": "r1", "jobId": "j1" }),
        }]
    );
}

#[test]
fn parse_turn_plain_answer_has_no_tool_calls() {
    let data = json!({
        "message": { "role": "assistant", "content": "The answer." },
        "done": true,
        "done_reason": "stop"
    });
    let turn = parse_ollama_turn(&data);
    assert_eq!(turn.text, "The answer.");
    assert!(turn.tool_calls.is_empty());
    assert_eq!(turn.stop, StopReason::End);
}

#[test]
fn parse_turn_tool_calls_with_length_done_reason_maps_to_length_not_tool_use() {
    // `done_reason: "length"` means the arguments may be truncated JSON — this
    // must win over the tool-call signal, never `ToolUse`.
    let data = json!({
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{ "function": { "name": "match_resume", "arguments": { "resumeId": "r1" } } }]
        },
        "done": true,
        "done_reason": "length"
    });
    let turn = parse_ollama_turn(&data);
    assert_eq!(turn.stop, StopReason::Length);
}

#[test]
fn thinking_family_gate_matches_documented_models_only() {
    for m in [
        "qwen3",
        "qwen3:30b",
        "qwen3:8b",
        "my-qwen3",
        "gpt-oss:20b",
        "gpt-oss:120b",
        "deepseek-r1",
        "deepseek-r1:70b",
        "deepseek-v3.1:671b",
    ] {
        assert!(ollama_family_supports_thinking(m), "{m} should think");
    }
    // qwen3-coder(-plus) matches the "qwen3" substring but is a separate,
    // non-thinking model — must NOT be swept in by a broad substring match.
    for m in [
        "qwen3-coder:480b",
        "qwen3-coder-plus",
        "llama3.1:8b",
        "mistral",
        "nomic-embed-text",
    ] {
        assert!(!ollama_family_supports_thinking(m), "{m} must not think");
    }
}

#[test]
fn coder_exclusion_is_scoped_to_qwen3_not_every_model_containing_coder() {
    // The exclusion is narrow — a hypothetical thinking-capable "coder"
    // variant in an UNRELATED family must not be swept out by name
    // collision alone (only the qwen3 branch excludes "coder").
    assert!(ollama_family_supports_thinking("gpt-oss-coder"));
    assert!(ollama_family_supports_thinking("deepseek-r1-coder"));
}

#[test]
fn effort_levels_mirror_the_thinking_gate() {
    assert_eq!(
        OllamaClient.effort_levels("gpt-oss:120b"),
        vec!["low", "medium", "high"]
    );
    assert!(OllamaClient.effort_levels("llama3.1:8b").is_empty());
}

#[test]
fn chat_stream_body_sends_think_only_for_a_thinking_model_with_effort_set() {
    let mut req = base_request();
    req.model = "gpt-oss:20b".to_string();
    req.effort = Some("high".to_string());
    let body = build_chat_stream_body(&req);
    assert_eq!(body["think"], json!("high"));
}

#[test]
fn chat_stream_body_omits_think_for_a_non_thinking_model_even_with_effort_set() {
    let mut req = base_request();
    req.model = "llama3.1:8b".to_string();
    req.effort = Some("high".to_string());
    let body = build_chat_stream_body(&req);
    assert!(body.get("think").is_none());
}

#[test]
fn chat_stream_body_omits_think_outside_the_verified_level_set() {
    // Same class as the Gemini/Anthropic/OpenAI gate: a stale/unrecognized
    // value must never be sent just because the CURRENT model is in the
    // thinking family — `effort` is stored per PROVIDER, not per model.
    let mut req = base_request();
    req.model = "gpt-oss:20b".to_string();
    req.effort = Some("xhigh".to_string());
    let body = build_chat_stream_body(&req);
    assert!(
        body.get("think").is_none(),
        "xhigh is outside OLLAMA_EFFORT_LEVELS — must not be sent"
    );
}

#[test]
fn chat_stream_body_omits_think_when_effort_not_set() {
    let mut req = base_request();
    req.model = "gpt-oss:20b".to_string();
    let body = build_chat_stream_body(&req);
    assert!(body.get("think").is_none());
}

// ── list_models ──────────────────────────────────────────────────────────────
// Ollama needs no key (a reachable host counts as healthy), so there is no
// missing-key case here — only status/shape/empty are relevant.

#[test]
fn parse_model_list_maps_tag_names() {
    let body = json!({
        "models": [{ "name": "llama3.1:8b" }, { "name": "gpt-oss:20b" }]
    });
    let names: Vec<String> = parse_model_list(&body)
        .unwrap()
        .into_iter()
        .map(|v| v["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["llama3.1:8b", "gpt-oss:20b"]);
}

#[test]
fn parse_model_list_normalizes_modified_at_to_millis_including_a_non_utc_offset() {
    // Ollama's `modified_at` may carry a non-UTC offset — epoch is
    // offset-independent, so `-07:00` shifts the millis value by +7h vs the
    // same wall-clock time at `Z`.
    let body = json!({
        "models": [{ "name": "llama3.1:8b", "modified_at": "2024-01-01T00:00:00-07:00" }]
    });
    let page = parse_model_list(&body).unwrap();
    assert_eq!(
        page,
        vec![json!({
            "name": "llama3.1:8b",
            "createdAt": 1_704_067_200_000i64 + 7 * 3_600_000,
        })]
    );
}

#[test]
fn parse_model_list_omits_optional_fields_the_provider_does_not_return_and_keeps_name_unchanged() {
    // `name` must stay byte-identical to the pre-widening shape — a stored
    // model preference matches against it. `/api/tags` never returns
    // `displayName`/`contextLength` (context length is only on `/api/show`).
    let body = json!({ "models": [{ "name": "llama3.1:8b" }] });
    let page = parse_model_list(&body).unwrap();
    assert_eq!(page, vec![json!({ "name": "llama3.1:8b" })]);
}

#[test]
fn parse_model_list_ok_empty_on_genuinely_empty_catalogue() {
    let body = json!({ "models": [] });
    assert_eq!(parse_model_list(&body).unwrap(), Vec::<Value>::new());
}

#[test]
fn parse_model_list_errors_when_models_field_is_missing() {
    let body = json!({ "unexpected": "shape" });
    assert!(matches!(
        parse_model_list(&body),
        Err(AppError::Provider(_))
    ));
}
