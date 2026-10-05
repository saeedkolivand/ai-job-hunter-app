//! The `ai_*` IPC commands. The command bodies live in one submodule per
//! responsibility (R8, issue #1280) and are glob re-exported here: a
//! `#[tauri::command]` also generates sibling macros the `generate_handler!` list
//! looks up beside the function, and a glob carries those along, so every command
//! keeps its `commands::ai::<name>` path — the path the handler list and the
//! agent-CLI policy table name it by.

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::documents::{embedding_space_changed, DocumentStore, EmbeddingConfig};
use crate::error::AppResult;
use crate::postings::PostingsCache;

use super::ai_provider::{resolve, ProviderId};

mod active_config;
mod embeddings;
mod generate;
mod local_models;
mod provider_keys;
mod research;

pub use active_config::*;
pub use embeddings::*;
pub use generate::*;
pub use local_models::*;
pub use provider_keys::*;
pub use research::*;

// ── AI-spend visibility (issue #1161) ─────────────────────────────────────
// The pure/`AppHandle`-free helpers below live in `spend.rs`, split out
// purely for R8 (the 1400-LOC hard cap) — same shape as
// `commands::match_resume`'s `constraints` split. The `#[tauri::command]`
// itself stays here, next to its `mod spend;`, so it's reachable at
// `commands::ai::ai_spend_summary` — the exact path `tauri::generate_handler!`
// and the agent-cli policy registry name it by.
mod spend;

/// Read-only AI-spend summary: `today`'s REAL per-provider token totals — as
/// reported by each provider's own response, never estimated (see
/// `commands::ai_provider::stream` / `pipeline::Completer::complete`, the two
/// chokepoints that record them) — plus an ESTIMATED USD cost from a static
/// list-price rate table (`crate::spend::estimate_cost`). The dollar figure is
/// a best-effort ballpark, not a billing-accurate source: a BYO-key user has
/// no billing API to query. Local (Ollama) and CLI-agent calls always cost
/// $0. A missing store (failed to open at startup) degrades to all-zero
/// rather than erroring.
///
/// `days` (issue #1161) scopes `windowTotals` and `perProvider` to the last N
/// UTC days ending today; `1` (the default, and pre-#1161 behavior) makes the
/// window "since midnight today", the same span `today` always covers.
/// `today` is ALWAYS calendar-day — a `days > 1` caller must read its
/// multi-day total from `windowTotals`, never from `today` (issue #1161's
/// C1-r1-RBA-1: an aggregate is labelled with the period it actually covers).
/// Clamped to [`crate::spend::SPEND_WINDOW_MAX_DAYS`]. The
/// resolved window is reported back as `window` so a caller never has to
/// re-derive what it asked for. `perProvider` lists every provider that has
/// EVER recorded a call, not just ones active in this window — a provider
/// with no activity here still gets a zero row, with a short `reason`
/// (`crate::spend::zero_row_reason`). `thinkingByModel` stays all-history
/// (`thinkingByModelWindow: "allTime"`) — it answers "how does this model
/// behave", not "what did this window cost".
#[tauri::command]
pub fn ai_spend_summary(app: AppHandle, days: Option<u32>) -> Value {
    let days = spend::resolve_window_days(days);
    let Some(store) = app.try_state::<crate::spend::SpendStore>() else {
        return spend::zero_summary(days);
    };
    spend::spend_summary_from_store(&store, days)
}

/// Scrub-then-validate `base_url` before it can reach persistence — the exact
/// pair `AiConfigStore::validate_settings` (`ai_config/validation.rs`) applies for
/// `ai_set_provider_settings`, extracted here as a pure, AppHandle-free
/// function so `ai_set_embedding_config` below stops being the one setter
/// that persists a renderer-supplied embedding endpoint (carrying the
/// provider API key plus résumé/job text on every embed call) unvalidated.
/// `base_url` only means anything for `OpenAiCompatible` (see `resolve`'s
/// doc comment), so it is dropped for every other provider before
/// validation; whatever survives is checked against
/// `net::ssrf::validate_provider_base_url`, which deliberately keeps
/// loopback/LAN addresses — a local LM Studio/vLLM/Ollama endpoint must keep
/// working.
fn scrub_and_validate_embedding_base_url(
    provider_id: ProviderId,
    base_url: Option<String>,
) -> AppResult<Option<String>> {
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
    Ok(base_url)
}

/// Set the active embedding provider/model. The provider must support embeddings
/// (validated server-side); an empty model resolves to the provider's default.
/// Changing this changes the embedding space — call `ai_reembed_all` afterwards
/// to rebuild the index so comparisons stay valid.
#[tauri::command]
pub async fn ai_set_embedding_config(
    app: AppHandle,
    provider: String,
    model: Option<String>,
    base_url: Option<String>,
) -> Value {
    let provider_id = match ProviderId::parse(&provider) {
        Ok(p) => p,
        Err(e) => return json!({ "success": false, "error": e }),
    };
    let base_url = match scrub_and_validate_embedding_base_url(provider_id, base_url) {
        Ok(u) => u,
        Err(e) => return json!({ "success": false, "error": e }),
    };
    let client = resolve(provider_id, base_url.clone());
    let model = model
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .or_else(|| client.default_embedding_model().map(String::from));
    let model = match model {
        Some(m) => m,
        None => {
            return json!({
                "success": false,
                "error": format!("{} does not support embeddings.", provider_id.as_str()),
            })
        }
    };
    if !client.capabilities(&model).supports_embeddings {
        return json!({
            "success": false,
            "error": format!("{} does not support embeddings.", provider_id.as_str()),
        });
    }
    let cfg = EmbeddingConfig {
        provider: provider_id.as_str().to_string(),
        model,
        base_url,
    };
    let store = app.state::<DocumentStore>();
    // Whether this is a real space change — the posting_vectors / match_scores
    // caches key on provider+model, so their old-space rows become unreachable
    // and must be reclaimed only when the space actually changes. Decision lives
    // in `embedding_space_changed` (shared with its unit test).
    let space_changed = embedding_space_changed(&store.embedding_config(), &cfg);
    match store.set_embedding_config(&cfg) {
        Ok(()) => {
            if space_changed {
                // Evict stale-space cache rows (mirrors how `ai_reembed_all`
                // clears the live `PostingsCache` embeddings).
                store.clear_posting_vectors().ok();
                store.clear_match_scores().ok();
                // Same reason, same branch: `help_vectors` keys on the
                // embedding space too, so every row in it is unreachable
                // after this flip (`documents::help_vectors`).
                store.clear_help_vectors().ok();
                // The comment above claimed this already happened; it never
                // did. `commands::hybrid_search`'s dense arm is the first
                // production consumer of `PostingsCache`'s embedding cache
                // (`postings::PostingsCache::get_embedding`/`set_embedding`),
                // and `EmbeddingConfig::matches` only guards a READ against a
                // stale-space row — a config flip with no read in between
                // would otherwise leave stale-space vectors sitting in the
                // cache indefinitely (harmless until read, but exactly the
                // dead-weight `ai_reembed_all`'s own clear exists to avoid).
                app.state::<Mutex<PostingsCache>>()
                    .lock()
                    .clear_embeddings();
            }
            json!({
                "success": true,
                "config": { "provider": cfg.provider, "model": cfg.model, "baseUrl": cfg.base_url },
            })
        }
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[cfg(test)]
mod tests;
