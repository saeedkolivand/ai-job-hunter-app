//! Ollama's `/api/tags` catalogue: parsing, the LOCAL model-list command's
//! best-effort fetch, the embedding-vs-chat name heuristic, and the health
//! probe's first-chat-model detection. Split out of `ollama.rs` (R8 LOC cap)
//! — a pure move.

use serde_json::Value;

use crate::error::{AppError, AppResult};

use super::super::{map_completion_transport_error, model_entry, parse_rfc3339_millis, timeouts};
use super::host;

/// Parse the `/api/tags` response body into `{name, createdAt?}` entries.
/// Pure so it's unit-testable without a network mock.
///
/// `/api/tags` returns `modified_at` (RFC3339, possibly with a non-UTC
/// offset) — normalized to epoch millis via [`parse_rfc3339_millis`], the
/// convention every `createdAt` field in this codebase uses. Neither
/// `displayName` nor `contextLength` is ever populated: `/api/tags` reports
/// neither (a model's context length is only on `/api/show`, a different
/// endpoint this function doesn't call).
pub(super) fn parse_model_list(body: &Value) -> AppResult<Vec<Value>> {
    let models = body
        .get("models")
        .and_then(|m| m.as_array())
        .ok_or_else(|| AppError::Provider("Ollama: response missing `models` array".to_string()))?;
    Ok(models
        .iter()
        .filter_map(|m| {
            let name = m.get("name").and_then(|n| n.as_str())?;
            let created_at_ms = m
                .get("modified_at")
                .and_then(|v| v.as_str())
                .and_then(parse_rfc3339_millis);
            Some(model_entry(name, None, created_at_ms, None))
        })
        .collect())
}

/// Fetch + parse `/api/tags` — `Err` on any transport, status, parse, or
/// missing-field failure; `Ok(vec![])` only for a genuinely empty catalogue.
/// Ollama needs no key, so there is no missing-key case here.
pub(super) async fn fetch_tag_models() -> AppResult<Vec<Value>> {
    let resp = crate::net::http::shared()
        .get(format!("{}/api/tags", host()))
        .timeout(timeouts::LIST_MODELS)
        .send()
        .await
        .map_err(|e| map_completion_transport_error(e, "Ollama", timeouts::LIST_MODELS))?;
    if !resp.status().is_success() {
        return Err(AppError::Provider(format!(
            "Ollama returned status: {}",
            resp.status()
        )));
    }
    let body: Value =
        crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
            .await
            .map_err(|e| AppError::Provider(format!("Ollama parse: {e}")))?;
    parse_model_list(&body)
}

/// `{ name }` list from `/api/tags` — best-effort: collapses any transport,
/// status, or parse failure to an empty list. Backs `ai_list_models` (the
/// LOCAL model-list command, distinct from `ai_list_provider_models`), which
/// is out of scope for this trait method's error-surfacing contract.
pub async fn list_tag_models() -> Vec<Value> {
    fetch_tag_models().await.unwrap_or_default()
}

/// Whether a local model name is an EMBEDDING-only model, i.e. one that
/// `/api/chat` rejects with a 400.
///
/// Ollama's `/api/tags` does not mark this: `details.family` is `bert` for some
/// (`nomic-embed-text`, `mxbai-embed-large`) but the base family for others
/// (`qwen3-embedding` reports `qwen3`), so the name is the only signal actually
/// present in the listing. Every embedding model Ollama publishes carries
/// `embed`/`embedding` in its name, which is what this matches.
///
/// ponytail: name heuristic, not a capability probe. The ceiling is a future
/// embedding model named without `embed` — it would be picked for chat and get
/// one 400, exactly as today, but now WARN-logged instead of silent. Upgrade
/// path if that happens: probe `/api/show` per model and read `capabilities`.
/// Pure + unit-tested.
pub(super) fn is_embedding_only_model(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("embed")
}

/// The first model in an `/api/tags` body that can actually hold a chat turn.
/// `None` when the user has ONLY embedding models installed. Used only for the
/// system-health panel's "detected local chat model" display
/// (`commands::system::system_health`) — job-ad translation reads the user's
/// own ACTIVE model instead (`commands::translation::resolve_translation_target`),
/// never this first-installed guess. Pure + unit-tested.
pub(super) fn first_chat_model(body: &Value) -> Option<String> {
    body.get("models")
        .and_then(|m| m.as_array())
        .into_iter()
        .flatten()
        .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
        .find(|name| !is_embedding_only_model(name))
        .map(String::from)
}

/// The user's configured Ollama chat model when `/api/tags` lists it (and it can
/// chat), else [`first_chat_model`]. Without this the health chip named whichever
/// model Ollama happened to list first, not the one the user runs (#1365).
pub(super) fn preferred_chat_model(body: &Value, preferred: Option<&str>) -> Option<String> {
    // `qwen3` and `qwen3:latest` are the same model to Ollama; report the LISTED spelling.
    let base = |n: &str| n.strip_suffix(":latest").unwrap_or(n).to_string();
    let listed = preferred
        .filter(|p| !is_embedding_only_model(p))
        .and_then(|want| {
            body.get("models")
                .and_then(|m| m.as_array())
                .into_iter()
                .flatten()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                .find(|name| base(name) == base(want))
        });
    listed.map(String::from).or_else(|| first_chat_model(body))
}

/// `(reachable, CHAT_model_name)` — the local health probe behind
/// `system_health`'s "ai.ready"/"ai.model" fields.
///
/// The chat filter is not cosmetic: this once returned `arr.first()`
/// unconditionally, so a user whose first `/api/tags` entry was
/// `qwen3-embedding:8b` saw that reported as the "detected" chat model. Job-ad
/// translation no longer calls this at all — see `first_chat_model`'s doc.
pub async fn reachable_model(preferred: Option<&str>) -> (bool, Option<String>) {
    match crate::net::http::shared()
        .get(format!("{}/api/tags", host()))
        .timeout(timeouts::HEALTH)
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => {
            let body: Value =
                crate::net::http::read_json_capped(r, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            (true, preferred_chat_model(&body, preferred))
        }
        _ => (false, None),
    }
}
