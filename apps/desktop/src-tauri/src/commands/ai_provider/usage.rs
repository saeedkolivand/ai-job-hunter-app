//! Real per-call AI-spend visibility: the [`Usage`] type + [`record_usage`]
//! chokepoint. Split out of `mod.rs` (R8 line-budget split).

use tauri::{AppHandle, Manager};

/// Real per-call token usage as reported by the provider's own response —
/// never estimated. Zero on both fields when a provider genuinely reports no
/// usage (e.g. a CLI agent — see `cli_agent`, which relies on the
/// [`super::AiProvider::complete_with_usage`] default rather than fabricating a
/// number). Consumed by `crate::spend` to compute an estimated dollar cost.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// Reasoning/"thinking" tokens, **only when the provider reports them as a
    /// distinct number**. `None` is not zero: it means this provider does not
    /// separate them, and recording a zero would read as "this model did no
    /// reasoning" — the opposite of the truth for a reasoning model.
    ///
    /// Who reports what, as of the adapters in this module:
    ///
    /// * OpenAI — `usage.completion_tokens_details.reasoning_tokens`. Current
    ///   Chat Completions models send the details object with a **`0`** here
    ///   when they did no reasoning, rather than omitting it, so a
    ///   non-reasoning OpenAI model records `Some(0)` — a measured zero, which
    ///   is a fact and not a fabrication. `None` is reserved for "the field was
    ///   not there at all" (an older/compatible gateway).
    /// * Gemini — `usageMetadata.thoughtsTokenCount`, which this app already
    ///   opts into by sending `thinkingConfig.includeThoughts`.
    /// * Anthropic — NOT reported separately; thinking tokens are counted
    ///   inside `output_tokens`.
    /// * Ollama — NOT reported separately; `eval_count` includes the thinking
    ///   channel. (The renderer measures the thinking/answer split in CHARS off
    ///   the live stream — see `GeneratingPanel` — which is a different unit
    ///   and is deliberately not written here as if it were tokens.)
    /// * CLI agents — report no usage at all.
    ///
    /// Where reported, it is a SUBSET of `output_tokens`, not an addition to
    /// it, so cost estimation is unaffected.
    pub thinking_tokens: Option<u32>,
}

/// Record one AI call's REAL token usage against today's spend via the
/// managed [`crate::spend::SpendStore`], if one is present. Best-effort:
/// spend tracking never blocks or fails a generation — a missing store (e.g.
/// it failed to open at startup) is silently skipped, exactly like the other
/// `try_state`-gated convenience writers in this crate (see
/// `commands::notifications::push_and_notify`). `base_url` is whatever base
/// URL the caller resolved the request against — passed straight through to
/// [`crate::spend::SpendStore::record`]'s free/paid cost gate, which only
/// ever consults it for the `openai-compatible` provider id (every other
/// provider ignores it), so a local LM Studio/llama.cpp/vLLM server never
/// shows a fake dollar figure. Pass `None` when no base URL was resolved
/// (every non-`openai-compatible` provider).
///
/// Lives HERE (the command/shell layer, L3) rather than in `crate::spend`
/// (a data-layer store, L1) because it needs `AppHandle`/`Manager` to resolve
/// the managed state — the architecture boundary test (R2: no Tauri below the
/// shell layer) forbids a store module from importing `tauri::*` itself.
/// `crate::spend::SpendStore` stays Tauri-free; this is the AppHandle→
/// `try_state`→`record` hop every call site (streaming, `Completer`, CLI
/// agents, `embed_text`) goes through.
///
/// Takes the whole [`Usage`] rather than loose token counts so a field the
/// providers report (today `thinking_tokens`) cannot be parsed at the adapter
/// and then dropped on the way to the store — which is exactly what a widening
/// parameter list invites.
pub(crate) fn record_usage(
    app: &AppHandle,
    provider: &str,
    model: &str,
    usage: Usage,
    base_url: Option<&str>,
) {
    if let Some(store) = app.try_state::<crate::spend::SpendStore>() {
        store.record(crate::spend::SpendRecord {
            provider: provider.to_string(),
            model: model.to_string(),
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            thinking_tokens: usage.thinking_tokens,
            run_id: None,
            base_url: base_url.map(str::to_string),
        });
    }
}
