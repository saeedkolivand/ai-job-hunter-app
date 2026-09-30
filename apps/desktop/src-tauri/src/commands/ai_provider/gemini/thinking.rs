//! Gemini model-generation classification (the v3+ boundary that governs
//! `thinkingLevel` vs `thinkingBudget`, and the deprecated sampling-param
//! omission it also gates) plus the shared temperature-resolution helper.
//! Split out of `gemini.rs` (R8 LOC cap) — a pure move.

/// Whether to request `thinkingConfig.includeThoughts`. Gemini 1.5 and the GA
/// 2.0 (non-thinking) models reject `thinkingConfig` with a 400, so this
/// enables it for Gemini 3+ ([`gemini_is_v3_or_later`] — the SAME v3+
/// boundary the effort feature's `thinkingLevel` shape uses; a real Gemini 3
/// id like `gemini-3-pro-preview` matches neither `"2.5"` nor `"thinking"`
/// on its own), the 2.5 family, and any explicit `*-thinking-*` model.
/// Unknown pre-3 future models simply don't surface thoughts (a graceful
/// miss, never a broken request).
pub(super) fn gemini_supports_thinking(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    gemini_is_v3_or_later(model) || m.contains("2.5") || m.contains("thinking")
}

/// Whether `model` is Gemini 3 or later — the boundary where the newer
/// `thinkingConfig.thinkingLevel` enum (`MINIMAL`/`LOW`/`MEDIUM`/`HIGH`) takes
/// over from the older `thinkingConfig.thinkingBudget` integer. Verified
/// against the live REST reference (`ai.google.dev/api/generate-content`,
/// fetched 2026-08-03): "`thinkingLevel` ... Recommended for Gemini 3 or
/// later models. Use with earlier models results in an error." Parses the
/// major version number right after the `gemini-` prefix (`gemini-3-pro-
/// preview`, `gemini-3.5-flash`, `gemini-3.6-flash`, …) so a future Gemini
/// 4/5/… release is recognized with no code change, unlike a growing
/// enumerated list.
pub(super) fn gemini_is_v3_or_later(model: &str) -> bool {
    let m = model
        .strip_prefix("models/")
        .unwrap_or(model)
        .to_ascii_lowercase();
    let Some(rest) = m.strip_prefix("gemini-") else {
        return false;
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().is_ok_and(|major| major >= 3)
}

/// Whether `model` is anything OTHER than [`gemini_is_v3_or_later`] — the
/// exact same boundary, inverted. Used by `GeminiClient::sampling_profile`
/// to decide whether its per-intent temperature defaults apply, mirroring
/// [`gemini_effective_temperature`]/[`gemini_omits_sampling_params`] and
/// every other gate in this file, which stay on [`gemini_is_v3_or_later`]
/// unchanged.
///
/// A PREVIOUS version of this function additionally required the literal
/// `gemini-` family prefix, treating any id it could not POSITIVELY classify
/// (a `gemma-*`/`learnlm-*`/other id — `parse_model_page` filters the
/// `/v1beta/models` listing only on the `models/` wrapper prefix, not on
/// family, so these DO reach this app's model picker) as unrecognized →
/// fully neutral, no temperature at all. That was wrong for this call site
/// and was dropped: unlike OpenAI's `OpenAiCompatible` gateways, this
/// provider has no other-vendor concept — every id reachable here is a
/// Google model served by Google's own endpoint, so there is nothing to
/// guess about. The unknown-model fail-safe this was copying from
/// `anthropic_supports_temperature` exists to avoid GUESSING an unknown
/// model's preferred *creative* sampling; `Intent::Deterministic` encodes an
/// APP requirement (the analyze surface's strict-JSON contract) rather than
/// a model preference, so withholding it is never correct — see
/// `openai_sampling_profile`'s identical fix for `OpenAiCompatible`
/// gateways (commit 89435a47) for the same mistake made once already. Do
/// not reintroduce a `starts_with("gemini-")` (or any other family-prefix)
/// requirement here — this is the SECOND time an "unclassifiable → neutral"
/// gate has cost a deterministic, strict-JSON surface its temperature.
pub(super) fn gemini_is_pre_v3(model: &str) -> bool {
    !gemini_is_v3_or_later(model)
}

/// Whether Gemini's deprecated sampling knobs — `temperature`, `topP`, and
/// `topK` (never wired up in this file — `AiGenerateRequest` has no `top_k`
/// field, so there is nothing to gate for it) — should be withheld for
/// `model`. ONE predicate decides all of them so a future model can't end up
/// gated for one and not another: that exact drift is how `topP` shipped
/// ungated in the first place while `temperature` already had this check.
///
/// Verified against two live Google references (fetched 2026-08-05), which
/// group all three identically rather than singling out `temperature`:
/// - `ai.google.dev/gemini-api/docs/whats-new-gemini-3.5`, "Parameter updates
///   and best practices in Gemini 3.x" — explicitly scoped to "all Gemini
///   3.x models", not just the newest releases: "`temperature`, `top_p`,
///   `top_k`: we strongly recommend not changing the default values."
/// - `ai.google.dev/gemini-api/docs/latest-model#sampling-parameter-deprecation`
///   (scoped to Gemini 3.6 Flash / 3.5 Flash-Lite "and all future Gemini
///   model releases"): "`temperature`, `top_p`, and `top_k` are deprecated
///   and ignored. In future model generations, supplying these parameters
///   returns an HTTP 400 error."
///
/// Reuses [`gemini_is_v3_or_later`] — the SAME boundary `thinkingLevel`
/// gates on — rather than an enumerated model list, so a future Gemini
/// 4/5/… release inherits the gate with no code change.
pub(super) fn gemini_omits_sampling_params(model: &str) -> bool {
    gemini_is_v3_or_later(model)
}

/// Effective `temperature` to send, or `None` to omit the field entirely.
/// `explicit` (the user's own choice, if any) ALWAYS wins, on every model —
/// this never overrides a deliberate value; only the app's OWN hardcoded
/// `fallback` default is gated (see [`gemini_omits_sampling_params`]).
/// Absent an explicit value, every call site in this file used to fall back
/// to its own hardcoded default (`0.7` for chat/complete, `0.2` for
/// research) regardless of model. Google's live docs
/// (`ai.google.dev/gemini-api/docs/gemini-3`, fetched 2026-08-04): "For all
/// Gemini 3 models, we strongly recommend keeping the temperature parameter
/// at its default value of `1.0`... Changing the temperature (setting it
/// below 1.0) may lead to unexpected behavior, such as looping or degraded
/// performance, particularly in complex mathematical or reasoning tasks."
/// Injecting either hardcoded default put every Gemini 3+ call (chat,
/// complete, AND research/synthesis, which is exactly the "complex
/// reasoning task" case) into the documented degradation case, including
/// every new user via the onboarding default (`gemini-3.6-flash`, itself
/// Gemini 3+). So on a gated model with no explicit value, this omits the
/// field so the API applies its OWN 1.0 default, instead of `fallback`; an
/// ungated model keeps `fallback` unchanged.
pub(super) fn gemini_effective_temperature(
    model: &str,
    explicit: Option<f64>,
    fallback: f64,
) -> Option<f64> {
    explicit.or_else(|| (!gemini_omits_sampling_params(model)).then_some(fallback))
}
