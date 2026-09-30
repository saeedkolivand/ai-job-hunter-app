//! Gemini `thinkingLevel` effort table plus the [`ModelCapabilities`]/
//! [`SamplingProfile`] the `AiProvider` trait methods report. Split out of
//! `gemini.rs` (R8 LOC cap) — a pure move.

use super::super::{
    Intent, ModelCapabilities, SamplingProfile, TokenParam, DETERMINISTIC_TEMPERATURE,
    PROSE_FREQUENCY_PENALTY, PROSE_GROUNDED_TEMPERATURE, PROSE_PRESENCE_PENALTY, PROSE_TEMPERATURE,
    PROSE_TOP_P,
};
use super::thinking::{gemini_is_pre_v3, gemini_is_v3_or_later};

/// Reasoning-effort levels Gemini 3.x accepts, PER MODEL — Google's live
/// table (`ai.google.dev/gemini-api/docs/thinking`, fetched 2026-08-04)
/// shows the accepted subset genuinely varies by model TIER, not just by
/// version (`gemini-3.1-flash-lite-image` supports only `minimal`/`high`;
/// `gemini-3.1-pro-preview` supports `low`/`medium`/`high`) — unlike every
/// other predicate in this crate, there is no clean shape rule to derive
/// this from the model id, so this is a genuine per-model lookup sourced
/// directly from that table:
///
/// | model                       | levels                          |
/// |------------------------------|---------------------------------|
/// | gemini-3.1-pro-preview         | low, medium, high                |
/// | gemini-3.1-flash-lite-image    | minimal, high                    |
/// | gemini-3-flash-preview, gemini-3.5-flash(-lite), gemini-3.6-flash | minimal, low, medium, high |
/// | gemini-3-pro-preview (SHUT DOWN — `ai.google.dev/gemini-api/docs/models`, checked 2026-08-04) | low, high |
///
/// `gemini-3.1-flash-lite` — the TEXT model (Stable, live), distinct from
/// `gemini-3.1-flash-lite-image` above — is deliberately absent from this
/// table, not an oversight: re-checked against the live thinking table
/// (`ai.google.dev/gemini-api/docs/thinking`, checked 2026-08-04) and it has
/// no row there at all, unlike every model listed above. Guessing it shares
/// its `-image` sibling's (or `gemini-3.5-flash-lite`'s) level set would be
/// exactly the "guessed value that could 400" this function exists to avoid
/// — it correctly falls through to the safe universal `["high"]` default
/// below until Google's table documents it.
///
/// Level acceptance is enforced POST-auth (proto/shape validation accepts
/// any `ThinkingLevel` enum member on every model — a request 400s only
/// later, on the model-specific check), so this table could not be probed
/// live without a key; treat Google's docs table as authoritative.
///
/// `gemini-3-pro-preview`'s row is kept even though the model itself is
/// shut down: a user with an already-saved config (or who types a model id
/// manually — the field is free text) still gets its real historical
/// levels instead of the generic `["high"]` fallback below, and a genuinely
/// wrong 400 is strictly worse than an accurate answer for a dead model
/// either way (the actual `embedContent`/`generateContent` call still fails
/// the SAME way regardless of what this function returns). Dropping the row
/// would be equally defensible — re-litigate if it becomes confusing rather
/// than helpful.
///
/// A model that passes [`gemini_is_v3_or_later`] but is NOT one of the rows
/// above is a genuinely new/unreleased id — falls back to `["high"]`, the
/// one level every row in the current table accepts (never a guessed value
/// that could 400; `effort: high` is also the documented no-op-equivalent
/// default on every provider in this crate, so it degrades gracefully as
/// "no override"). Pre-3 models (including the 2.5 family) get no levels at
/// all.
pub(super) fn gemini_effort_levels(model: &str) -> Vec<&'static str> {
    if !gemini_is_v3_or_later(model) {
        return Vec::new();
    }
    let m = model
        .strip_prefix("models/")
        .unwrap_or(model)
        .to_ascii_lowercase();
    if m.contains("gemini-3.1-flash-lite-image") {
        vec!["minimal", "high"]
    } else if m.contains("gemini-3.1-pro-preview") {
        vec!["low", "medium", "high"]
    } else if m.contains("gemini-3-pro-preview") {
        // SHUT DOWN as of 2026-08-04 (`ai.google.dev/gemini-api/docs/models`)
        // — kept for a saved/manually-typed id, see the doc comment above.
        vec!["low", "high"]
    } else if m.contains("gemini-3-flash-preview")
        || m.contains("gemini-3.5-flash")
        || m.contains("gemini-3.6-flash")
    {
        vec!["minimal", "low", "medium", "high"]
    } else {
        // Includes `gemini-3.1-flash-lite` (the text model, NOT `-image`) —
        // absent from the live thinking table as of the date above, so this
        // safe universal fallback is the correct answer, not a gap. See the
        // doc comment above before "fixing" this by guessing its levels.
        vec!["high"]
    }
}

/// The [`ModelCapabilities`] Gemini reports for `model` — moved out of the
/// `AiProvider::capabilities` trait method body so that method stays a thin
/// delegator.
pub(super) fn gemini_capabilities(model: &str) -> ModelCapabilities {
    ModelCapabilities {
        // Verified, not assumed: stays `true` unconditionally, including
        // for Gemini 3+. This field's established meaning across every
        // provider in this crate (see `openai.rs`/`anthropic.rs`'s own
        // `supports_temperature` gates) is "does the API REJECT this
        // field" — OpenAI's o-series 400s on it, Anthropic's adaptive-
        // thinking models 400 on it, but Gemini 3+ does not: Google's own
        // docs (`ai.google.dev/gemini-api/docs/gemini-3`) only recommend
        // AGAINST changing it from 1.0 for quality reasons, never say the
        // field itself is rejected. That quality concern is handled at
        // the send site instead (`gemini_effective_temperature`), which
        // omits an INVENTED default rather than a user's real choice —
        // this flag isn't consulted there today (unlike OpenAI/
        // Anthropic's `caps.supports_temperature` gate) and has no
        // renderer consumer either, so changing its value here wouldn't
        // move anything user-visible; it would just make it inaccurate.
        supports_temperature: true,
        supports_system_role: true, // mapped to systemInstruction
        supports_streaming: true,
        supports_reasoning: gemini_is_v3_or_later(model),
        supports_tools: true,
        supports_json_mode: true,
        supports_embeddings: true,
        // Native Google Search grounding tool (account-key gated at call time).
        supports_web_search: true,
        token_param: TokenParam::MaxOutputTokens,
    }
}

/// The [`SamplingProfile`] Gemini reports for `model`/`intent` — moved out of
/// the `AiProvider::sampling_profile` trait method body for the same reason
/// as [`gemini_capabilities`].
///
/// Neutral on Gemini 3+ (Google: "Remove these parameters from all
/// requests" — see [`super::thinking::gemini_omits_sampling_params`]); every
/// OTHER model on this provider declares real per-intent values reproducing
/// this app's pre-fix shipped numbers — the same [`DETERMINISTIC_TEMPERATURE`]/
/// [`PROSE_TEMPERATURE`]/[`PROSE_GROUNDED_TEMPERATURE`] + penalty
/// constants every other accepting adapter uses (this app's pre-fix
/// renderer sent the identical numbers to Gemini as every other cloud
/// provider). Gated on [`gemini_is_pre_v3`] — the exact inverse of
/// [`gemini_is_v3_or_later`], NOT additionally restricted to ids that
/// carry the literal `gemini-` prefix (see that function's own doc for
/// why a prefix requirement was tried here and dropped).
/// `gemini_effective_temperature` itself is UNCHANGED — still gated on
/// the wider `gemini_omits_sampling_params` for its own callers (e.g.
/// `complete_impl`), independent of this method; the two gates are now
/// the same v3 boundary, just phrased from opposite sides.
pub(super) fn gemini_sampling_profile(model: &str, intent: Intent) -> SamplingProfile {
    if !gemini_is_pre_v3(model) {
        return SamplingProfile::default();
    }
    match intent {
        // `Default` (no declared intent) resolves the same as
        // `Deterministic` — see `Intent`'s own doc comment
        // (`commands/ai_provider/mod.rs`).
        Intent::Deterministic | Intent::Default => SamplingProfile {
            temperature: Some(DETERMINISTIC_TEMPERATURE),
            ..SamplingProfile::default()
        },
        Intent::Prose => SamplingProfile {
            temperature: Some(PROSE_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            frequency_penalty: Some(PROSE_FREQUENCY_PENALTY),
            presence_penalty: Some(PROSE_PRESENCE_PENALTY),
            ..SamplingProfile::default()
        },
        // Same register as `Prose`, MINUS presence_penalty (see
        // `Intent::ProseGrounded`'s own doc comment for why).
        Intent::ProseGrounded => SamplingProfile {
            temperature: Some(PROSE_GROUNDED_TEMPERATURE),
            top_p: Some(PROSE_TOP_P),
            frequency_penalty: Some(PROSE_FREQUENCY_PENALTY),
            ..SamplingProfile::default()
        },
    }
}
