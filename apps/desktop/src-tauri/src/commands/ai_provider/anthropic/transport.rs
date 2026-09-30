//! Anthropic key resolution + the `/v1/models` transport (listing, pagination,
//! key liveness). Split out of `anthropic.rs` (R8 LOC cap) — a pure move.

use serde_json::Value;
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;
use crate::error::{AppError, AppResult};

use super::super::pagination::incomplete_catalogue_error;
use super::super::{
    bounded, friendly_api_error, model_entry, pagination_step, parse_rfc3339_millis, timeouts,
    AiProvider, PaginationStep,
};
use super::{AnthropicClient, BASE, VERSION};

/// Safety bound on `list_models` pagination — real catalogues are a few dozen
/// models; this just prevents an unbounded loop if the API ever reports
/// `has_more` forever.
const MAX_LIST_MODELS_PAGES: usize = 50;

/// Resolve the stored Anthropic key, erroring on a missing/blank one and
/// TRIMMING the value it returns — not just checking the trimmed form is
/// non-empty and handing back the original padded string. A pasted key with
/// a trailing space/newline would otherwise reach the `x-api-key` header
/// as-is: a trailing space just 401s; an embedded `\n` makes the header value
/// invalid and the request never builds at all. Shared by `list_models` and
/// `test_key` so the two structurally agree on what counts as "no key"
/// (previously `test_key` alone accepted a whitespace-only key and burned a
/// 401 round-trip on it). Pure (no `AppHandle`) so it's unit-testable without
/// a mock-app harness.
pub(super) fn require_anthropic_key(stored: Option<String>) -> AppResult<String> {
    stored
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_string)
        .ok_or_else(|| AppError::Config("No API key found".to_string()))
}

/// Parse ONE page of the `/v1/models` response body into `{name, displayName?,
/// createdAt?, contextLength?}` entries (keeping only `claude-*` ids) plus the
/// cursor for the next page when `has_more` is true. Pure so it's
/// unit-testable without a network mock.
///
/// Anthropic's `/v1/models` returns `display_name` (string), `created_at`
/// (RFC3339 — normalized to epoch millis via [`parse_rfc3339_millis`]), and
/// `max_input_tokens` (integer) — verified against the live docs. Any field
/// the response omits is left out entirely, never defaulted.
pub(super) fn parse_model_page(body: &Value) -> AppResult<(Vec<Value>, Option<String>)> {
    let data = body.get("data").and_then(|d| d.as_array()).ok_or_else(|| {
        AppError::Provider("Anthropic: response missing `data` array".to_string())
    })?;
    let names = data
        .iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(|v| v.as_str())?;
            if !id.starts_with("claude-") {
                return None;
            }
            let display_name = m.get("display_name").and_then(|v| v.as_str());
            let created_at_ms = m
                .get("created_at")
                .and_then(|v| v.as_str())
                .and_then(parse_rfc3339_millis);
            let context_length = m.get("max_input_tokens").and_then(|v| v.as_i64());
            Some(model_entry(id, display_name, created_at_ms, context_length))
        })
        .collect();
    let has_more = body
        .get("has_more")
        .and_then(|b| b.as_bool())
        .unwrap_or(false);
    let cursor = if has_more {
        let last_id = body
            .get("last_id")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        match last_id {
            Some(id) => Some(id.to_string()),
            // `has_more: true` with no usable `last_id` is a malformed
            // response, not a clean end-of-pages — silently stopping here
            // would return a truncated catalogue as `Ok`, exactly the
            // silent-truncation bug pagination exists to prevent.
            None => {
                return Err(AppError::Provider(
                    "Anthropic: has_more is true but last_id is missing or blank".to_string(),
                ))
            }
        }
    } else {
        None
    };
    Ok((names, cursor))
}

impl AnthropicClient {
    /// The full paginated `/v1/models` transport — no `AppHandle`, so it's
    /// directly testable against a `wiremock::MockServer` by passing its
    /// `uri()` as `base` (production always calls this with `BASE`).
    /// Mirrors [`OpenAiClient::list_models_transport`](super::super::openai::OpenAiClient::list_models_transport).
    ///
    /// Loops through `after_id` pages up to `MAX_LIST_MODELS_PAGES`, bounded
    /// by a SINGLE cumulative `total_deadline` across every page (not a
    /// fresh timeout per request) — also a parameter (production always
    /// passes `timeouts::LIST_MODELS_TOTAL`) so a test can force it to
    /// expire in milliseconds instead of 30 real seconds.
    pub(super) async fn list_models_transport(
        &self,
        base: &str,
        api_key: &str,
        total_deadline: std::time::Duration,
    ) -> AppResult<Vec<Value>> {
        let client = crate::net::http::shared();
        let mut all = Vec::new();
        let mut after_id: Option<String> = None;
        let deadline = tokio::time::Instant::now() + total_deadline;
        for page_index in 0..MAX_LIST_MODELS_PAGES {
            let mut req = client
                .get(format!("{base}/models"))
                .header("x-api-key", api_key)
                .header("anthropic-version", VERSION)
                // No explicit `limit`: the `after_id` cursor loop below already
                // guarantees the full catalogue, so a page-size override would
                // only save a round-trip on a 300s-cached query — in exchange
                // for a hardcoded maximum we cannot verify, which would 400 the
                // WHOLE listing if it were ever wrong. Verified fields are
                // `has_more`/`last_id` (Anthropic SDK `SyncPage`); the accepted
                // `limit` ceiling is not, so we don't assert one.
                .timeout(timeouts::LIST_MODELS);
            if let Some(id) = &after_id {
                req = req.query(&[("after_id", id.as_str())]);
            }
            let name = self.id().as_str();
            // Every I/O step below races the SAME cumulative `deadline` via
            // `bounded` — not just the send. A stalled body would otherwise
            // blow straight through `total_deadline` even though the send
            // itself resolved (headers arrived) well within budget.
            let resp = bounded(deadline, name, req.send())
                .await?
                .map_err(|e| AppError::Network(format!("{name}: request failed: {e}")))?;
            let status = resp.status();
            if !status.is_success() {
                let body_text = bounded(
                    deadline,
                    name,
                    crate::net::http::read_text_capped(
                        resp,
                        crate::net::http::DEFAULT_MAX_BODY_BYTES,
                    ),
                )
                .await?
                .unwrap_or_default();
                return Err(friendly_api_error(self.id(), status, &body_text));
            }
            let body: Value = bounded(
                deadline,
                name,
                crate::net::http::read_json_capped::<Value>(
                    resp,
                    crate::net::http::DEFAULT_MAX_BODY_BYTES,
                ),
            )
            .await?
            .map_err(|e| AppError::Provider(format!("{name}: parse: {e}")))?;
            let (mut page, cursor) = parse_model_page(&body)?;
            all.append(&mut page);
            match pagination_step(page_index, MAX_LIST_MODELS_PAGES, &after_id, cursor) {
                PaginationStep::Continue(id) => after_id = Some(id),
                PaginationStep::Done => break,
                PaginationStep::Stalled => {
                    return Err(AppError::Provider(format!(
                        "{name}: has_more is true but last_id didn't advance — the provider claims another page exists but gave no way to reach it"
                    )))
                }
                PaginationStep::Incomplete => {
                    return Err(incomplete_catalogue_error(name, MAX_LIST_MODELS_PAGES))
                }
            }
        }
        Ok(all)
    }

    /// Key-liveness probe body of `AiProvider::test_key` — moved out of the
    /// trait method so it stays a thin delegator.
    pub(super) async fn test_key_impl(&self, app: &AppHandle) -> AppResult<()> {
        let api_key = require_anthropic_key(get_provider_key(app, self.id().credential_key()))?;
        // Liveness probe via `GET /v1/models` (the same endpoint `list_models`
        // uses). A key-only authenticated GET avoids pinning a specific chat model
        // snapshot — the old probe hardcoded `claude-3-haiku-20240307`, so key
        // validation would have broken the day that snapshot is retired.
        let client = crate::net::http::shared();
        let resp = client
            .get(format!("{BASE}/models"))
            .header("x-api-key", &api_key)
            .header("anthropic-version", VERSION)
            .timeout(timeouts::LIST_MODELS)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            Err(friendly_api_error(self.id(), status, &body_text))
        }
    }
}
