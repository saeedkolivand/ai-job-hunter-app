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
    parse_ollama_turn, parse_ollama_usage, parse_web_search, resolve_intent, Intent, OllamaClient,
    SamplingProfile, StreamPiece, DETERMINISTIC_TEMPERATURE, PROSE_GROUNDED_TEMPERATURE,
    PROSE_REPEAT_PENALTY, PROSE_TEMPERATURE, PROSE_TOP_P,
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

/// Mirrors what `OllamaClient::chat_stream` does: resolve this adapter's own
/// profile for `req.model` + `req.intent`, merged with the request's
/// explicit numeric overrides.
fn sampling_for(req: &AiGenerateRequest) -> SamplingProfile {
    OllamaClient
        .sampling_profile(&req.model, resolve_intent(req))
        .resolve(req)
}

#[test]
fn sampling_profile_declares_real_values_for_every_model_no_unknown_model_fail_safe() {
    // Unlike every cloud adapter, Ollama has no "this family 400s" split —
    // every model gets the SAME real per-intent values, including an
    // unrecognized/future model id (there is no unsafe case to fail toward
    // neutral for).
    for model in ["llama3.1:8b", "some-unrecognized-future-model"] {
        assert_eq!(
            OllamaClient.sampling_profile(model, Intent::Deterministic),
            SamplingProfile {
                temperature: Some(DETERMINISTIC_TEMPERATURE),
                ..SamplingProfile::default()
            },
            "{model}"
        );
        assert_eq!(
            OllamaClient.sampling_profile(model, Intent::Prose),
            SamplingProfile {
                temperature: Some(PROSE_TEMPERATURE),
                top_p: Some(PROSE_TOP_P),
                repeat_penalty: Some(PROSE_REPEAT_PENALTY),
                ..SamplingProfile::default()
            },
            "{model}"
        );
        assert_eq!(
            OllamaClient.sampling_profile(model, Intent::ProseGrounded),
            SamplingProfile {
                temperature: Some(PROSE_GROUNDED_TEMPERATURE),
                top_p: Some(PROSE_TOP_P),
                repeat_penalty: Some(PROSE_REPEAT_PENALTY),
                ..SamplingProfile::default()
            },
            "{model}"
        );
        // `Default` (no declared intent) resolves the same as `Deterministic`.
        assert_eq!(
            OllamaClient.sampling_profile(model, Intent::Default),
            OllamaClient.sampling_profile(model, Intent::Deterministic),
            "{model}"
        );
    }
}

#[test]
fn wire_body_carries_the_declared_deterministic_temperature_with_no_explicit_override() {
    // End-to-end: was DEAD before `chat_stream` was wired through
    // `sampling_profile` — `stream_chat` called `build_chat_stream_body(req)`
    // directly, reading `req.temperature` (always `None` from the renderer
    // now) with no fallback at all, silently deferring to whatever the
    // model's Modelfile says (see `sampling_profile`'s doc comment for the
    // empirical live-Ollama evidence this is unsafe). Mutation check: change
    // `DETERMINISTIC_TEMPERATURE` or delete the `Intent::Deterministic` arm
    // in `OllamaClient::sampling_profile` and this must fail.
    let mut req = base_request();
    req.temperature = None;
    req.intent = Some("deterministic".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        body["options"]["temperature"],
        json!(DETERMINISTIC_TEMPERATURE)
    );
}

#[test]
fn wire_body_prose_grounded_carries_its_own_temperature_not_proses() {
    // Ollama's wire body has no presence_penalty field at all (see
    // `build_chat_stream_body`'s doc comment), so on this adapter the only
    // observable difference between the two prose intents is the temperature.
    // It is HIGHER for grounded, not lower — see `PROSE_GROUNDED_TEMPERATURE`'s
    // doc comment for why that is not the contradiction it looks like.
    let mut req = base_request();
    req.temperature = None;
    req.intent = Some("prose_grounded".to_string());
    let grounded = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(
        grounded["options"]["temperature"],
        json!(PROSE_GROUNDED_TEMPERATURE)
    );
    assert_eq!(grounded["options"]["top_p"], json!(PROSE_TOP_P));
    assert_eq!(
        grounded["options"]["repeat_penalty"],
        json!(PROSE_REPEAT_PENALTY)
    );

    req.intent = Some("prose".to_string());
    let prose = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(prose["options"]["temperature"], json!(PROSE_TEMPERATURE));
}

#[test]
fn chat_stream_body_serializes_top_p_and_repeat_penalty_when_set() {
    let mut req = base_request();
    req.top_p = Some(0.95);
    req.repeat_penalty = Some(1.15);
    let body = build_chat_stream_body(&req, sampling_for(&req));
    // `base_request()` sets `temperature: Some(0.8)` — the request's
    // explicit override the Settings slider depends on — and it must still
    // win over the intent-derived profile after `.resolve(req)`.
    assert_eq!(body["options"]["temperature"], json!(0.8));
    assert_eq!(body["options"]["top_p"], json!(0.95));
    assert_eq!(body["options"]["repeat_penalty"], json!(1.15));
    // frequency_penalty is never remapped into Ollama's repeat_penalty field.
    assert!(body["options"].get("frequency_penalty").is_none());
}

#[test]
fn chat_stream_body_omits_sampling_options_when_none() {
    let req = base_request();
    let body = build_chat_stream_body(&req, sampling_for(&req));
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
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert_eq!(body["think"], json!("high"));
}

#[test]
fn chat_stream_body_omits_think_for_a_non_thinking_model_even_with_effort_set() {
    let mut req = base_request();
    req.model = "llama3.1:8b".to_string();
    req.effort = Some("high".to_string());
    let body = build_chat_stream_body(&req, sampling_for(&req));
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
    let body = build_chat_stream_body(&req, sampling_for(&req));
    assert!(
        body.get("think").is_none(),
        "xhigh is outside OLLAMA_EFFORT_LEVELS — must not be sent"
    );
}

#[test]
fn chat_stream_body_omits_think_when_effort_not_set() {
    let mut req = base_request();
    req.model = "gpt-oss:20b".to_string();
    let body = build_chat_stream_body(&req, sampling_for(&req));
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

// ── Local chat-model selection (the qwen3-embedding:8b → /api/chat 400) ────────
//
// `reachable_model` returned `/api/tags`'s FIRST entry unconditionally, and
// `reachable_chat_model` forwarded it to `translate_text`, which POSTs it to
// `/api/chat`. A user whose first entry was an embedding model therefore took a
// guaranteed 400 per job ad — eight in one reported session, invisible, because
// translation falls back to the untranslated original on any error.

#[test]
fn embedding_only_models_are_recognized_by_name() {
    for name in [
        "qwen3-embedding:8b",
        "nomic-embed-text:latest",
        "mxbai-embed-large",
        "snowflake-arctic-embed2",
        "EMBEDDINGGEMMA:300M", // case-insensitive
    ] {
        assert!(
            super::is_embedding_only_model(name),
            "{name} must be excluded from chat"
        );
    }
}

#[test]
fn real_chat_models_are_not_mistaken_for_embedding_models() {
    for name in [
        "gemma4:31b",
        "gpt-oss:20b",
        "llama3.1:8b",
        "qwen3:32b",
        "deepseek-v4-flash:0731",
        "mistral-small:24b",
    ] {
        assert!(
            !super::is_embedding_only_model(name),
            "{name} must stay eligible for chat"
        );
    }
}

#[test]
fn first_chat_model_skips_a_leading_embedding_model() {
    // Exactly the reported shape: the embedding model sorts first in /api/tags.
    let body = serde_json::json!({
        "models": [
            { "name": "qwen3-embedding:8b" },
            { "name": "gemma4:31b" },
        ]
    });
    assert_eq!(
        super::first_chat_model(&body).as_deref(),
        Some("gemma4:31b")
    );
}

#[test]
fn first_chat_model_is_none_when_only_embedding_models_are_installed() {
    // Better than returning one anyway: the caller skips translation instead of
    // sending a request the daemon is guaranteed to reject.
    let body = serde_json::json!({ "models": [{ "name": "qwen3-embedding:8b" }] });
    assert_eq!(super::first_chat_model(&body), None);
    assert_eq!(super::first_chat_model(&serde_json::json!({})), None);
}

// ── Structured output (`complete_structured`) ─────────────────────────────────

/// The structured half of a non-streaming call with only `format` set — the
/// per-test variations below override the one field they are about.
fn structured_call(format: Value) -> super::StructuredCall<'static> {
    super::StructuredCall {
        format,
        effort: None,
        max_tokens: None,
        context_window: None,
    }
}

#[test]
fn complete_body_carries_the_schema_in_ollamas_own_format_field() {
    // Ollama's constrained-decoding key is `format` (NOT OpenAI's
    // `response_format`), and it takes the JSON Schema verbatim — no dialect
    // translation. Mutation check: drop the `format` insert in
    // `build_complete_body` and this fails.
    let schema = json!({ "type": "object", "properties": { "score": { "type": "integer" } } });
    let body = super::build_complete_body(
        "llama3.1:8b",
        "sys",
        "user",
        Some(0.3),
        Some(structured_call(super::structured::ollama_format(Some(
            &schema,
        )))),
    );
    assert_eq!(body["format"], schema);
    assert_eq!(body["stream"], json!(false));
}

#[test]
fn complete_body_falls_back_to_the_json_format_string_without_a_schema() {
    let body = super::build_complete_body(
        "llama3.1:8b",
        "sys",
        "user",
        None,
        Some(structured_call(super::structured::ollama_format(None))),
    );
    assert_eq!(body["format"], json!("json"));
}

#[test]
fn complete_body_omits_format_entirely_on_the_plain_completion_path() {
    // `complete`/`complete_with_usage` pass `None` — an unconstrained call must
    // stay byte-identical to what it sent before structured output existed.
    let body = super::build_complete_body("llama3.1:8b", "sys", "user", Some(0.3), None);
    assert!(
        body.get("format").is_none(),
        "plain completions must not start asking for JSON: {body}"
    );
    assert!(body.get("think").is_none(), "{body}");
    assert_eq!(body["options"], json!({ "temperature": 0.3 }));
}

#[test]
fn complete_body_carries_think_only_where_the_streaming_body_would() {
    // `complete_structured` is the only non-streaming path handed the whole
    // `AiGenerateRequest`, and it dropped `effort` on the floor while
    // `chat_stream` honored it: a thinking model asked for a JSON answer ran
    // with thinking off no matter what the user picked. Both paths now share
    // ONE gate. Mutation check: drop the `think` insert in
    // `build_complete_body` and the first assertion fails.
    let body = super::build_complete_body(
        "gpt-oss:20b",
        "sys",
        "user",
        None,
        Some(super::StructuredCall {
            effort: Some("high"),
            ..structured_call(super::structured::ollama_format(None))
        }),
    );
    assert_eq!(body["think"], json!("high"));
    // Top-level, never under `options` — Ollama ignores it there.
    assert!(body["options"].get("think").is_none(), "{body}");

    // A model outside the thinking family 400s on the field, and a level
    // outside `OLLAMA_EFFORT_LEVELS` 400s on any model (`effort` is stored per
    // PROVIDER, so a stale value reaches here unchanged).
    for (model, effort) in [
        ("llama3.1:8b", "high"),
        ("qwen3-coder:480b", "high"),
        ("gpt-oss:20b", "xhigh"),
        ("gpt-oss:20b", "   "),
    ] {
        let body = super::build_complete_body(
            model,
            "sys",
            "user",
            None,
            Some(super::StructuredCall {
                effort: Some(effort),
                ..structured_call(super::structured::ollama_format(None))
            }),
        );
        assert!(
            body.get("think").is_none(),
            "{model} must not be sent think {effort:?}: {body}"
        );
    }
}

#[test]
fn complete_body_carries_the_same_token_options_the_streaming_body_would() {
    // HIGH: the structured path dropped BOTH `num_predict` and `num_ctx` while
    // `chat_stream` sent them — same class as the `effort` drop above, and the
    // costlier one: `complete_structured` is the path that carries a whole
    // résumé plus a job ad, so a missing `num_ctx` silently truncated the
    // prompt against Ollama's small default context. Asserted against the
    // STREAMING body's own values, so the two can only drift together.
    // Mutation check: drop either `options.insert` in `build_complete_body`
    // and this fails.
    let mut req = base_request();
    req.max_tokens = Some(777);
    req.context_window = Some(32_768);
    let stream = build_chat_stream_body(&req, sampling_for(&req));
    let body = super::build_complete_body(
        &req.model,
        "sys",
        "user",
        None,
        Some(super::StructuredCall {
            max_tokens: req.max_tokens,
            context_window: req.context_window,
            ..structured_call(super::structured::ollama_format(None))
        }),
    );
    assert_eq!(stream["options"]["num_predict"], json!(777), "{stream}");
    assert_eq!(stream["options"]["num_ctx"], json!(32_768), "{stream}");
    assert_eq!(
        body["options"]["num_predict"],
        stream["options"]["num_predict"]
    );
    assert_eq!(body["options"]["num_ctx"], stream["options"]["num_ctx"]);
}

#[test]
fn complete_body_omits_the_token_options_the_request_left_unset() {
    // The negative half: an unset field must be ABSENT, never `null` — Ollama
    // reads a present `num_ctx` as an override, and the model's own Modelfile
    // default is the right answer when the request has no opinion.
    let req = base_request();
    assert!(req.max_tokens.is_none() && req.context_window.is_none());
    let body = super::build_complete_body(
        &req.model,
        "sys",
        "user",
        Some(0.3),
        Some(super::StructuredCall {
            max_tokens: req.max_tokens,
            context_window: req.context_window,
            ..structured_call(super::structured::ollama_format(None))
        }),
    );
    assert_eq!(body["options"], json!({ "temperature": 0.3 }), "{body}");
}
