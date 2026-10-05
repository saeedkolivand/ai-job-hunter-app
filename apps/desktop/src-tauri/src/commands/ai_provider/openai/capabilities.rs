//! OpenAI/OpenAI-compatible model classification (o-series vs gpt-5.x
//! reasoning families) and the [`ModelCapabilities`]/[`SamplingProfile`] the
//! `AiProvider` trait methods report. Split out of `openai.rs` (R8 LOC cap)
//! — a pure move.

use super::super::{
    Intent, ModelCapabilities, ProviderId, SamplingProfile, TokenParam, DETERMINISTIC_TEMPERATURE,
    PROSE_FREQUENCY_PENALTY, PROSE_GROUNDED_TEMPERATURE, PROSE_PRESENCE_PENALTY, PROSE_TEMPERATURE,
    PROSE_TOP_P,
};
use super::{OpenAiClient, OPENAI_EFFORT_LEVELS};

/// OpenAI reasoning families (the `o`-series: o1, o3, o4, … and future `o`N)
/// reject `temperature` and require `max_completion_tokens` instead of
/// `max_tokens`. Matched by the `o`+digit convention so new o-series models are
/// handled without a code change.
///
/// This predicate is the `supports_temperature`/`token_param` gate ONLY — it
/// does NOT cover OpenAI's current gpt-5.x reasoning line (see
/// [`is_gpt5_or_later_reasoning_family`]), which accepts a normal
/// `temperature`/`max_tokens` unlike the o-series. Reusing this for the
/// `reasoning_effort` gate would silently exclude gpt-5.x — the two gates
/// answer different questions and must stay separate.
pub(super) fn is_reasoning_model(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    let mut bytes = m.bytes();
    matches!((bytes.next(), bytes.next()), (Some(b'o'), Some(d)) if d.is_ascii_digit())
}

/// OpenAI's CURRENT reasoning-model line — gpt-5 and later (verified against
/// the live reasoning guide, `platform.openai.com/docs/guides/reasoning`,
/// fetched 2026-08-04: "Start with `gpt-5.6` for most reasoning workloads");
/// `docs/models/gpt-5.6.md`-style model pages confirm `reasoning_effort`
/// support on `/v1/chat/completions`. Distinct from (and additive to)
/// [`is_reasoning_model`]'s legacy o-series gate — a REQUEST SCHEMA
/// (`CreateChatCompletionRequest`) never carries a model list, so this is
/// verified against the provider's model/capability docs, not the schema.
///
/// Matches any `gpt-`+digit-major≥5 id (`gpt-5`, `gpt-5-mini`, `gpt-5.4`,
/// `gpt-5.5`, `gpt-5.6` and its `-sol`/`-terra`/`-luna` aliases) so a NEW
/// gpt-5.x variant — or a later numbered major line, should OpenAI keep this
/// convention — is picked up with no code change, EXCEPT the `-chat-latest`
/// family (`gpt-5-chat-latest`, `gpt-5.1-chat-latest`, …): OpenAI's
/// non-reasoning conversational variant of each gpt-5.x generation (mirrors
/// the older `chatgpt-4o-latest` naming), confirmed in the live
/// `ModelIdsShared` enum — explicitly excluded.
pub(super) fn is_gpt5_or_later_reasoning_family(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    if m.contains("chat-latest") {
        return false;
    }
    let Some(rest) = m.strip_prefix("gpt-") else {
        return false;
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().is_ok_and(|major| major >= 5)
}

/// Non-reasoning profile shared by native OpenAI AND `OpenAiCompatible`
/// gateways (LM Studio/vLLM/OpenRouter/custom endpoints — same wire
/// protocol): real values reproducing this app's pre-fix shipped numbers
/// for every intent — see the shared constants' doc comments
/// (`commands/ai_provider/sampling.rs`) for the exact per-surface history each
/// one preserves. `frequency_penalty`/`presence_penalty` both sit inside
/// OpenAI's own documented "reasonable" band
/// (`platform.openai.com/docs/api-reference/chat/create`: "Number between
/// -2.0 and 2.0 ... reasonable values are between 0 and 1").
fn openai_sampling_profile(intent: Intent) -> SamplingProfile {
    match intent {
        // `Default` (no declared intent) resolves the same as `Deterministic`
        // — see `Intent`'s own doc comment (`commands/ai_provider/sampling.rs`).
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

/// Ollama Cloud's `/v1` layer hardcodes `temperature: 1.0, top_p: 1.0` when
/// the caller omits them — overriding the model's own Modelfile defaults —
/// so, unlike every OTHER OpenAI-compatible gateway (an unknown, arbitrary
/// catalog this app never assumes anything about), omitting is NEVER neutral
/// here for ANY family: every intent must declare real values, or the
/// consequence is a forced 1.0/1.0 regardless of what this app intended.
/// `gpt-oss` gets its own vendor-recommended values (source:
/// `github.com/openai/gpt-oss` README, "Recommended Sampling Parameters":
/// temperature 1.0, top_p 1.0 — which happen to coincide with `/v1`'s own
/// hardcoded default, so this is declared for auditability, not because
/// omission would behave differently for gpt-oss specifically). Every other
/// family reuses the SAME per-intent targets native OpenAI does — this app's
/// pre-fix renderer sent the identical numbers to every cloud provider,
/// Ollama Cloud included. `repeat_penalty`/`num_ctx` can't be reached at all
/// via `/v1` (Ollama-native-only fields), so `Prose`/`ProseGrounded` never
/// set them here even though local Ollama does. `Intent::Default` resolves
/// to `Intent::Deterministic`'s numbers (see `Intent`'s own doc comment) —
/// deliberately NOT left neutral here specifically, since neutral would mean
/// the forced 1.0/1.0 this whole function exists to avoid.
pub(super) fn ollama_cloud_sampling_profile(model: &str, intent: Intent) -> SamplingProfile {
    if model.to_ascii_lowercase().contains("gpt-oss") {
        return SamplingProfile {
            temperature: Some(1.0),
            top_p: Some(1.0),
            ..SamplingProfile::default()
        };
    }
    openai_sampling_profile(intent)
}

impl OpenAiClient {
    /// Whether this client's provider id exposes OpenAI's native `web_search`
    /// tool — only native OpenAI does; every OpenAI-compatible gateway can't be
    /// assumed to support it, and Ollama Cloud overrides `research()`/
    /// `research_salary()` on its own client. Factored to a pure, `AppHandle`-free
    /// predicate purely so the gate stays unit-testable (this crate has no
    /// `tauri::test` mock-app harness to drive `web_search_complete` itself end
    /// to end — see the same note on `salary_research::SalaryResearch::enrich`).
    pub(super) fn supports_web_search(&self) -> bool {
        self.id == ProviderId::OpenAi
    }

    /// Whether this client's provider id + model accepts the `reasoning_effort`
    /// field on `/chat/completions` (verified against the provider's live
    /// model/capability docs — a REQUEST SCHEMA like
    /// `CreateChatCompletionRequest` never carries a model list, so a gate
    /// like this one is checked against OpenAI's reasoning guide + model
    /// pages, not the schema — and Ollama's OpenAI-compatibility reference,
    /// fetched 2026-08-04). Native OpenAI: the legacy o-series
    /// ([`is_reasoning_model`]) OR the current gpt-5.x line
    /// ([`is_gpt5_or_later_reasoning_family`]) — two SEPARATE gates ORed
    /// together, not one reused, because gpt-5.x accepts a normal
    /// `temperature` unlike the o-series (see `is_reasoning_model`'s doc
    /// comment). Ollama Cloud: a DIFFERENT gate —
    /// [`super::super::ollama::ollama_family_supports_thinking`], the same
    /// thinking-model-family classifier local Ollama's native `think` field
    /// uses — its `/v1` endpoint is OpenAI-compatible but its model CATALOG is
    /// Ollama's own (e.g. `gpt-oss:120b` doesn't match the `o`+digit or
    /// `gpt-5`+ conventions). Every other OpenAI-compatible gateway (LM
    /// Studio, OpenRouter, generic `openai-compatible`) is an unknown catalog
    /// — never guessed, so a wrong value can't 400 a gateway this adapter
    /// knows nothing about.
    pub(super) fn supports_reasoning_effort(&self, model: &str) -> bool {
        match self.id {
            ProviderId::OpenAi => {
                is_reasoning_model(model) || is_gpt5_or_later_reasoning_family(model)
            }
            ProviderId::OllamaCloud => super::super::ollama::ollama_family_supports_thinking(model),
            _ => false,
        }
    }

    /// Whether this client's provider id accepts OpenAI's `response_format`
    /// field on `/chat/completions` — native OpenAI (which defines it) and
    /// Ollama Cloud (whose `/v1` endpoint documents structured outputs through
    /// the same field). A generic `openai-compatible` gateway is an unknown
    /// catalog behind an unknown server build, exactly like
    /// [`Self::supports_reasoning_effort`]'s `_ => false` arm: guessing wrong
    /// 400s a whole generation, while omitting the field only costs the
    /// prompt-discipline fallback, so an unknown gateway is never guessed.
    pub(super) fn supports_response_format(&self) -> bool {
        matches!(self.id, ProviderId::OpenAi | ProviderId::OllamaCloud)
    }

    /// The [`ModelCapabilities`] this client reports for `model` — moved out
    /// of the `AiProvider::capabilities` trait method body so that method
    /// stays a thin delegator.
    pub(super) fn capabilities_impl(&self, model: &str) -> ModelCapabilities {
        // Rejecting `temperature` is an o-series-ONLY quirk — distinct from
        // "accepts reasoning_effort" (Ollama Cloud's gpt-oss/deepseek/qwen3
        // models accept both temperature AND reasoning_effort), so these are
        // two separate gates, not one reused variable.
        let rejects_temperature = is_reasoning_model(model);
        ModelCapabilities {
            supports_temperature: !rejects_temperature,
            supports_system_role: true,
            supports_streaming: true,
            supports_reasoning: self.supports_reasoning_effort(model),
            supports_tools: true,
            // Corrected from a blanket `true`: a generic `openai-compatible`
            // gateway is an unknown server build, and this adapter never
            // sends it `response_format` for exactly that reason — the
            // declared capability now matches what `complete_structured`
            // actually does. Same gate, one source of truth.
            supports_json_mode: self.supports_response_format(),
            supports_embeddings: true,
            // Only native OpenAI exposes the `web_search` tool; any
            // OpenAI-compatible gateway (LM Studio, OpenRouter, …) can't be
            // assumed to — see `supports_web_search()`.
            supports_web_search: self.supports_web_search(),
            token_param: if rejects_temperature {
                TokenParam::MaxCompletionTokens
            } else {
                TokenParam::MaxTokens
            },
        }
    }

    /// The effort levels this client reports for `model` — moved out of the
    /// `AiProvider::effort_levels` trait method body for the same reason as
    /// [`Self::capabilities_impl`].
    pub(super) fn effort_levels_impl(&self, model: &str) -> Vec<&'static str> {
        if self.supports_reasoning_effort(model) {
            OPENAI_EFFORT_LEVELS.to_vec()
        } else {
            Vec::new()
        }
    }

    /// The [`SamplingProfile`] this client reports for `model`/`intent` —
    /// moved out of the `AiProvider::sampling_profile` trait method body for
    /// the same reason as [`Self::capabilities_impl`].
    pub(super) fn sampling_profile_impl(&self, model: &str, intent: Intent) -> SamplingProfile {
        // Ollama Cloud gets its own family table — never the generic
        // native-OpenAI defaults below (see the doc comment on
        // `ollama_cloud_sampling_profile`).
        if self.id == ProviderId::OllamaCloud {
            return ollama_cloud_sampling_profile(model, intent);
        }
        // o-series (`is_reasoning_model`) genuinely reject `temperature`
        // (`capabilities().supports_temperature` already gates the send
        // site). gpt-5.x TECHNICALLY accepts a normal `temperature`/`top_p`
        // (see `is_gpt5_or_later_reasoning_family`'s doc comment) but a
        // reasoning model doesn't need the old per-task tuning either — both
        // families stay neutral here so this app never second-guesses their
        // own adaptive defaults.
        if is_reasoning_model(model) || is_gpt5_or_later_reasoning_family(model) {
            return SamplingProfile::default();
        }
        // `openai_sampling_profile`'s numbers ARE applied to
        // `OpenAiCompatible` gateways (LM Studio/vLLM/OpenRouter/custom
        // endpoints) too, deliberately — this is NOT the unknown-model
        // fail-safe the other adapters use for an unrecognized *model*.
        // `Intent` (e.g. `Deterministic`, which the analyze prompt's
        // strict-JSON contract relies on — `runAnalysis` hard-throws on a
        // parse failure) is an APP requirement on the response shape, not a
        // guess about a specific model's preferred creative sampling, so it
        // belongs on every provider that speaks this wire protocol and
        // accepts `temperature` — `caps.supports_temperature` (checked at
        // the send site) is what actually gates whether a value is sent at
        // all, same as native OpenAI. This also matches pre-fix behavior:
        // the renderer used to send these exact numbers to every
        // OpenAI-compatible gateway. Do not re-gate this on `self.id ==
        // ProviderId::OpenAi` — that was tried and reverted (it silently
        // neutralized the JSON-strict analysis surface for every gateway).
        openai_sampling_profile(intent)
    }
}
