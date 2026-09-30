//! `anthropic::anthropic_output_format` / `anthropic_output_config` tests.

use super::super::anthropic::anthropic_output_format;
use super::super::*;

#[test]
fn anthropic_output_format_is_none_without_a_usable_schema() {
    assert_eq!(anthropic_output_format(None), None);
    // A root array can't be closed into an object schema.
    assert_eq!(
        anthropic_output_format(Some(&json!({ "type": "array" }))),
        None
    );
    // A composition keyword neither `strictify` nor `openai_strict_keywords_only`
    // walks into — same degrade as the OpenAI side.
    let anyof = json!({
        "type": "object",
        "properties": {
            "note": {
                "anyOf": [
                    { "type": "object", "properties": { "text": { "type": "string" } } },
                    { "type": "string" },
                ],
            },
        },
    });
    assert_eq!(anthropic_output_format(Some(&anyof)), None);
}

#[test]
fn anthropic_output_format_closes_a_flat_object_schema() {
    let schema = json!({
        "type": "object",
        "properties": { "score": { "type": "integer" }, "notes": { "type": "string" } },
    });
    let format = anthropic_output_format(Some(&schema)).expect("translatable");
    assert_eq!(format["type"], json!("json_schema"));
    let out = &format["schema"];
    assert_eq!(out["additionalProperties"], json!(false));
    let mut required: Vec<&str> = out["required"]
        .as_array()
        .expect("required array")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    required.sort_unstable();
    assert_eq!(required, ["notes", "score"]);
}

#[test]
fn anthropic_output_config_wraps_the_format_and_adds_effort_only_when_given() {
    let schema = json!({ "type": "object", "properties": { "score": { "type": "integer" } } });
    let format = anthropic_output_config(Some(&schema), None).expect("translatable");
    assert_eq!(format["format"]["type"], json!("json_schema"));
    assert!(format.get("effort").is_none());

    let with_effort = anthropic_output_config(Some(&schema), Some("xhigh")).expect("translatable");
    assert_eq!(with_effort["effort"], json!("xhigh"));
}

#[test]
fn anthropic_output_config_is_none_without_a_usable_schema() {
    assert_eq!(anthropic_output_config(None, Some("xhigh")), None);
}

#[test]
fn anthropic_output_format_rejects_value_constraints_its_schema_subset_does_not_document() {
    // Nested at least one level deep (properties/items) so the recursive
    // walk is exercised, not just the root. Mutation check: drop
    // `anthropic_strict_keywords_only`'s extra rejections and every
    // assertion below fails.
    let cases = [
        (
            "a `minimum` nested in properties",
            json!({
                "type": "object",
                "properties": { "score": { "type": "integer", "minimum": 0 } },
            }),
        ),
        (
            "a `pattern` nested in properties",
            json!({
                "type": "object",
                "properties": { "id": { "type": "string", "pattern": "^[A-Z]+$" } },
            }),
        ),
        (
            "a `maxItems` nested under an array property",
            json!({
                "type": "object",
                "properties": {
                    "tags": { "type": "array", "maxItems": 3, "items": { "type": "string" } },
                },
            }),
        ),
        (
            "a `minItems` outside {0, 1}",
            json!({
                "type": "object",
                "properties": {
                    "tags": { "type": "array", "minItems": 2, "items": { "type": "string" } },
                },
            }),
        ),
        (
            "an unsupported `format`",
            json!({
                "type": "object",
                "properties": { "note": { "type": "string", "format": "regex" } },
            }),
        ),
    ];
    for (label, schema) in cases {
        assert_eq!(
            anthropic_output_format(Some(&schema)),
            None,
            "{label} must degrade to prompt_only, not ship inside output_config"
        );
    }
}

#[test]
fn anthropic_output_format_accepts_its_documented_min_items_and_format_values() {
    // The other half of the allowlist: degrading is only honest if the
    // values Anthropic's docs DO accept still reach the decoder.
    let schema = json!({
        "type": "object",
        "properties": {
            "tags": { "type": "array", "minItems": 1, "items": { "type": "string" } },
            "seen": { "type": "string", "format": "email" },
        },
    });
    assert!(anthropic_output_format(Some(&schema)).is_some());
}
