//! `gemini::gemini_response_schema` and `ollama_format` tests.

use super::super::*;
use super::support::deep_schema;

#[test]
fn gemini_response_schema_fails_the_whole_schema_past_the_depth_cap() {
    assert!(gemini_response_schema(&deep_schema(MAX_SCHEMA_DEPTH + 2)).is_none());
    assert!(gemini_response_schema(&deep_schema(2)).is_some());
}

#[test]
fn gemini_response_schema_uppercases_types_and_drops_unsupported_keywords() {
    let out = gemini_response_schema(&json!({
        "type": "object",
        "title": "Result",
        "additionalProperties": false,
        "required": ["score"],
        "properties": {
            "score": { "type": "integer", "description": "0-100" },
            "tags": { "type": "array", "items": { "type": "string" } },
        },
    }))
    .expect("translatable");
    assert_eq!(out["type"], json!("OBJECT"));
    assert_eq!(out["required"], json!(["score"]));
    assert_eq!(out["properties"]["score"]["type"], json!("INTEGER"));
    assert_eq!(out["properties"]["score"]["description"], json!("0-100"));
    assert_eq!(out["properties"]["tags"]["items"]["type"], json!("STRING"));
    assert!(out.get("title").is_none());
    assert!(out.get("additionalProperties").is_none());
}

#[test]
fn gemini_response_schema_keeps_documented_value_constraints() {
    // `pattern`, `minLength`/`maxLength` and `minimum`/`maximum` are real
    // fields of Gemini's live `Schema` object, and each one CONSTRAINS what
    // the model may emit — dropping them (as "purely descriptive") shipped
    // a weaker constraint than the caller asked for, which is exactly what
    // this translator refuses to do everywhere else. Mutation check: remove
    // them from `GEMINI_KEPT_KEYWORDS` and this fails.
    let out = gemini_response_schema(&json!({
        "type": "object",
        "minProperties": 1,
        "properties": {
            "id": { "type": "string", "pattern": "^[A-Z]{2}-\\d+$", "minLength": 4 },
            "score": { "type": "integer", "minimum": 0, "maximum": 100 },
        },
    }))
    .expect("translatable");
    assert_eq!(out["minProperties"], json!(1));
    assert_eq!(out["properties"]["id"]["pattern"], json!("^[A-Z]{2}-\\d+$"));
    assert_eq!(out["properties"]["id"]["minLength"], json!(4));
    assert_eq!(out["properties"]["score"]["minimum"], json!(0));
    assert_eq!(out["properties"]["score"]["maximum"], json!(100));
}

#[test]
fn gemini_response_schema_rejects_the_whole_schema_when_one_property_is_untranslatable() {
    // A union type arrives as an array, not a `&str` — no OpenAPI-subset
    // equivalent. Dropping just that property would silently stop
    // constraining it, so the whole translation fails and the caller
    // falls back to `responseMimeType` + the prompt hint.
    assert!(gemini_response_schema(&json!({
        "type": "object",
        "properties": { "note": { "type": ["string", "null"] } },
    }))
    .is_none());
    assert!(gemini_response_schema(&json!({ "properties": {} })).is_none());
    assert!(gemini_response_schema(&json!("nonsense")).is_none());
}

#[test]
fn ollama_format_passes_the_schema_through_and_falls_back_to_the_json_string() {
    let schema = json!({ "type": "object", "properties": {} });
    assert_eq!(ollama_format(Some(&schema)), schema);
    assert_eq!(ollama_format(None), json!("json"));
}
