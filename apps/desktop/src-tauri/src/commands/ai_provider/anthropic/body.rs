//! Anthropic `/messages` request-body builders — one per call shape
//! (streaming chat, plain completion, structured, web-search, tool-calling).
//! Split out of `anthropic.rs` (R8 LOC cap) — a pure move.

use serde_json::{json, Value};

use super::super::{AiGenerateRequest, SamplingProfile};
use super::capabilities::{anthropic_effort_levels, anthropic_supports_temperature};
use super::thinking::{
    adaptive_max_tokens, anthropic_supports_thinking, anthropic_uses_adaptive_thinking,
    classic_thinking_engages, contains_version_needle, normalize_model_id,
};

/// Build the `/messages` streaming request body for a given
/// [`AiGenerateRequest`] + resolved [`SamplingProfile`] (mirrors
/// `openai.rs`'s `build_chat_stream_body`'s `caps.supports_temperature` gate,
/// via [`anthropic_supports_temperature`]). Pure + unit-tested. `sampling` is
/// already merged with the request's explicit numeric overrides (see
/// [`SamplingProfile::resolve`]) — `top_p` is Anthropic's only sampling knob
/// beyond temperature (no frequency/presence penalty in this API), each set
/// only when `Some` AND only when [`anthropic_supports_temperature`] (false
/// for every adaptive-thinking model, and for an unrecognized
/// `claude-`-prefixed id — see its doc comment): those models reject *any*
/// non-default `temperature`/`top_p`/`top_k` on every request per Anthropic's
/// docs ("Sampling parameters"), and we don't know each model's own default
/// value, so omitting the field entirely is the only universally-safe
/// choice — `AnthropicClient::sampling_profile` already returns a neutral
/// profile for those models too, so this is belt-and-suspenders, not the
/// only gate. Classic extended thinking ALSO omits `temperature` regardless
/// of `sampling` (Anthropic forces it to 1.0 internally; omitting IS that
/// default, and is one fewer place to get the number wrong) and never gets
/// `top_p` (400s alongside `thinking`) — this is a PER-REQUEST condition
/// (whether `max_tokens` is large enough to trigger classic thinking), not a
/// per-model one, so it can't live in `sampling_profile` itself.
pub(super) fn build_chat_stream_body(req: &AiGenerateRequest, sampling: SamplingProfile) -> Value {
    let max_tokens = req.max_tokens.unwrap_or(4096);

    let system_content: String = req
        .messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let messages: Vec<Value> = req
        .messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();

    // Thinking-token budget headroom: on BOTH classic and adaptive models
    // thinking tokens are billed as output tokens against `max_tokens`, so it
    // must be inflated to fit them AND the visible response.
    //
    // Classic thinking is gated on `classic_thinking_engages` (Anthropic's
    // "big enough task" heuristic, not a support check): under the gate no
    // `thinking` key is sent and nothing is inflated — always safe, since a
    // wrongful `thinking` block 400s the generation on classic-only models.
    // The gate is a named predicate because callers outside this module size
    // budgets around it (`extension_bridge::answer_assist`: its cap under it,
    // its one retry over — both asserted in this file's tests). Adaptive
    // thinking has no gate at all; [`adaptive_max_tokens`] owns that case and
    // gives the streaming and non-streaming builders one shared formula.
    let is_classic = anthropic_supports_thinking(&req.model);
    let is_adaptive = anthropic_uses_adaptive_thinking(&req.model);
    let classic_thinking_budget = if is_classic && classic_thinking_engages(max_tokens) {
        max_tokens / 2
    } else {
        0
    };
    let actual_max_tokens = if is_adaptive {
        adaptive_max_tokens(&req.model, max_tokens)
    } else {
        max_tokens.saturating_add(classic_thinking_budget)
    };

    let mut body = json!({
        "model": req.model,
        "messages": messages,
        "max_tokens": actual_max_tokens,
        "stream": true,
    });
    // Adaptive checked FIRST: a future model id that (incorrectly) matches
    // both predicates must fail toward the safe adaptive shape, not toward
    // the classic shape that 400s on an adaptive-only model.
    if is_adaptive {
        // Opt into "summarized" display — it defaults to "omitted" (empty
        // thinking blocks) on every adaptive model, which would silently
        // blank the app's thinking view. `temperature`/`top_p` stay omitted
        // (see fn doc comment above; `anthropic_supports_temperature` is
        // false for every adaptive model).
        body["thinking"] = json!({ "type": "adaptive", "display": "summarized" });
    } else if is_classic && classic_thinking_budget > 0 {
        body["thinking"] = json!({ "type": "enabled", "budget_tokens": classic_thinking_budget });
    } else if anthropic_supports_temperature(&req.model) {
        if let Some(t) = sampling.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(top_p) = sampling.top_p {
            body["top_p"] = json!(top_p);
        }
    }
    if !system_content.is_empty() {
        body["system"] = json!(system_content);
    }
    // `output_config.effort` is orthogonal to `thinking` (works with or
    // without it, per Anthropic's docs) — only ever sent when it's one of
    // the levels THIS model's tier actually accepts (`anthropic_effort_levels`),
    // not just when the model supports effort at all: `effort` is stored PER
    // PROVIDER (`preferences-store.ts`), not per model, so a saved `xhigh`
    // from Sonnet 5 must not survive a switch to Sonnet 4.6 (no `xhigh`) or
    // Opus 4.5 (no `xhigh`/`max`) — same class of bug as Gemini's
    // `thinkingLevel` gate in `gemini.rs`.
    if let Some(effort) = req
        .effort
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
    {
        if anthropic_effort_levels(&req.model).contains(&effort) {
            body["output_config"] = json!({ "effort": effort });
        }
    }
    body
}

/// Build the non-streaming `/messages` body shared by `complete`/
/// `complete_with_usage`. Pure + unit-tested — mirrors
/// [`build_chat_stream_body`]'s [`anthropic_supports_temperature`] gate but
/// never sends a `thinking` key at all (no thinking-view display concern on
/// this single-shot completion path).
pub(super) fn build_complete_body(
    model: &str,
    system: &str,
    user: &str,
    temperature: Option<f64>,
) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": adaptive_max_tokens(model, 4096),
        "messages": [ { "role": "user", "content": user } ],
    });
    if anthropic_supports_temperature(model) {
        body["temperature"] = json!(temperature.unwrap_or(0.7));
    }
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    body
}

/// [`build_complete_body`] plus an optional `output_config` merge — the one
/// extra field the structured path adds. Pure + unit-tested without HTTP;
/// shared by `complete_impl` itself, so an absent `output_config` behaves
/// exactly like [`build_complete_body`] alone.
pub(super) fn build_structured_body(
    model: &str,
    system: &str,
    user: &str,
    temperature: Option<f64>,
    output_config: Option<Value>,
) -> Value {
    let mut body = build_complete_body(model, system, user, temperature);
    if let Some(oc) = output_config {
        body["output_config"] = oc;
    }
    body
}

/// Build the non-streaming `/messages` body shared by every `research*`
/// facet (native `web_search` tool). Pure + unit-tested — same
/// [`anthropic_supports_temperature`] gate as [`build_complete_body`]; the
/// hardcoded `0.2` (favor precision over creativity for a research brief) is
/// simply skipped instead of overridden on adaptive models. Tool version per
/// model: `web_search_20260209` where documented, else `web_search_20250305`.
pub(super) fn build_web_search_body(model: &str, system: &str, user: &str) -> Value {
    let m = normalize_model_id(model);
    let newer_tool = [
        "opus-5",
        "opus-4-8",
        "opus-4-7",
        "opus-4-6",
        "sonnet-5",
        "sonnet-4-6",
    ]
    .iter()
    .any(|needle| contains_version_needle(&m, needle));
    let tool_type = if newer_tool {
        "web_search_20260209"
    } else {
        "web_search_20250305"
    };
    let mut body = json!({
        "model": model,
        "max_tokens": adaptive_max_tokens(model, 1024),
        "system": system,
        "messages": [{ "role": "user", "content": user }],
        "tools": [{ "type": tool_type, "name": "web_search", "max_uses": 3 }],
    });
    if anthropic_supports_temperature(model) {
        body["temperature"] = json!(0.2);
    }
    body
}

/// Build the non-streaming `/messages` body for `AnthropicClient::chat_with_tools`.
/// Pure + unit-tested — same [`anthropic_supports_temperature`] gate as
/// [`build_complete_body`].
pub(super) fn build_tools_body(
    model: &str,
    system: &str,
    wire_messages: Vec<Value>,
    tool_specs: Vec<Value>,
    temperature: Option<f64>,
) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": adaptive_max_tokens(model, 4096),
        "messages": wire_messages,
        "tools": tool_specs,
    });
    if anthropic_supports_temperature(model) {
        body["temperature"] = json!(temperature.unwrap_or(0.7));
    }
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    body
}
