//! `openai::openai_response_format` / `openai::strictify` tests.

use super::super::openai::strictify;
use super::super::*;
use super::support::deep_schema;

#[test]
fn openai_response_format_degrades_to_json_object_past_the_schema_depth_cap() {
    // LOW: `strictify` recursed unbounded. Schemas are developer-authored
    // today, so the cap only ever fires on a config-supplied (or cyclic)
    // one — where blowing the stack is the alternative. Degrading to
    // `json_object` is the same fallback a non-object root already takes.
    assert_eq!(
        openai_response_format(Some(&deep_schema(MAX_SCHEMA_DEPTH + 2))),
        json!({ "type": "json_object" })
    );
    // …and a schema within the cap is still strict-mode'd.
    assert_eq!(
        openai_response_format(Some(&deep_schema(2)))["type"],
        json!("json_schema")
    );
}

#[test]
fn openai_response_format_is_strict_json_schema_with_required_and_closed_objects() {
    let schema = json!({
        "type": "object",
        "properties": { "score": { "type": "integer" }, "notes": { "type": "string" } },
    });
    let format = openai_response_format(Some(&schema));
    assert_eq!(format["type"], json!("json_schema"));
    assert_eq!(format["json_schema"]["strict"], json!(true));
    let out = &format["json_schema"]["schema"];
    assert_eq!(out["additionalProperties"], json!(false));
    // Strict mode requires EVERY property in `required` — the caller's
    // schema had none at all. Order-insensitive: it follows the schema
    // map's own iteration order, which is not part of the contract.
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
fn openai_response_format_falls_back_to_json_object_without_an_object_rooted_schema() {
    assert_eq!(
        openai_response_format(None),
        json!({ "type": "json_object" })
    );
    // A root array can't be strict-mode'd (OpenAI requires a root object).
    assert_eq!(
        openai_response_format(Some(&json!({ "type": "array" }))),
        json!({ "type": "json_object" })
    );
}

#[test]
fn strictify_closes_nested_objects_and_array_items_too() {
    let out = strictify(
        &json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": { "type": "object", "properties": { "id": { "type": "string" } } },
                },
            },
        }),
        0,
    )
    .expect("within the depth cap");
    let item = &out["properties"]["items"]["items"];
    assert_eq!(item["additionalProperties"], json!(false));
    assert_eq!(item["required"], json!(["id"]));
}

#[test]
fn openai_response_format_degrades_on_a_keyword_strict_mode_does_not_permit() {
    // Same hazard class as the composition keywords above, one level down:
    // `strictify` copies every keyword it doesn't understand VERBATIM into
    // a `strict: true` schema, and OpenAI's strict mode is an allowlist at
    // the vendor — an unlisted keyword 400s the whole request ("'minLength'
    // is not permitted") with no degrade anywhere. `minLength`/
    // `minProperties` are the realistic case rather than a hypothetical:
    // both are `GEMINI_KEPT_KEYWORDS` entries, so a schema written for this
    // codebase's OTHER native path carries them. Mutation check: drop the
    // `openai_strict_keywords_only` filter in `openai_response_format` and
    // every assertion in this loop fails.
    for (label, schema) in [
        (
            "a string constraint outside the documented subset",
            json!({
                "type": "object",
                "properties": { "id": { "type": "string", "minLength": 4 } },
            }),
        ),
        (
            "an object constraint outside the documented subset",
            json!({
                "type": "object",
                "minProperties": 1,
                "properties": { "id": { "type": "string" } },
            }),
        ),
        (
            "a documented-unsupported conditional",
            json!({
                "type": "object",
                "properties": { "n": { "type": "integer" } },
                "if": { "type": "object" },
                "then": { "type": "object" },
            }),
        ),
        (
            "a documented-unsupported dependency",
            json!({
                "type": "object",
                "properties": { "n": { "type": "integer" } },
                "dependentRequired": { "n": ["m"] },
            }),
        ),
        (
            "an unsupported keyword nested under an array's items",
            json!({
                "type": "object",
                "properties": {
                    "tags": {
                        "type": "array",
                        "items": { "type": "string", "maxLength": 8 },
                    },
                },
            }),
        ),
    ] {
        assert_eq!(
            openai_response_format(Some(&schema)),
            json!({ "type": "json_object" }),
            "{label} must degrade, not ship inside `strict: true`"
        );
    }
}

#[test]
fn openai_response_format_keeps_every_documented_strict_mode_constraint() {
    // The other half of the allowlist: degrading is only honest if the
    // subset OpenAI DOES document still reaches the decoder. Every keyword
    // here is from the live "Supported properties" list, and each one is
    // asserted to survive `strictify` verbatim — the guard must never
    // become "strip the constraint and ship anyway", which weakens
    // validation invisibly.
    let schema = json!({
        "type": "object",
        "title": "Result",
        "description": "an ATS verdict",
        "properties": {
            "id": { "type": "string", "pattern": "^[A-Z]{2}-\\d+$" },
            "kind": { "type": "string", "enum": ["ats", "manual"] },
            "seen": { "type": "string", "format": "date-time" },
            "score": {
                "type": "integer",
                "minimum": 0,
                "maximum": 100,
                "multipleOf": 5,
            },
            "ratio": {
                "type": "number",
                "exclusiveMinimum": 0,
                "exclusiveMaximum": 1,
            },
            "tags": {
                "type": "array",
                "minItems": 1,
                "maxItems": 5,
                "items": { "type": "string" },
            },
        },
    });
    let format = openai_response_format(Some(&schema));
    assert_eq!(format["type"], json!("json_schema"));
    let out = &format["json_schema"]["schema"];
    assert_eq!(out["title"], json!("Result"));
    assert_eq!(out["properties"]["id"]["pattern"], json!("^[A-Z]{2}-\\d+$"));
    assert_eq!(out["properties"]["kind"]["enum"], json!(["ats", "manual"]));
    assert_eq!(out["properties"]["seen"]["format"], json!("date-time"));
    assert_eq!(out["properties"]["score"]["multipleOf"], json!(5));
    assert_eq!(out["properties"]["ratio"]["exclusiveMaximum"], json!(1));
    assert_eq!(out["properties"]["tags"]["maxItems"], json!(5));
    // …and the strict-mode requirements are still added on top.
    assert_eq!(out["additionalProperties"], json!(false));
    assert_eq!(out["properties"]["tags"]["items"]["type"], json!("string"));
}
