//! Non-streaming structured-output body construction
//! (`build_complete_body`/`StructuredCall`).

use serde_json::json;

use super::super::super::structured;
use super::super::chat::{build_chat_stream_body, build_complete_body, StructuredCall};
use super::support::{base_request, sampling_for, structured_call};

#[test]
fn complete_body_carries_the_schema_in_ollamas_own_format_field() {
    // Ollama's constrained-decoding key is `format` (NOT OpenAI's
    // `response_format`), and it takes the JSON Schema verbatim — no dialect
    // translation. Mutation check: drop the `format` insert in
    // `build_complete_body` and this fails.
    let schema = json!({ "type": "object", "properties": { "score": { "type": "integer" } } });
    let body = build_complete_body(
        "llama3.1:8b",
        "sys",
        "user",
        Some(0.3),
        Some(structured_call(structured::ollama_format(Some(&schema)))),
    );
    assert_eq!(body["format"], schema);
    // Streamed + re-assembled (#1353): `format` rides the stream request.
    assert_eq!(body["stream"], json!(true));
}

#[test]
fn complete_body_falls_back_to_the_json_format_string_without_a_schema() {
    let body = build_complete_body(
        "llama3.1:8b",
        "sys",
        "user",
        None,
        Some(structured_call(structured::ollama_format(None))),
    );
    assert_eq!(body["format"], json!("json"));
}

#[test]
fn complete_body_omits_format_entirely_on_the_plain_completion_path() {
    // `complete`/`complete_with_usage` pass `None` — an unconstrained call must
    // stay byte-identical to what it sent before structured output existed.
    let body = build_complete_body("llama3.1:8b", "sys", "user", Some(0.3), None);
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
    let body = build_complete_body(
        "gpt-oss:20b",
        "sys",
        "user",
        None,
        Some(StructuredCall {
            effort: Some("high"),
            ..structured_call(structured::ollama_format(None))
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
        let body = build_complete_body(
            model,
            "sys",
            "user",
            None,
            Some(StructuredCall {
                effort: Some(effort),
                ..structured_call(structured::ollama_format(None))
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
    let body = build_complete_body(
        &req.model,
        "sys",
        "user",
        None,
        Some(StructuredCall {
            max_tokens: req.max_tokens,
            context_window: req.context_window,
            ..structured_call(structured::ollama_format(None))
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
    let body = build_complete_body(
        &req.model,
        "sys",
        "user",
        Some(0.3),
        Some(StructuredCall {
            max_tokens: req.max_tokens,
            context_window: req.context_window,
            ..structured_call(structured::ollama_format(None))
        }),
    );
    assert_eq!(body["options"], json!({ "temperature": 0.3 }), "{body}");
}

/// The plain-text effort path (`complete_with_effort`, what repair/humanize
/// use): `format: None` sends NO `format`, but the effort and limits still
/// reach the wire. Mutation check: unconditionally insert `format` and the
/// first assertion fails; drop the `think` gate for it and the last fails.
#[test]
fn a_plain_call_carries_think_and_limits_but_no_format() {
    let body = build_complete_body(
        "qwen3.8:latest",
        "sys",
        "user",
        None,
        Some(StructuredCall {
            format: None,
            effort: Some("off"),
            max_tokens: Some(256),
            context_window: Some(8_192),
        }),
    );
    assert!(body.get("format").is_none(), "{body}");
    assert_eq!(body["think"], json!(false), "{body}");
    assert_eq!(body["options"]["num_predict"], json!(256));
    assert_eq!(body["options"]["num_ctx"], json!(8_192));
}
