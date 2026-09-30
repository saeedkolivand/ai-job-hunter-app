//! OpenAI's strict `json_schema` translator. Split out of `structured.rs`
//! (R8 line-budget split): one provider's wire-shape translator per file.
//! [`strictify`] and [`OPENAI_STRICT_KEYWORDS`] are `pub(super)` because
//! `structured::anthropic` reuses both (Anthropic's schema subset is a
//! narrower version of the same closed-object shape).

use serde_json::{json, Map, Value};

use super::{has_untranslatable_keyword, MAX_SCHEMA_DEPTH};

/// The ONLY JSON Schema keywords OpenAI's strict `json_schema` mode accepts —
/// its "Supported types" + "Supported properties" lists verbatim
/// (`platform.openai.com/docs/guides/structured-outputs`, read 2026-08-10),
/// plus the structural keys [`strictify`] itself writes. Strict mode is an
/// ALLOWLIST at the vendor: an unlisted keyword is not ignored, it 400s the
/// whole request ("If you turn on Structured Outputs by supplying
/// `strict: true` and call the API with an unsupported JSON Schema, you will
/// receive an error" — e.g. `'minLength' is not permitted`).
///
/// `title` is not in that doc list but is provably accepted: every
/// Pydantic-generated schema carries one on every model and field, and
/// OpenAI's own `_ensure_strict_json_schema` helper (the blessed strict-mode
/// input path) leaves it in place.
///
/// Deliberately NOT listed even though OpenAI documents them: `anyOf`, `$ref`
/// and `$defs`. [`super::COMPOSITION_KEYWORDS`] already degrades those one
/// filter earlier for cross-provider consistency, so listing them here would
/// be dead — and if that filter ever went away, this list would keep failing
/// them in the safe direction rather than letting [`strictify`] stamp
/// `strict: true` over a subtree it never walked.
///
/// Under-listing costs a degrade to `json_object` (the shape is still asked
/// for, by the directive + filled example, just not decoder-constrained);
/// over-listing costs a 400 with no degrade at all. Anything ambiguous
/// therefore stays off the list.
pub(super) const OPENAI_STRICT_KEYWORDS: &[&str] = &[
    // Structure — the three the caller writes plus the two `strictify` adds.
    "additionalProperties",
    "items",
    "properties",
    "required",
    "type",
    // Annotations OpenAI's own examples/SDK path carry.
    "description",
    "enum",
    "title",
    // Documented `string` constraints.
    "format",
    "pattern",
    // Documented `number` constraints.
    "exclusiveMaximum",
    "exclusiveMinimum",
    "maximum",
    "minimum",
    "multipleOf",
    // Documented `array` constraints.
    "maxItems",
    "minItems",
];

/// Whether every schema node [`strictify`] will emit is an object carrying
/// ONLY [`OPENAI_STRICT_KEYWORDS`] — the OpenAI-side mirror of
/// [`super::has_untranslatable_keyword`], and the reason a schema written for
/// another provider's dialect degrades instead of 400ing.
///
/// "Is an object" is half the check, not a precondition: the non-object schema
/// positions (tuple-form `items`, a boolean schema) are exactly the ones
/// [`strictify`] cannot close, so they are rejected here — see the comment on
/// that arm below.
///
/// Rides along with [`strictify`]'s OWN walk (`properties` values + `items`)
/// rather than scanning the raw [`Value`] tree the way
/// [`super::has_untranslatable_keyword`] does, because here the distinction
/// matters: the KEYS of a `properties` map are the caller's field names, not
/// keywords, and a blind tree scan would degrade every schema that has a
/// field called anything at all.
///
/// Stripping the offending keyword instead was the tempting alternative and is
/// the worse one: it would silently ship a WEAKER constraint than the caller
/// wrote (the `min_length` that stops being enforced is invisible in the
/// response), which is exactly what [`super::gemini::gemini_response_schema`]
/// refuses to do on its own side. Degrading the whole schema is the honest
/// failure.
fn openai_strict_keywords_only(schema: &Value, depth: usize) -> bool {
    if depth > MAX_SCHEMA_DEPTH {
        return false;
    }
    // Every position this walk reaches — the root, a `properties` value, an
    // `items` value — must be a schema OBJECT. A non-object here is a shape
    // neither this vetter nor [`strictify`] can descend into, and both used to
    // bottom out returning "fine", so the subtree shipped VERBATIM under
    // `strict: true`. The reachable cases are TUPLE-form `items` (an array of
    // schemas — draft-07 tuple validation, `prefixItems` in 2020-12) and a
    // boolean schema (`items: true`); OpenAI's strict subset documents neither
    // (its array properties are `minItems`/`maxItems` — see
    // [`OPENAI_STRICT_KEYWORDS`]), and an undocumented construct in strict mode
    // 400s the whole request. So this fails the schema — the SAME whole-schema
    // degrade [`super::COMPOSITION_KEYWORDS`] takes, for the same reason.
    let Some(obj) = schema.as_object() else {
        return false;
    };
    obj.keys()
        .all(|key| OPENAI_STRICT_KEYWORDS.contains(&key.as_str()))
        && obj
            .get("properties")
            .and_then(Value::as_object)
            .is_none_or(|props| {
                props
                    .values()
                    .all(|value| openai_strict_keywords_only(value, depth + 1))
            })
        && obj
            .get("items")
            .is_none_or(|items| openai_strict_keywords_only(items, depth + 1))
}

/// OpenAI's `response_format`: strict `json_schema` when the caller supplied
/// an object-rooted schema, else plain `json_object` mode (which constrains
/// only "is JSON", relying on the directive + hint for the shape).
///
/// Strict mode has two schema requirements the caller's plain JSON Schema
/// usually doesn't meet — every property listed in `required`, and
/// `additionalProperties: false` on every object — so [`strictify`] adds them
/// rather than 400ing on a schema that is otherwise perfectly valid. A
/// non-object root can't be strict-mode'd at all (OpenAI requires a root
/// object), so it degrades to `json_object` instead of being rejected — and so
/// does a schema nested past [`MAX_SCHEMA_DEPTH`], one carrying a
/// [`super::COMPOSITION_KEYWORDS`] entry [`strictify`] cannot close, one
/// carrying any keyword outside [`OPENAI_STRICT_KEYWORDS`], and one carrying a
/// non-object schema node ([`openai_strict_keywords_only`] — tuple-form
/// `items` is the realistic case).
pub(in crate::commands::ai_provider) fn openai_response_format(schema: Option<&Value>) -> Value {
    match schema
        .filter(|s| s.get("type").and_then(Value::as_str) == Some("object"))
        .filter(|s| !has_untranslatable_keyword(s, 0))
        .filter(|s| openai_strict_keywords_only(s, 0))
        .and_then(|schema| strictify(schema, 0))
    {
        Some(schema) => json!({
            "type": "json_schema",
            "json_schema": {
                "name": "structured_output",
                "strict": true,
                "schema": schema,
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
///
/// `None` past [`MAX_SCHEMA_DEPTH`] — failing the whole schema (the caller
/// falls back to `json_object`) rather than emitting a subtree that silently
/// isn't strict, which would 400 at the vendor instead. The other two ways to
/// reach a not-actually-strict subtree are both screened out by the caller
/// before this runs: a composition keyword this walker never descends into
/// (see [`super::COMPOSITION_KEYWORDS`]) and a non-object node it cannot
/// descend into at all — tuple-form `items`, which the non-object arm below
/// would otherwise clone through verbatim (see [`openai_strict_keywords_only`]).
///
/// `pub(super)`: reused by `structured::anthropic`, whose schema subset needs
/// the SAME structural closer.
pub(super) fn strictify(schema: &Value, depth: usize) -> Option<Value> {
    if depth > MAX_SCHEMA_DEPTH {
        return None;
    }
    let Some(obj) = schema.as_object() else {
        return Some(schema.clone());
    };
    let mut out = obj.clone();
    match obj.get("type").and_then(Value::as_str) {
        Some("object") => {
            if let Some(props) = obj.get("properties").and_then(Value::as_object) {
                out.insert(
                    "required".to_string(),
                    Value::Array(props.keys().map(|k| json!(k)).collect()),
                );
                let mut mapped = Map::new();
                for (key, value) in props {
                    mapped.insert(key.clone(), strictify(value, depth + 1)?);
                }
                out.insert("properties".to_string(), Value::Object(mapped));
            }
            out.insert("additionalProperties".to_string(), json!(false));
        }
        Some("array") => {
            if let Some(items) = obj.get("items") {
                out.insert("items".to_string(), strictify(items, depth + 1)?);
            }
        }
        _ => {}
    }
    Some(Value::Object(out))
}
