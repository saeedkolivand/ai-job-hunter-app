//! The shared half of [`AiProvider::complete_structured`](super::AiProvider::complete_structured):
//! the prompt-discipline fallback every provider gets for free, plus the
//! per-provider translators for a provider that has a native
//! constrained-decoding field (split into `structured/openai.rs`,
//! `structured/anthropic.rs`, `structured/gemini.rs` — R8 line-budget split;
//! each is a pure `Value -> Value` translator, so each adapter still owns
//! its own transport, only the shape mapping is shared).
//!
//! Why the per-provider translators live together rather than in each adapter:
//! they all translate the SAME caller-supplied flat JSON Schema into a
//! different vendor dialect, so a hardening applied to one and silently not
//! the others is exactly the drift this codebase keeps re-discovering (see
//! [`super::pagination`]'s module doc for the same argument).
//!
//! **The prompt-discipline path is the PERMANENT fallback**, not a stopgap: a
//! provider with no native JSON mode, an unknown gateway, a CLI agent, or a
//! caller that has an example but no schema all land here and must keep
//! working. No caller may require native constrained decoding.
//!
//! The return leg lives here too: [`JsonParseError::reask_detail`] is the one
//! sanctioned way to quote a rejected response back to the model, because the
//! fence primitive it needs ([`fenced`]) cannot be imported by
//! [`crate::pipeline::json`] itself (see that method's doc).

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::error::AppResult;
use crate::pipeline::json::JsonParseError;
use crate::prompt_fence::fenced;

use super::{
    flatten_messages, resolve_intent, AiGenerateRequest, AiProvider, ChatMsg, Role, Usage,
};

mod anthropic;
mod gemini;
mod openai;

pub(super) use anthropic::anthropic_output_config;
pub(super) use gemini::gemini_response_schema;
pub(super) use openai::openai_response_format;

/// The trusted instruction appended to the SYSTEM slot (never the user slot —
/// untrusted résumé/job-ad text rides there, and mixing an instruction into it
/// is the OWASP LLM01 mistake this codebase segregates against everywhere
/// else). Deliberately mentions "JSON" verbatim: OpenAI's `json_object`
/// response format REJECTS a request whose messages never say the word.
const JSON_ONLY_DIRECTIVE: &str = "Output contract: reply with ONE valid JSON value and nothing \
else — no prose, no preamble, no explanation, no Markdown code fence. When you have no value for \
a key, still emit it with an empty/neutral value of the right type.";

/// The half of the output contract that only makes sense once an example is
/// actually appended — so it is appended WITH the example, never on its own.
/// A schema-less caller (hint `""`, a supported and test-pinned path) used to
/// be told to copy the keys "shown in the example below" and to "not add keys"
/// with no example anywhere in the prompt: an instruction to conform to
/// something that isn't there is at best noise and at worst invites the model
/// to invent the missing example.
const JSON_EXAMPLE_DIRECTIVE: &str = "Use exactly the keys, nesting and value types shown in the \
example below; the example's VALUES are placeholders and must never be copied. Do not add keys.";

/// The transcript [`Role`] a request message's wire string names — the
/// structured path's only role decision, so it is made once, here.
///
/// Case-insensitive: an exact-lowercase `== "system"` demotes a `"System"`
/// message into the UNTRUSTED user slot, which is the worst direction for a
/// casing difference to fail in. Anything unrecognized maps to `User` for the
/// same reason (fail toward "untrusted", never toward "instruction").
fn role_of(wire: &str) -> Role {
    let wire = wire.trim();
    if wire.eq_ignore_ascii_case("system") {
        Role::System
    } else if wire.eq_ignore_ascii_case("assistant") {
        Role::Assistant
    } else if wire.eq_ignore_ascii_case("tool") {
        Role::Tool
    } else {
        Role::User
    }
}

/// Build the `(system, user)` pair for a structured completion: the request's
/// system messages, then [`JSON_ONLY_DIRECTIVE`], then the filled-example
/// `schema_hint`; every non-system message flattened into the user slot.
///
/// The split itself is [`flatten_messages`] — the SAME flattener every other
/// single-slot path uses — rather than a local join, so a prior assistant or
/// tool turn keeps its `Assistant: ` / `Tool result: ` provenance marker here
/// too. Concatenating them raw made untrusted model output (and, on the tool
/// path, text that came off a job board) byte-indistinguishable from what the
/// user actually typed, inside the very slot this module segregates for
/// exactly that reason (OWASP LLM01).
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
    let messages: Vec<ChatMsg> = req
        .messages
        .iter()
        .map(|m| ChatMsg {
            role: role_of(&m.role),
            content: m.content.clone(),
        })
        .collect();
    let (mut system, user) = flatten_messages(&messages);
    if !system.is_empty() {
        system.push_str("\n\n");
    }
    system.push_str(JSON_ONLY_DIRECTIVE);
    let hint = schema_hint.trim();
    if !hint.is_empty() {
        system.push(' ');
        system.push_str(JSON_EXAMPLE_DIRECTIVE);
        system.push_str("\n\nExample of the required shape:\n");
        system.push_str(hint);
    }
    (system, user)
}

/// The fence tag wrapping a rejected response's parser detail on its way into
/// a re-ask prompt (see [`JsonParseError::reask_detail`]).
const REASK_DETAIL_TAG: &str = "invalid_json_detail";

/// Char cap for that fenced detail. serde quotes the offending fragment, and
/// that fragment is the MODEL's own output — a single string value can be the
/// whole response — so the re-ask needs a bound of its own; a serde message
/// that says anything useful is far shorter than this.
const REASK_DETAIL_CAP: usize = 1_000;

impl JsonParseError {
    /// This failure's parser detail, wrapped as untrusted DATA and ready to
    /// paste into a re-ask prompt — `""` for the variants that carry none
    /// ([`NotFound`](JsonParseError::NotFound) /
    /// [`Truncated`](JsonParseError::Truncated): there is nothing to quote,
    /// and the fix there is a different request, not a correction).
    ///
    /// **This, not [`detail`](JsonParseError::detail), is what a re-ask
    /// builds from.** The detail quotes a fragment of the model's own
    /// (attacker-influenceable) output, so pasting it raw would smuggle it
    /// into the trusted half of the next prompt — the LLM01 mistake the
    /// structured path segregates against. [`fenced`] is the crate's ONE
    /// boundary mechanism (ADR-010): it caps the fragment and neutralizes
    /// every fence tag and `[tool_result` marker inside it, including a forged
    /// copy of this block's own closing tag.
    ///
    /// It lives in this module rather than next to the error type for
    /// historical reasons: [`fenced`] used to be an L3 primitive (`agent::tools`)
    /// and `pipeline::json` is L1/L2, so architecture rule R7 forbade that
    /// import. [`fenced`] has since relocated to the dependency-free L0
    /// [`crate::prompt_fence`] (PR-5 step 1) and `agent` itself is gone
    /// (PR-5 step 2), so the R7 blocker is clear — this could become an
    /// ordinary method on [`JsonParseError`] importing `prompt_fence`
    /// directly. Left as-is for now (a separate move, not bundled into the
    /// agent deletion).
    pub fn reask_detail(&self) -> String {
        match self.detail() {
            "" => String::new(),
            detail => fenced(REASK_DETAIL_TAG, detail, REASK_DETAIL_CAP),
        }
    }
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

// ── Shared schema-walk infrastructure (used by every per-provider translator) ──

/// How deep the per-provider translators will walk a caller's schema before
/// failing the whole translation. Schemas are developer-authored today and
/// nothing legitimate comes close (OpenAI's own strict mode rejects past 5
/// levels of nesting); the cap is the guard for the day one becomes config-
/// or model-supplied, where a pathological — or accidentally cyclic, once
/// `$ref` resolution exists — schema would otherwise recurse until the stack
/// blows. Every caller already handles "no usable schema": OpenAI degrades to
/// `json_object`, Gemini to `responseMimeType` + the prompt hint.
const MAX_SCHEMA_DEPTH: usize = 16;

/// JSON Schema keywords that COMPOSE or REFERENCE other schemas. No
/// per-provider translator walks into them — each recurses through
/// `properties`/`items` only — so a schema carrying one anywhere fails the
/// whole translation ([`has_untranslatable_keyword`]) instead of being
/// half-translated.
///
/// The alternative is worse on BOTH sides, in two opposite ways the
/// translators already refuse elsewhere:
///
/// - OpenAI: `strictify` would stamp `strict: true` over a subtree it never
///   strictified — an `anyOf` branch keeps its own `properties` with no
///   `required`/`additionalProperties`, and OpenAI 400s the entire request.
///   That was the one schema path with no degrade at all (a non-object root and
///   the depth cap both already fall back to `json_object`).
/// - Gemini: the keyword would simply be dropped (it is not in
///   `GEMINI_KEPT_KEYWORDS`), silently sending a WEAKER constraint than the
///   caller asked for — the same silent-weakening `gemini_response_schema`
///   already rejects for an untranslatable `type`.
///
/// Both callers degrade whole: OpenAI to `json_object`, Gemini to
/// `responseMimeType` + the prompt hint. The prompt still carries the directive
/// and the filled example either way, so the shape is still asked for — just
/// not constrained by the decoder.
const COMPOSITION_KEYWORDS: &[&str] = &["anyOf", "oneOf", "allOf", "not", "$ref", "$defs"];

/// Whether `schema` carries a [`COMPOSITION_KEYWORDS`] entry at ANY depth —
/// including under keys neither walker visits (`additionalProperties`,
/// `patternProperties`, an `anyOf` branch's own `properties`, …), which is why
/// this scans the raw [`Value`] tree rather than riding along with a walker.
///
/// "Too deep to scan" counts as untranslatable: a `Value` is acyclic, so this
/// only fires on a genuinely pathological schema, and the answer there is the
/// same degrade the walkers' own [`MAX_SCHEMA_DEPTH`] already produces.
fn has_untranslatable_keyword(schema: &Value, depth: usize) -> bool {
    if depth > MAX_SCHEMA_DEPTH {
        return true;
    }
    match schema {
        Value::Object(map) => {
            map.keys()
                .any(|key| COMPOSITION_KEYWORDS.contains(&key.as_str()))
                || map
                    .values()
                    .any(|value| has_untranslatable_keyword(value, depth + 1))
        }
        Value::Array(items) => items
            .iter()
            .any(|item| has_untranslatable_keyword(item, depth + 1)),
        _ => false,
    }
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
mod tests;
