//! The shared half of [`AiProvider::complete_structured`](super::AiProvider::complete_structured):
//! the prompt-discipline fallback every provider gets for free, plus ONE pure
//! translator per provider that has a native constrained-decoding field.
//!
//! Why the per-provider translators live together rather than in each adapter:
//! they all translate the SAME caller-supplied flat JSON Schema into a
//! different vendor dialect, so a hardening applied to one and silently not
//! the others is exactly the drift this codebase keeps re-discovering (see
//! [`super::pagination`]'s module doc for the same argument). They are pure
//! `Value -> Value` functions with no HTTP, so each adapter still owns its own
//! transport; only the shape mapping is shared. (`mod.rs`, `openai.rs` and
//! `gemini.rs` are also all within a few dozen lines of the R8 LOC cap, so
//! there is nowhere else for this to go.)
//!
//! **The prompt-discipline path is the PERMANENT fallback**, not a stopgap: a
//! provider with no native JSON mode, an unknown gateway, a CLI agent, or a
//! caller that has an example but no schema all land here and must keep
//! working. No caller may require native constrained decoding.

use serde_json::{json, Map, Value};
use tauri::AppHandle;

use crate::error::AppResult;

use super::{resolve_intent, AiGenerateRequest, AiProvider, Usage};

/// The trusted instruction appended to the SYSTEM slot (never the user slot —
/// untrusted résumé/job-ad text rides there, and mixing an instruction into it
/// is the OWASP LLM01 mistake this codebase segregates against everywhere
/// else). Deliberately mentions "JSON" verbatim: OpenAI's `json_object`
/// response format REJECTS a request whose messages never say the word.
const JSON_ONLY_DIRECTIVE: &str = "Output contract: reply with ONE valid JSON value and nothing \
else — no prose, no preamble, no explanation, no Markdown code fence. Use exactly the keys, \
nesting and value types shown in the example below; the example's VALUES are placeholders and \
must never be copied. Do not add keys. When you have no value for a key, still emit it with an \
empty/neutral value of the right type.";

/// Build the `(system, user)` pair for a structured completion: the request's
/// system messages, then [`JSON_ONLY_DIRECTIVE`], then the filled-example
/// `schema_hint`; every non-system message concatenated into the user slot.
///
/// The directive/hint go at the END of the system slot so the static prefix
/// (the caller's own system prompt) is byte-identical to the non-structured
/// call — prompt caching keys on that prefix.
///
/// Applied on the native paths too, not just the fallback: every vendor
/// documents constrained decoding as "still describe the shape you want", and
/// it keeps a schema-less caller (hint only) identical across providers. Pure
/// + unit-tested.
pub(super) fn structured_prompt(req: &AiGenerateRequest, schema_hint: &str) -> (String, String) {
    let join = |keep_system: bool| {
        req.messages
            .iter()
            .filter(|m| (m.role == "system") == keep_system)
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let mut system = join(true);
    if !system.is_empty() {
        system.push_str("\n\n");
    }
    system.push_str(JSON_ONLY_DIRECTIVE);
    let hint = schema_hint.trim();
    if !hint.is_empty() {
        system.push_str("\n\nExample of the required shape:\n");
        system.push_str(hint);
    }
    (system, join(false))
}

/// The provider's OWN temperature for this request — never a hardcoded number
/// here. Routes through [`AiProvider::sampling_profile`] + the request's
/// explicit overrides exactly like `chat_stream`, so a model that must not be
/// sent a sampling parameter at all still gets `None` (see
/// [`super::SamplingProfile`]'s doc comment). Passing `req.temperature`
/// straight through would instead hit each adapter's `unwrap_or(0.7)` — a
/// creative-writing temperature on a JSON call.
pub(super) fn structured_temperature<P: AiProvider + ?Sized>(
    provider: &P,
    req: &AiGenerateRequest,
) -> Option<f64> {
    provider
        .sampling_profile(&req.model, resolve_intent(req))
        .resolve(req)
        .temperature
}

/// The prompt-discipline-only structured completion — the body of
/// [`AiProvider::complete_structured`](super::AiProvider::complete_structured)'s
/// default, and the fallback every native override returns to when it has no
/// usable schema. Generic over `?Sized` so it works from the trait default
/// (`&Self`) and from a concrete adapter, exactly like
/// [`super::single_shot_turn`].
pub(super) async fn prompt_only<P: AiProvider + ?Sized>(
    provider: &P,
    app: &AppHandle,
    req: &AiGenerateRequest,
    schema_hint: &str,
) -> AppResult<(String, Usage)> {
    let (system, user) = structured_prompt(req, schema_hint);
    let temperature = structured_temperature(provider, req);
    provider
        .complete_with_usage(app, &req.model, &system, &user, temperature)
        .await
}

// ── Per-provider wire shapes ─────────────────────────────────────────────────

/// OpenAI's `response_format`: strict `json_schema` when the caller supplied
/// an object-rooted schema, else plain `json_object` mode (which constrains
/// only "is JSON", relying on the directive + hint for the shape).
///
/// Strict mode has two schema requirements the caller's plain JSON Schema
/// usually doesn't meet — every property listed in `required`, and
/// `additionalProperties: false` on every object — so [`strictify`] adds them
/// rather than 400ing on a schema that is otherwise perfectly valid. A
/// non-object root can't be strict-mode'd at all (OpenAI requires a root
/// object), so it degrades to `json_object` instead of being rejected.
pub(super) fn openai_response_format(schema: Option<&Value>) -> Value {
    match schema.filter(|s| s.get("type").and_then(Value::as_str) == Some("object")) {
        Some(schema) => json!({
            "type": "json_schema",
            "json_schema": {
                "name": "structured_output",
                "strict": true,
                "schema": strictify(schema),
            },
        }),
        None => json!({ "type": "json_object" }),
    }
}

/// Add OpenAI strict-mode's two structural requirements to `schema`,
/// recursively (a flat schema bottoms out immediately; nesting is handled so a
/// slightly-less-flat caller can't silently 400): every object gets
/// `additionalProperties: false` and a `required` listing ALL of its
/// properties. Everything else the caller wrote is preserved verbatim.
fn strictify(schema: &Value) -> Value {
    let Some(obj) = schema.as_object() else {
        return schema.clone();
    };
    let mut out = obj.clone();
    match obj.get("type").and_then(Value::as_str) {
        Some("object") => {
            if let Some(props) = obj.get("properties").and_then(Value::as_object) {
                out.insert(
                    "required".to_string(),
                    Value::Array(props.keys().map(|k| json!(k)).collect()),
                );
                out.insert(
                    "properties".to_string(),
                    Value::Object(
                        props
                            .iter()
                            .map(|(k, v)| (k.clone(), strictify(v)))
                            .collect(),
                    ),
                );
            }
            out.insert("additionalProperties".to_string(), json!(false));
        }
        Some("array") => {
            if let Some(items) = obj.get("items") {
                out.insert("items".to_string(), strictify(items));
            }
        }
        _ => {}
    }
    Value::Object(out)
}

/// Keywords Gemini's OpenAPI-subset `Schema` shares verbatim with JSON Schema.
/// Anything NOT listed here (`additionalProperties`, `$schema`, `title`,
/// `pattern`, `default`, …) is dropped — Gemini rejects unknown fields, and
/// dropping a purely-descriptive keyword loses no constraint the model would
/// have honored anyway.
const GEMINI_KEPT_KEYWORDS: &[&str] = &[
    "description",
    "enum",
    "format",
    "maxItems",
    "minItems",
    "nullable",
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
/// array and is not a `&str`). Failing the WHOLE schema rather than dropping
/// the untranslatable property is deliberate: a dropped property silently
/// stops constraining a field the caller asked to constrain, which is the
/// silent-truncation failure mode this codebase rejects elsewhere.
pub(super) fn gemini_response_schema(schema: &Value) -> Option<Value> {
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
            mapped.insert(key.clone(), gemini_response_schema(value)?);
        }
        out.insert("properties".to_string(), Value::Object(mapped));
    }
    if let Some(items) = obj.get("items") {
        out.insert("items".to_string(), gemini_response_schema(items)?);
    }
    Some(Value::Object(out))
}

/// Ollama's `format` field: the JSON Schema itself when the caller has one
/// (Ollama constrains decoding against it directly — no dialect translation,
/// unlike Gemini), else the `"json"` string, which only guarantees valid JSON.
pub(super) fn ollama_format(schema: Option<&Value>) -> Value {
    match schema {
        Some(schema) => schema.clone(),
        None => json!("json"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc_contracts::ai::AiGenerateRequestMessage;

    fn request(messages: &[(&str, &str)]) -> AiGenerateRequest {
        AiGenerateRequest {
            model: "llama3.1:8b".to_string(),
            messages: messages
                .iter()
                .map(|(role, content)| AiGenerateRequestMessage {
                    role: (*role).to_string(),
                    content: (*content).to_string(),
                })
                .collect(),
            locale: "en".to_string(),
            temperature: None,
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
    fn structured_prompt_appends_the_directive_and_the_filled_example_to_the_system_slot() {
        let (system, user) = structured_prompt(
            &request(&[("system", "You are an ATS."), ("user", "résumé text")]),
            r#"{"score": 72}"#,
        );
        assert!(system.starts_with("You are an ATS.\n\n"));
        assert!(system.contains(JSON_ONLY_DIRECTIVE));
        assert!(system.contains(r#"{"score": 72}"#));
        // The untrusted half stays untouched — no instruction is ever mixed
        // into the slot carrying résumé/job-ad text (OWASP LLM01).
        assert_eq!(user, "résumé text");
        assert!(!user.contains(JSON_ONLY_DIRECTIVE));
    }

    #[test]
    fn structured_prompt_keeps_the_callers_system_prefix_byte_identical() {
        // Prompt caching keys on the static prefix: the directive must be
        // appended AFTER the caller's system prompt, never prepended.
        let prefix = "You are an ATS.";
        let (system, _) = structured_prompt(&request(&[("system", prefix)]), "{}");
        assert_eq!(&system[..prefix.len()], prefix);
    }

    #[test]
    fn structured_prompt_survives_an_empty_system_and_an_empty_hint() {
        let (system, user) = structured_prompt(&request(&[("user", "hi")]), "   ");
        assert_eq!(system, JSON_ONLY_DIRECTIVE);
        assert!(!system.contains("Example of the required shape"));
        assert_eq!(user, "hi");
    }

    #[test]
    fn structured_prompt_concatenates_multiple_messages_per_slot() {
        let (system, user) = structured_prompt(
            &request(&[
                ("system", "rule one"),
                ("user", "first"),
                ("assistant", "second"),
            ]),
            "",
        );
        assert!(system.starts_with("rule one\n\n"));
        assert_eq!(user, "first\n\nsecond");
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
        let out = strictify(&json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": { "type": "object", "properties": { "id": { "type": "string" } } },
                },
            },
        }));
        let item = &out["properties"]["items"]["items"];
        assert_eq!(item["additionalProperties"], json!(false));
        assert_eq!(item["required"], json!(["id"]));
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
}
