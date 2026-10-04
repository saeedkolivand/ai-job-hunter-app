//! Write-time validation of a provider's settings: the strict check the
//! interactive writer runs, its lenient sibling for seed/import, and the
//! context-window bound both share.

use super::{AiConfigStore, MAX_CONTEXT_WINDOW, MIN_CONTEXT_WINDOW};
use crate::commands::ai_provider::ProviderId;
use crate::error::AppResult;

impl AiConfigStore {
    /// Strict validation used by the interactive writer: a cross-family model or a
    /// bad base_url is a hard error (surfaced to the user in Settings). Trims and
    /// drops empty strings so an empty model/base_url stores as NULL. An empty
    /// model is allowed here (a valid intermediate settings state, and legitimate
    /// for CLI agents) — the "no model selected" rule is enforced at generation
    /// resolve time (`Completer::from_active`), not at settings-write time.
    pub(super) fn validate_settings(
        provider_id: ProviderId,
        model: Option<String>,
        base_url: Option<String>,
        context_window: Option<u32>,
    ) -> AppResult<(Option<String>, Option<String>, Option<u32>)> {
        let model = model
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty());
        if let Some(ref m) = model {
            provider_id.validate_model(m)?;
        }
        let context_window = validate_context_window(context_window)?;
        // `base_url` is only meaningful for `OpenAiCompatible` — `resolve()`
        // ignores it for every other provider. It's inert for egress there, but
        // a stored value still reaches `record_usage`'s free/paid cost gate, so
        // drop it to NULL for any other provider rather than persist dead data
        // that could nudge cost classification.
        let base_url = if matches!(provider_id, ProviderId::OpenAiCompatible) {
            base_url
                .map(|u| u.trim().to_string())
                .filter(|u| !u.is_empty())
        } else {
            None
        };
        if let Some(ref u) = base_url {
            crate::net::ssrf::validate_provider_base_url(u)?;
        }
        Ok((model, base_url, context_window))
    }

    /// Lenient sibling of [`Self::validate_settings`] for seed/import: drop a
    /// cross-family model and a bad base_url instead of erroring, so a first-run
    /// seed or a restore never fails on one bad field.
    pub(super) fn scrub_settings(
        provider_id: ProviderId,
        model: Option<String>,
        base_url: Option<String>,
        context_window: Option<u32>,
    ) -> (Option<String>, Option<String>, Option<u32>) {
        let model = model
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .filter(|m| provider_id.validate_model(m).is_ok());
        // Same non-`OpenAiCompatible` guard as `validate_settings` — a
        // native-provider base_url from a first-run renderer seed or a restored
        // backup bundle is inert for egress but still reaches `record_usage`'s
        // free/paid cost gate, so drop it to NULL rather than persist it.
        let base_url = if matches!(provider_id, ProviderId::OpenAiCompatible) {
            base_url
                .map(|u| u.trim().to_string())
                .filter(|u| !u.is_empty())
                .filter(|u| crate::net::ssrf::validate_provider_base_url(u).is_ok())
        } else {
            None
        };
        // Out-of-range → dropped, never clamped: a clamp would silently invent
        // a window the user never chose, and the provider's own default is the
        // honest answer to "we don't know".
        let context_window = context_window.filter(|c| validate_context_window(Some(*c)).is_ok());
        (model, base_url, context_window)
    }
}

/// A stored context window, or a hard error naming the bound it broke.
///
/// The value reaches an Ollama request as `options.num_ctx`, where an absurd
/// number is not merely wrong — it is an out-of-memory kill of the user's
/// machine on the next generation.
pub fn validate_context_window(context_window: Option<u32>) -> AppResult<Option<u32>> {
    match context_window {
        Some(c) if !(MIN_CONTEXT_WINDOW..=MAX_CONTEXT_WINDOW).contains(&c) => Err(format!(
            "A context window of {c} is outside the supported range \
             {MIN_CONTEXT_WINDOW}–{MAX_CONTEXT_WINDOW} tokens."
        )
        .into()),
        other => Ok(other),
    }
}
