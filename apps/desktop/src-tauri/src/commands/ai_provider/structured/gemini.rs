//! Gemini's `responseSchema` (OpenAPI-3.0 subset) translator. Split out of
//! `structured.rs` (R8 line-budget split): one provider's wire-shape
//! translator per file.

use serde_json::{json, Map, Value};

use super::{has_untranslatable_keyword, MAX_SCHEMA_DEPTH};

/// Keywords Gemini's OpenAPI-subset `Schema` shares verbatim with JSON Schema
/// — every field the live `Schema` reference (`ai.google.dev/api`, checked
/// 2026-08-10) documents AND this translator can pass through untouched.
/// Anything NOT listed here is dropped, in two very different classes:
///
/// - **Fields Gemini does not document at all** (`additionalProperties`,
///   `$schema`, …). Gemini rejects unknown fields, so they cannot be sent;
///   they are also the ones whose loss costs no constraint (`additionalProperties`
///   has no `Schema` equivalent — Gemini's own strictness is a different knob).
/// - **Documented fields this translator deliberately does not carry**:
///   `title`/`example`/`default`/`propertyOrdering` (presentation, not
///   constraint — dropping them changes nothing the model would have honored),
///   and `anyOf`, whose sub-schemas would need translating too and which
///   [`super::has_untranslatable_keyword`] therefore degrades the WHOLE schema
///   on rather than silently weakening it.
///
/// The value CONSTRAINTS below are the point of the list: `pattern`,
/// `minLength`/`maxLength`, `minimum`/`maximum` and
/// `minProperties`/`maxProperties` are real, documented Gemini fields that
/// constrain what the model may emit — a regex or a numeric bound is not
/// "purely descriptive", and silently dropping one is exactly the weakened
/// constraint [`gemini_response_schema`] refuses to send elsewhere.
const GEMINI_KEPT_KEYWORDS: &[&str] = &[
    "description",
    "enum",
    "format",
    "maxItems",
    "maxLength",
    "maxProperties",
    "maximum",
    "minItems",
    "minLength",
    "minProperties",
    "minimum",
    "nullable",
    "pattern",
    "required",
];

/// Translate a JSON Schema into Gemini's `responseSchema` dialect (an
/// OpenAPI-3.0 subset): JSON Schema's lowercase `type` becomes the
/// proto-JSON enum NAME (`"object"` → `"OBJECT"`), unsupported keywords are
/// dropped, and `properties`/`items` recurse.
///
/// `None` — meaning "fall back to `responseMimeType` + the prompt hint" —
/// whenever ANY part of the schema has no equivalent (a missing/unknown
/// `type`, or a union type like `["string","null"]`, which arrives as an
/// array and is not a `&str`), when it composes or references another schema
/// ([`super::COMPOSITION_KEYWORDS`] — the SAME degrade OpenAI takes, so one
/// schema never lands strict-mode'd on one provider and silently weakened on
/// the other), and likewise past [`MAX_SCHEMA_DEPTH`].
/// Failing the WHOLE schema rather than dropping the untranslatable property
/// is deliberate: a dropped property silently stops constraining a field the
/// caller asked to constrain, which is the silent-truncation failure mode this
/// codebase rejects elsewhere.
pub(in crate::commands::ai_provider) fn gemini_response_schema(schema: &Value) -> Option<Value> {
    if has_untranslatable_keyword(schema, 0) {
        return None;
    }
    gemini_schema_at(schema, 0)
}

/// [`gemini_response_schema`]'s recursion, carrying the nesting depth.
fn gemini_schema_at(schema: &Value, depth: usize) -> Option<Value> {
    if depth > MAX_SCHEMA_DEPTH {
        return None;
    }
    let obj = schema.as_object()?;
    let wire_type = match obj.get("type").and_then(Value::as_str)? {
        "object" => "OBJECT",
        "array" => "ARRAY",
        "string" => "STRING",
        "integer" => "INTEGER",
        "number" => "NUMBER",
        "boolean" => "BOOLEAN",
        _ => return None,
    };
    let mut out = Map::new();
    out.insert("type".to_string(), json!(wire_type));
    for key in GEMINI_KEPT_KEYWORDS {
        if let Some(value) = obj.get(*key) {
            out.insert((*key).to_string(), value.clone());
        }
    }
    if let Some(props) = obj.get("properties").and_then(Value::as_object) {
        let mut mapped = Map::new();
        for (key, value) in props {
            mapped.insert(key.clone(), gemini_schema_at(value, depth + 1)?);
        }
        out.insert("properties".to_string(), Value::Object(mapped));
    }
    if let Some(items) = obj.get("items") {
        out.insert("items".to_string(), gemini_schema_at(items, depth + 1)?);
    }
    Some(Value::Object(out))
}
