//! Anthropic's `output_config.format` structured-output translator. Split
//! out of `structured.rs` (R8 line-budget split): one provider's wire-shape
//! translator per file. Reuses [`super::openai::strictify`] and
//! [`super::openai::OPENAI_STRICT_KEYWORDS`] — Anthropic's schema subset is a
//! narrower version of the same closed-object shape OpenAI's strict mode
//! needs.

use serde_json::{json, Value};

use super::openai::{strictify, OPENAI_STRICT_KEYWORDS};
use super::{has_untranslatable_keyword, MAX_SCHEMA_DEPTH};

/// `format` values Anthropic's structured-outputs schema subset documents
/// (`platform.claude.com/docs/en/build-with-claude/structured-outputs`).
/// Anything else must fail the schema — raw HTTP, no SDK transform to strip it.
const ANTHROPIC_ACCEPTED_FORMATS: &[&str] = &[
    "date-time",
    "time",
    "date",
    "duration",
    "email",
    "hostname",
    "uri",
    "ipv4",
    "ipv6",
    "uuid",
];

/// The Anthropic-side mirror of `openai::openai_strict_keywords_only` — same
/// walk (root, `properties` values, `items`) and the same
/// [`OPENAI_STRICT_KEYWORDS`] allowlist, PLUS extra rejections for value
/// constraints Anthropic's schema subset doesn't document at all: `minimum`,
/// `maximum`, `exclusiveMinimum`, `exclusiveMaximum`, `multipleOf`,
/// `pattern`, `maxItems` (any value); `minItems` outside `{0, 1}`; `format`
/// outside [`ANTHROPIC_ACCEPTED_FORMATS`]. A separate, stricter check rather
/// than a shared one because — unlike Anthropic's own SDKs, which strip
/// unsupported constraints client-side — this adapter builds `output_config`
/// by hand over raw HTTP, so a schema valid for OpenAI strict mode (e.g.
/// `"minimum": 0`) would otherwise 400 the whole Anthropic generation.
fn anthropic_strict_keywords_only(schema: &Value, depth: usize) -> bool {
    if depth > MAX_SCHEMA_DEPTH {
        return false;
    }
    let Some(obj) = schema.as_object() else {
        return false;
    };
    if !obj
        .keys()
        .all(|key| OPENAI_STRICT_KEYWORDS.contains(&key.as_str()))
    {
        return false;
    }
    const REJECTED_UNCONDITIONALLY: &[&str] = &[
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "exclusiveMaximum",
        "multipleOf",
        "pattern",
        "maxItems",
    ];
    if obj
        .keys()
        .any(|key| REJECTED_UNCONDITIONALLY.contains(&key.as_str()))
    {
        return false;
    }
    if let Some(min_items) = obj.get("minItems") {
        if !matches!(min_items.as_u64(), Some(0) | Some(1)) {
            return false;
        }
    }
    if let Some(format) = obj.get("format") {
        let accepted = format
            .as_str()
            .is_some_and(|f| ANTHROPIC_ACCEPTED_FORMATS.contains(&f));
        if !accepted {
            return false;
        }
    }
    obj.get("properties")
        .and_then(Value::as_object)
        .is_none_or(|props| {
            props
                .values()
                .all(|value| anthropic_strict_keywords_only(value, depth + 1))
        })
        && obj
            .get("items")
            .is_none_or(|items| anthropic_strict_keywords_only(items, depth + 1))
}

/// Anthropic's `output_config.format` for `schema`, or `None` when there is
/// nothing it can constrain against — the caller then stays on
/// [`super::prompt_only`]. Same object-rooted-and-closed gate as
/// `openai::openai_response_format`'s strict branch, PLUS
/// [`anthropic_strict_keywords_only`]'s narrower value-constraint check —
/// Anthropic's schema subset accepts fewer constraints than OpenAI's.
pub(super) fn anthropic_output_format(schema: Option<&Value>) -> Option<Value> {
    schema
        .filter(|s| s.get("type").and_then(Value::as_str) == Some("object"))
        .filter(|s| !has_untranslatable_keyword(s, 0))
        .filter(|s| anthropic_strict_keywords_only(s, 0))
        .and_then(|schema| strictify(schema, 0))
        .map(|schema| json!({ "type": "json_schema", "schema": schema }))
}

/// Anthropic's `output_config` body field: [`anthropic_output_format`]
/// wrapped under `"format"`, plus `"effort"` when the caller passes one —
/// already gated against this model's tier by the caller
/// (`anthropic::AnthropicClient::complete_structured`), since that gate needs
/// the model id this shape-only translator deliberately doesn't take.
/// `None` — the same "stay on [`super::prompt_only`]" signal as
/// [`anthropic_output_format`] — when there is no usable schema at all.
pub(in crate::commands::ai_provider) fn anthropic_output_config(
    schema: Option<&Value>,
    effort: Option<&str>,
) -> Option<Value> {
    let mut output_config = json!({ "format": anthropic_output_format(schema)? });
    if let Some(effort) = effort {
        output_config["effort"] = json!(effort);
    }
    Some(output_config)
}
