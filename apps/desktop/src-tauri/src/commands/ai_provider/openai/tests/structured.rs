//! Non-streaming structured-output body construction
//! (`build_complete_body`/`StructuredCall`/`reasoning_effort`).

use serde_json::json;

use super::super::super::{structured, AiProvider, ProviderId, SamplingProfile};
use super::super::body::{
    build_chat_stream_body, build_complete_body, reasoning_effort, StructuredCall,
};
use super::super::OpenAiClient;
use super::support::{base_request, chat_caps, structured_call};

#[test]
fn complete_body_carries_a_strict_json_schema_response_format_when_a_schema_is_supplied() {
    // The wire assertion for OpenAI's native structured output: the field is
    // `response_format`, and strict mode needs the schema nested under
    // `json_schema.schema`. Mutation check: drop the `response_format` insert
    // in `build_complete_body` and this fails.
    let schema = json!({ "type": "object", "properties": { "score": { "type": "integer" } } });
    let body = build_complete_body(
        "gpt-4o",
        "sys",
        "user",
        Some(0.3),
        chat_caps(true),
        Some(structured_call(structured::openai_response_format(Some(
            &schema,
        )))),
    );
    assert_eq!(body["response_format"]["type"], json!("json_schema"));
    assert_eq!(
        body["response_format"]["json_schema"]["strict"],
        json!(true)
    );
    assert_eq!(
        body["response_format"]["json_schema"]["schema"]["properties"]["score"]["type"],
        json!("integer")
    );
    assert_eq!(body["stream"], json!(false));
}

#[test]
fn complete_body_omits_response_format_entirely_on_the_plain_completion_path() {
    // `complete`/`complete_with_usage` pass `None` — an unconstrained call must
    // stay byte-identical to what it sent before structured output existed.
    let body = build_complete_body("gpt-4o", "sys", "user", Some(0.3), chat_caps(true), None);
    assert!(
        body.get("response_format").is_none(),
        "plain completions must not start asking for JSON: {body}"
    );
    assert!(body.get("reasoning_effort").is_none(), "{body}");
    assert!(body.get("max_tokens").is_none(), "{body}");
}

#[test]
fn complete_body_carries_reasoning_effort_only_where_the_streaming_body_would() {
    // `complete_structured` is the only non-streaming path handed the whole
    // `AiGenerateRequest`, and it dropped `effort` on the floor while
    // `chat_stream` honored it: a reasoning model asked for a JSON answer ran
    // at OpenAI's default effort no matter what the user picked. Both paths now
    // share ONE gate, so the two can't drift again. Mutation check: drop the
    // `reasoning_effort` insert in `build_complete_body` and the first
    // assertion fails.
    let client = OpenAiClient::new(ProviderId::OpenAi, None);

    let caps = client.capabilities("o3-mini");
    let body = build_complete_body(
        "o3-mini",
        "sys",
        "user",
        None,
        caps,
        Some(StructuredCall {
            effort: Some("high"),
            ..structured_call(json!({ "type": "json_object" }))
        }),
    );
    assert_eq!(body["reasoning_effort"], json!("high"));

    // A model that does not take the field must never be sent it — it 400s.
    let caps = client.capabilities("gpt-4o");
    let body = build_complete_body(
        "gpt-4o",
        "sys",
        "user",
        None,
        caps,
        Some(StructuredCall {
            effort: Some("high"),
            ..structured_call(json!({ "type": "json_object" }))
        }),
    );
    assert!(
        body.get("reasoning_effort").is_none(),
        "a non-reasoning model must not be sent reasoning_effort: {body}"
    );

    // …and neither may a level outside `OPENAI_EFFORT_LEVELS` (effort is stored
    // per PROVIDER, so a stale value from another model can arrive here), nor a
    // blank/absent one.
    let caps = client.capabilities("o3-mini");
    assert_eq!(reasoning_effort(Some("xhigh"), caps), None);
    assert_eq!(reasoning_effort(Some("  "), caps), None);
    assert_eq!(reasoning_effort(None, caps), None);
    // Padding the stored value must not defeat the gate either.
    assert_eq!(reasoning_effort(Some(" high "), caps), Some("high"));
}

#[test]
fn complete_body_carries_the_same_token_limit_field_the_streaming_body_would() {
    // HIGH: the structured path dropped `req.max_tokens` while `chat_stream`
    // sent it — the same class as the `effort` drop above. Asserted against the
    // STREAMING body's own value on BOTH spellings of the field, so the two
    // can only drift together and the o-series rename can't be got wrong on
    // one path only. Mutation check: drop the `max_tokens` insert in
    // `build_complete_body` (or hardcode either spelling in `token_field`) and
    // this fails.
    let client = OpenAiClient::new(ProviderId::OpenAi, None);
    for (model, field) in [
        ("gpt-4o", "max_tokens"),
        ("o3-mini", "max_completion_tokens"),
    ] {
        let mut req = base_request();
        req.model = model.to_string();
        req.max_tokens = Some(777);
        let caps = client.capabilities(model);
        let stream = build_chat_stream_body(&req, caps, SamplingProfile::default());
        let body = build_complete_body(
            model,
            "sys",
            "user",
            None,
            caps,
            Some(StructuredCall {
                max_tokens: req.max_tokens,
                ..structured_call(json!({ "type": "json_object" }))
            }),
        );
        assert_eq!(stream[field], json!(777), "{model}: {stream}");
        assert_eq!(body[field], stream[field], "{model}: {body}");
    }
}

#[test]
fn complete_body_omits_the_token_limit_the_request_left_unset() {
    // The negative half: an unset `max_tokens` must be ABSENT, never `null` —
    // the vendor's own per-model default is the right answer when the request
    // has no opinion, and `null` is a 400 on some gateways.
    let client = OpenAiClient::new(ProviderId::OpenAi, None);
    let req = base_request();
    assert!(req.max_tokens.is_none());
    let body = build_complete_body(
        &req.model,
        "sys",
        "user",
        None,
        client.capabilities(&req.model),
        Some(StructuredCall {
            max_tokens: req.max_tokens,
            ..structured_call(json!({ "type": "json_object" }))
        }),
    );
    assert!(body.get("max_tokens").is_none(), "{body}");
    assert!(body.get("max_completion_tokens").is_none(), "{body}");
}
