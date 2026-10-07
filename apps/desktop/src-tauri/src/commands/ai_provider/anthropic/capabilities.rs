//! Anthropic capability gates: temperature/effort/structured-output support
//! per model, plus the [`ModelCapabilities`]/[`SamplingProfile`] the
//! `AiProvider` trait methods report. Split out of `anthropic.rs` (R8 LOC
//! cap) — a pure move.

use super::super::{
    Intent, ModelCapabilities, SamplingProfile, TokenParam, DETERMINISTIC_TEMPERATURE,
    PROSE_GROUNDED_TEMPERATURE, PROSE_TEMPERATURE, PROSE_TOP_P,
};
use super::thinking::{
    anthropic_is_legacy_pre_thinking, anthropic_supports_thinking,
    anthropic_uses_adaptive_thinking, contains_version_needle, normalize_model_id,
};

/// Whether Anthropic will accept a non-default `temperature`/`top_p` on
/// `model` at all — the single source of truth for both
/// [`anthropic_capabilities`]'s `supports_temperature` field AND every
/// request builder in `super::body` (computed once here rather than
/// re-derived independently in two places that could drift out of sync).
///
/// `false` for every adaptive-thinking model ([`anthropic_uses_adaptive_thinking`]
/// — 400s on ANY non-default value, on every request). Also `false`, as a
/// **fail-safe**, for a `claude-`-prefixed id that matches NEITHER thinking
/// classification AND isn't a known [`anthropic_is_legacy_pre_thinking`]
/// model: that combination means a genuinely new Anthropic family this
/// adapter hasn't learned a needle for yet — since sending a non-default
/// temperature 400s on an adaptive model but omitting it is ALWAYS accepted,
/// defaulting an unclassified NEW Claude id to "no temperature" is the
/// direction that can never 400, which is what restores the zero-code-change
/// promise for a new model family. A legacy pre-thinking id (definitely safe
/// — proven by years of unchanged behavior) and a non-`claude-` id (never a
/// real Anthropic model id) both keep the old always-on behavior.
pub(super) fn anthropic_supports_temperature(model: &str) -> bool {
    if anthropic_uses_adaptive_thinking(model) {
        return false;
    }
    if !anthropic_supports_thinking(model)
        && normalize_model_id(model).starts_with("claude-")
        && !anthropic_is_legacy_pre_thinking(model)
    {
        return false;
    }
    true
}

/// Whether `model` accepts the `output_config.effort` parameter — verified
/// against Anthropic's live docs
/// (`platform.claude.com/docs/en/build-with-claude/effort`, fetched
/// 2026-08-03): "The effort parameter is supported by Claude Fable 5, Claude
/// Mythos 5, Claude Opus 5, Claude Opus 4.8, Claude Mythos Preview, Claude
/// Opus 4.7, Claude Opus 4.6, Claude Sonnet 5, Claude Sonnet 4.6, and Claude
/// Opus 4.5." This is a DIFFERENT (larger) set than
/// [`anthropic_uses_adaptive_thinking`] — that same page: "On Claude Opus
/// 4.5, the only extended-thinking-only model that supports effort" — and
/// Opus/Sonnet 4.6 support effort without being adaptive-thinking models at
/// all, so effort support can't be derived from either existing thinking
/// predicate. A closed, explicitly-verified list (mirrors this file's other
/// version-needle gates) — an unrecognized future model defaults to `false`
/// (never a guessed value; `output_config.effort` 400s on a model that
/// doesn't support it). Every needle (including the two Mythos names below)
/// is boundary-checked via [`contains_version_needle`], as they are in every
/// gate in this file. What differs is BREADTH, not boundary handling: this is
/// a closed list of VERSIONED names, so only the two Mythos releases the
/// effort page currently documents ("Claude Mythos 5", "Claude Mythos
/// Preview") match — a `claude-mythos-6` does not. The broader
/// [`anthropic_uses_adaptive_thinking`] (a different predicate this one
/// deliberately does NOT reuse) gates the bare `mythos` FAMILY instead,
/// because guessing wrong is safe there and 400s here.
pub(super) fn anthropic_supports_effort(model: &str) -> bool {
    let m = normalize_model_id(model);
    contains_version_needle(&m, "opus-4-5")
        || contains_version_needle(&m, "opus-4-6")
        || contains_version_needle(&m, "opus-4-7")
        || contains_version_needle(&m, "opus-4-8")
        || contains_version_needle(&m, "sonnet-4-6")
        || contains_version_needle(&m, "sonnet-5")
        || contains_version_needle(&m, "opus-5")
        || contains_version_needle(&m, "fable-5")
        || contains_version_needle(&m, "mythos-5")
        || contains_version_needle(&m, "mythos-preview")
}

/// Effort LEVELS `model` actually accepts — same live page as
/// `anthropic_supports_effort` (`platform.claude.com/docs/en/build-with-claude/effort`,
/// fetched 2026-08-04). The accepted level SET genuinely varies by model
/// tier, exactly like Gemini's `thinkingLevel` (see `gemini_effort_levels` in
/// `gemini.rs`) — this is NOT the binary "supports effort or doesn't" gate
/// above; it's a second, finer-grained lookup:
///
/// | tier | models | levels |
/// |---|---|---|
/// | full (5) | Fable 5, Mythos 5, Opus 5, Opus 4.8, Opus 4.7, Sonnet 5 | low, medium, high, max, xhigh |
/// | no `xhigh` (4) | Mythos Preview, Opus 4.6, Sonnet 4.6 | low, medium, high, max |
/// | `low`/`medium`/`high` only (1) | Opus 4.5 | low, medium, high |
///
/// The page states this explicitly per level, not per model: `max` is
/// "Available on Claude Fable 5, Claude Mythos 5, Claude Opus 5, Claude Opus
/// 4.8, Claude Mythos Preview, Claude Opus 4.7, Claude Opus 4.6, Claude
/// Sonnet 5, and Claude Sonnet 4.6" — every `anthropic_supports_effort`
/// model EXCEPT Opus 4.5 (confirmed separately: "On Claude Opus 4.5, the
/// only extended-thinking-only model that supports effort..."). `xhigh` is
/// "Available on Claude Fable 5, Claude Mythos 5, Claude Opus 5, Claude Opus
/// 4.8, Claude Opus 4.7, and Claude Sonnet 5" — narrower again, dropping
/// Mythos Preview/Opus 4.6/Sonnet 4.6 too ("xhigh is a newer level; some
/// models that support max don't support xhigh"). `low`/`medium`/`high` are
/// universal across every effort-capable model (no per-model exclusion
/// documented for any of the three). Empty for a model
/// `anthropic_supports_effort` rejects outright.
pub(super) fn anthropic_effort_levels(model: &str) -> Vec<&'static str> {
    if !anthropic_supports_effort(model) {
        return Vec::new();
    }
    let m = normalize_model_id(model);
    if contains_version_needle(&m, "opus-4-5") {
        vec!["low", "medium", "high"]
    } else if contains_version_needle(&m, "mythos-preview")
        || contains_version_needle(&m, "opus-4-6")
        || contains_version_needle(&m, "sonnet-4-6")
    {
        vec!["low", "medium", "high", "max"]
    } else {
        vec!["low", "medium", "high", "max", "xhigh"]
    }
}

/// Whether `model`'s API offers native **structured outputs** (server-side
/// constrained decoding against a JSON Schema) at all — the Claude 4.5
/// generation and later, plus Opus 4.1. Claude 3.x and the 4.0 models never
/// got it. A closed, explicitly-listed set exactly like
/// [`anthropic_supports_effort`], boundary-checked via
/// [`contains_version_needle`]; deliberately NOT derived from any existing
/// thinking/effort predicate (those cover different, non-coinciding model
/// sets — see `anthropic_supports_effort`'s doc). An unrecognized or future
/// id defaults to **false**: the conservative direction, since the fallback
/// (prompt discipline in `AiProvider::complete_structured`'s default) always
/// works and a wrongly-claimed capability cannot.
///
/// Gates `AnthropicClient::complete_structured`'s native path
/// (`output_config.format`, GA — no beta header). A model outside this set,
/// or a schema `structured::anthropic_output_format` can't close, falls
/// back to prompt discipline, so `complete_structured` works on every model.
pub(super) fn anthropic_supports_structured_outputs(model: &str) -> bool {
    let m = normalize_model_id(model);
    // The 4.5 generation (the first with structured outputs) plus Opus 4.1.
    contains_version_needle(&m, "opus-4-1")
        || contains_version_needle(&m, "opus-4-5")
        || contains_version_needle(&m, "sonnet-4-5")
        || contains_version_needle(&m, "haiku-4-5")
        // Everything Anthropic shipped after it, per this adapter's known set.
        || contains_version_needle(&m, "opus-4-6")
        || contains_version_needle(&m, "opus-4-7")
        || contains_version_needle(&m, "opus-4-8")
        || contains_version_needle(&m, "sonnet-4-6")
        || contains_version_needle(&m, "haiku-4-6")
        || contains_version_needle(&m, "opus-5")
        || contains_version_needle(&m, "sonnet-5")
        || contains_version_needle(&m, "haiku-5")
        || contains_version_needle(&m, "fable-5")
        || contains_version_needle(&m, "mythos-5")
        || contains_version_needle(&m, "mythos-preview")
}

/// The `effort` `AnthropicClient::complete_structured` actually sends:
/// `raw` trimmed, checked non-empty, and kept only for a level THIS model's
/// tier accepts ([`anthropic_effort_levels`]) — `effort` is stored PER
/// PROVIDER, so a saved `xhigh` from Sonnet 5 must not survive a switch to a
/// model whose tier rejects it (same guard `super::body::build_chat_stream_body`
/// already applies to its own `effort` field). Pure + unit-tested.
pub(super) fn anthropic_structured_effort<'a>(
    model: &str,
    raw: Option<&'a str>,
) -> Option<&'a str> {
    raw.map(str::trim)
        .filter(|e| !e.is_empty())
        .filter(|e| anthropic_effort_levels(model).contains(e))
}

/// The `output_config` the plain-text `complete_with_effort` path sends: just
/// the gated effort (see [`anthropic_structured_effort`]), no `format`. `None`
/// when no effort is set or this model's tier rejects it, leaving the body
/// exactly as `build_complete_body` makes it (which never carries a
/// `thinking` block). Pure + unit-tested.
pub(super) fn anthropic_effort_output_config(
    model: &str,
    raw: Option<&str>,
) -> Option<serde_json::Value> {
    anthropic_structured_effort(model, raw).map(|effort| serde_json::json!({ "effort": effort }))
}

/// The [`ModelCapabilities`] Anthropic reports for `model` — moved out of the
/// `AiProvider::capabilities` trait method body so that method stays a thin
/// delegator (this file is the model-classification layer, per
/// [`anthropic_supports_temperature`]'s doc comment).
pub(super) fn anthropic_capabilities(model: &str) -> ModelCapabilities {
    // Every adaptive-thinking model 400s on ANY non-default temperature/
    // top_p/top_k on EVERY request, thinking or not (Anthropic's "Sampling
    // parameters" note), and an unrecognized `claude-`-prefixed id fails
    // safe to the same "no temperature" default — see
    // [`anthropic_supports_temperature`], the single source of truth this
    // field AND every request builder in `super::body` both gate on.
    if !anthropic_supports_thinking(model) && !anthropic_uses_adaptive_thinking(model) {
        // Debug-only observability, never user-facing, never blocks the
        // call: this is either a legacy non-thinking model (nothing wrong
        // — it simply predates thinking) or an unrecognized new Anthropic
        // family this adapter hasn't learned a needle for yet, in which
        // case its thinking view (if any) stays blank until it's added.
        tracing::debug!(
            model,
            "anthropic: no thinking classification (legacy non-thinking model or \
             unrecognized new family)"
        );
    }
    ModelCapabilities {
        supports_temperature: anthropic_supports_temperature(model),
        // Anthropic carries the system prompt as a top-level field, not a role.
        supports_system_role: false,
        supports_streaming: true,
        supports_reasoning: anthropic_supports_effort(model),
        supports_tools: true,
        // Corrected from a blanket `false`: the 4.5 generation and later
        // (plus Opus 4.1) DO have native structured outputs — see
        // `anthropic_supports_structured_outputs`, which also spells out
        // why this adapter's `complete_structured` still takes the trait
        // default and why no caller may read this flag as "this call will
        // be natively constrained".
        supports_json_mode: anthropic_supports_structured_outputs(model),
        supports_embeddings: false,
        // Native server-side web_search tool (account-key gated at call time).
        supports_web_search: true,
        token_param: TokenParam::MaxTokens,
    }
}

/// The [`SamplingProfile`] Anthropic reports for `model`/`intent` — moved out
/// of the `AiProvider::sampling_profile` trait method body for the same
/// reason as [`anthropic_capabilities`].
///
/// Neutral on every adaptive-thinking (Claude 4.7+/5) model AND on an
/// unrecognized `claude-`-prefixed id ([`anthropic_supports_temperature`]'s
/// own fail-safe) — `temperature`/`top_p`/`top_k` 400 there regardless of
/// value, and Anthropic has no frequency/presence/repeat penalty
/// parameters at ALL (so `Intent::ProseGrounded`'s presence-penalty
/// distinction from `Intent::Prose` collapses to "same as Prose" here —
/// there is nothing to withhold). A model
/// [`anthropic_supports_temperature`] accepts (legacy pre-thinking, or a
/// classic-thinking-capable model NOT currently thinking) declares real
/// values reproducing this app's pre-fix shipped numbers — this app's
/// pre-fix renderer sent the identical numbers to Anthropic as every other
/// cloud provider. `top_p` is Anthropic's only sampling knob beyond
/// temperature (no frequency/presence penalty in this API).
pub(super) fn anthropic_sampling_profile(model: &str, intent: Intent) -> SamplingProfile {
    if !anthropic_supports_temperature(model) {
        return SamplingProfile::default();
    }
    match intent {
        // `Default` (no declared intent) resolves the same as
        // `Deterministic` — see `Intent`'s own doc comment
        // (`commands/ai_provider/sampling.rs`).
        Intent::Deterministic | Intent::Default => SamplingProfile {
            temperature: Some(DETERMINISTIC_TEMPERATURE),
            ..SamplingProfile::default()
        },
        Intent::Prose => SamplingProfile {
            temperature: Some(PROSE_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            ..SamplingProfile::default()
        },
        Intent::ProseGrounded => SamplingProfile {
            temperature: Some(PROSE_GROUNDED_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            ..SamplingProfile::default()
        },
    }
}
