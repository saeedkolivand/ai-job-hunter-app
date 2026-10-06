//! Non-streaming structured-output body construction
//! (`build_complete_body`/`StructuredCall`).

use serde_json::json;

use super::super::super::{structured, SamplingProfile};
use super::super::body::{build_chat_stream_body, build_complete_body, StructuredCall};
use super::support::{base_request, structured_call};

#[test]
fn complete_body_carries_response_mime_type_and_the_translated_response_schema() {
    // Gemini's constrained-decoding keys live under `generationConfig` and the
    // schema is an OpenAPI-3.0 subset — an UPPERCASE type, not JSON Schema's
    // lowercase one. Mutation check: drop either insert in
    // `build_complete_body`, or lowercase the type in
    // `structured::gemini_response_schema`, and this fails.
    let schema = json!({ "type": "object", "properties": { "score": { "type": "integer" } } });
    let body = build_complete_body(
        "gemini-2.5-flash",
        "sys",
        "user",
        Some(0.3),
        Some(StructuredCall {
            schema: structured::gemini_response_schema(&schema),
            ..structured_call()
        }),
    );
    let config = &body["generationConfig"];
    assert_eq!(config["responseMimeType"], json!("application/json"));
    assert_eq!(config["responseSchema"]["type"], json!("OBJECT"));
    assert_eq!(
        config["responseSchema"]["properties"]["score"]["type"],
        json!("INTEGER")
    );
    assert_eq!(body["systemInstruction"]["parts"][0]["text"], json!("sys"));
}

#[test]
fn complete_body_keeps_json_mode_when_the_schema_cannot_be_translated() {
    // A union type has no OpenAPI-subset equivalent: JSON mode still applies,
    // but no half-translated shape constraint is ever sent.
    let schema = json!({ "type": "object", "properties": { "n": { "type": ["string", "null"] } } });
    let translated = structured::gemini_response_schema(&schema);
    assert!(translated.is_none());
    let body = build_complete_body(
        "gemini-2.5-flash",
        "",
        "user",
        None,
        Some(StructuredCall {
            schema: translated,
            ..structured_call()
        }),
    );
    let config = &body["generationConfig"];
    assert_eq!(config["responseMimeType"], json!("application/json"));
    assert!(config.get("responseSchema").is_none());
}

#[test]
fn complete_body_omits_both_json_fields_on_the_plain_completion_path() {
    // `complete`/`complete_with_usage` pass `None` — an unconstrained call must
    // stay byte-identical to what it sent before structured output.
    let body = build_complete_body("gemini-2.5-flash", "sys", "user", Some(0.3), None);
    let config = &body["generationConfig"];
    assert!(config.get("responseMimeType").is_none(), "{body}");
    assert!(config.get("responseSchema").is_none(), "{body}");
    assert!(config.get("thinkingConfig").is_none(), "{body}");
    assert!(config.get("maxOutputTokens").is_none(), "{body}");
    assert_eq!(config["temperature"], json!(0.3));
}

#[test]
fn complete_body_carries_the_same_max_output_tokens_the_streaming_body_would() {
    // HIGH: the structured path dropped `req.max_tokens` while `chat_stream`
    // sent it — the same class as the `effort` drop below. Asserted against
    // the STREAMING body's own value, so the two can only drift together.
    // Mutation check: drop the `maxOutputTokens` insert in
    // `build_complete_body` and this fails.
    let mut req = base_request();
    req.max_tokens = Some(777);
    let stream = build_chat_stream_body(&req, SamplingProfile::default());
    let body = build_complete_body(
        &req.model,
        "sys",
        "user",
        None,
        Some(StructuredCall {
            max_tokens: req.max_tokens,
            ..structured_call()
        }),
    );
    assert_eq!(
        stream["generationConfig"]["maxOutputTokens"],
        json!(777),
        "{stream}"
    );
    assert_eq!(
        body["generationConfig"]["maxOutputTokens"],
        stream["generationConfig"]["maxOutputTokens"]
    );

    // The negative half: an unset limit must be ABSENT, never `null` —
    // Google's own per-model default is the right answer when the request has
    // no opinion.
    let unset = build_complete_body(&req.model, "sys", "user", None, Some(structured_call()));
    assert!(
        unset["generationConfig"].get("maxOutputTokens").is_none(),
        "{unset}"
    );
}

#[test]
fn complete_body_carries_the_thinking_level_only_where_the_streaming_body_would() {
    // `complete_structured` is the only non-streaming path handed the whole
    // `AiGenerateRequest`, and it dropped `effort` on the floor while
    // `chat_stream` honored it: a Gemini 3 model asked for a JSON answer ran at
    // Google's default thinking level no matter what the user picked. Both
    // paths now share ONE per-model gate. Mutation check: drop the
    // `thinkingConfig` insert in `build_complete_body` and the first assertion
    // fails.
    let body = build_complete_body(
        "gemini-3.1-pro-preview",
        "sys",
        "user",
        None,
        Some(StructuredCall {
            effort: Some("medium"),
            ..structured_call()
        }),
    );
    assert_eq!(
        body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
        json!("MEDIUM")
    );
    // NEVER `includeThoughts` here, unlike the streaming body: this path's
    // reader joins every `parts[].text`, so a thought part would land inside
    // the string the caller parses as JSON.
    assert!(
        body["generationConfig"]["thinkingConfig"]
            .get("includeThoughts")
            .is_none(),
        "{body}"
    );

    // A model that does not accept the level must never be sent it — a pre-3
    // model 400s on `thinkingLevel` outright, and a 3.x model 400s on a level
    // outside its own row of Google's table (`effort` is stored per PROVIDER,
    // so a value picked on another model reaches here unchanged).
    for (model, effort) in [
        ("gemini-2.5-flash", "medium"),
        ("gemini-3-pro-preview", "medium"),
        ("gemini-3.1-pro-preview", "minimal"),
        ("gemini-3.1-pro-preview", "   "),
    ] {
        let body = build_complete_body(
            model,
            "sys",
            "user",
            None,
            Some(StructuredCall {
                effort: Some(effort),
                ..structured_call()
            }),
        );
        assert!(
            body["generationConfig"].get("thinkingConfig").is_none(),
            "{model} must not be sent thinkingLevel {effort:?}: {body}"
        );
    }
}

/// The plain-text effort path (`complete_with_effort`): `json: false` sends no
/// JSON mode/schema, and the token cap still rides along.
#[test]
fn a_plain_call_sends_no_json_mode() {
    let body = build_complete_body(
        "gemini-2.5-flash",
        "sys",
        "user",
        None,
        Some(StructuredCall {
            json: false,
            max_tokens: Some(300),
            ..structured_call()
        }),
    );
    let config = &body["generationConfig"];
    assert!(config.get("responseMimeType").is_none(), "{body}");
    assert!(config.get("responseSchema").is_none(), "{body}");
    assert_eq!(config["maxOutputTokens"], json!(300));
}
