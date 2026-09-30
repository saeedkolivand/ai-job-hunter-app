//! Gemini key resolution + the `/v1beta/models` transport (listing,
//! pagination, key liveness). Split out of `gemini.rs` (R8 LOC cap) — a pure
//! move.

use serde_json::Value;
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;
use crate::error::{AppError, AppResult};

use super::super::pagination::incomplete_catalogue_error;
use super::super::{
    bounded, friendly_api_error, model_entry, pagination_step, timeouts, AiProvider,
    PaginationStep, ProviderId,
};
use super::GeminiClient;

/// Safety bound on `list_models` pagination — real catalogues are a few dozen
/// models; this just prevents an unbounded loop if the API ever returns a
/// `nextPageToken` forever.
const MAX_LIST_MODELS_PAGES: usize = 50;

/// Validate the key the keychain returned, rejecting a missing/blank one
/// early and TRIMMING the value it returns — not just checking the trimmed
/// form is non-empty and handing back the original padded string. A pasted
/// key with a trailing space/newline would otherwise reach the
/// `x-goog-api-key` header as-is: a trailing space just 401s; an embedded
/// `\n` makes the header value invalid and the request never builds at all.
///
/// Pure (no `AppHandle`) so it's unit-testable. Several call paths previously
/// defaulted a missing key to `""` and still issued the request, sending an empty
/// `x-goog-api-key` header — a guaranteed 401 round-trip. This fails fast with the
/// same unauthorized error `friendly_api_error` maps a real 401/403 to, so the
/// message stays consistent.
pub(super) fn validate_gemini_key(stored: Option<String>) -> AppResult<String> {
    match stored.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        Some(k) => Ok(k.to_string()),
        None => Err(AppError::Config(format!(
            "{}: invalid or unauthorized API key.",
            ProviderId::Gemini.as_str()
        ))),
    }
}

/// Resolve the stored Gemini key, rejecting a missing/blank one before any request.
pub(super) fn require_gemini_key(app: &AppHandle) -> AppResult<String> {
    validate_gemini_key(get_provider_key(app, ProviderId::Gemini.credential_key()))
}

/// Parse ONE page of the `/v1beta/models` response body into `{name,
/// displayName?, contextLength?}` entries, stripping the `models/` prefix
/// Gemini's wire format uses, plus the `nextPageToken` for the next page, if
/// any. `-preview`/experimental ids are NOT filtered out here — `/v1beta`
/// (unlike `/v1`) lists them, and they're valid, selectable models (e.g. the
/// curated Pro-tier default in `provider-meta.ts` is a `-preview` id). Pure
/// so it's unit-testable without a network mock.
///
/// Gemini's `/v1beta/models` returns `displayName` (string) and
/// `inputTokenLimit` (integer) — verified against the live docs. It does
/// **not** return a creation timestamp at all, so no `createdAt` field is
/// ever populated for this provider — never a fabricated one.
pub(super) fn parse_model_page(body: &Value) -> AppResult<(Vec<Value>, Option<String>)> {
    let models = body
        .get("models")
        .and_then(|d| d.as_array())
        .ok_or_else(|| AppError::Provider("Gemini: response missing `models` array".to_string()))?;
    let names = models
        .iter()
        .filter_map(|m| {
            let raw_name = m.get("name").and_then(|v| v.as_str())?;
            if !raw_name.starts_with("models/") {
                return None;
            }
            let name = raw_name.strip_prefix("models/").unwrap_or(raw_name);
            let display_name = m.get("displayName").and_then(|v| v.as_str());
            let context_length = m.get("inputTokenLimit").and_then(|v| v.as_i64());
            Some(model_entry(name, display_name, None, context_length))
        })
        .collect();
    let next_page_token = body
        .get("nextPageToken")
        .and_then(|t| t.as_str())
        .filter(|t| !t.is_empty())
        .map(String::from);
    Ok((names, next_page_token))
}

impl GeminiClient {
    /// The full paginated `/v1beta/models` transport — no `AppHandle`, so
    /// it's directly testable against a `wiremock::MockServer` by passing
    /// its `uri()` as `base` (production always calls this with `BASE`).
    /// Mirrors [`OpenAiClient::list_models_transport`](super::super::openai::OpenAiClient::list_models_transport).
    ///
    /// Loops through `pageToken` pages up to `MAX_LIST_MODELS_PAGES`,
    /// bounded by a SINGLE cumulative `total_deadline` across every page
    /// (not a fresh timeout per request) — also a parameter (production
    /// always passes `timeouts::LIST_MODELS_TOTAL`) so a test can force it
    /// to expire in milliseconds instead of 30 real seconds.
    pub(super) async fn list_models_transport(
        &self,
        base: &str,
        api_key: &str,
        total_deadline: std::time::Duration,
    ) -> AppResult<Vec<Value>> {
        let client = crate::net::http::shared();
        let mut all = Vec::new();
        let mut page_token: Option<String> = None;
        let deadline = tokio::time::Instant::now() + total_deadline;
        for page_index in 0..MAX_LIST_MODELS_PAGES {
            let mut req = client
                .get(format!("{base}/v1beta/models"))
                .header("x-goog-api-key", api_key)
                // No explicit `pageSize` — see the matching note in
                // `anthropic.rs`: the `pageToken` loop already yields every
                // page, so an unverified page-size ceiling would risk 400-ing
                // the whole listing to save one cached round-trip.
                .timeout(timeouts::LIST_MODELS);
            if let Some(token) = &page_token {
                req = req.query(&[("pageToken", token.as_str())]);
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
            let (mut page, next) = parse_model_page(&body)?;
            all.append(&mut page);
            match pagination_step(page_index, MAX_LIST_MODELS_PAGES, &page_token, next) {
                PaginationStep::Continue(token) => page_token = Some(token),
                PaginationStep::Done => break,
                PaginationStep::Stalled => {
                    return Err(AppError::Provider(format!(
                        "{name}: a nextPageToken is present but didn't advance — the provider claims another page exists but gave no way to reach it"
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
        let api_key = require_gemini_key(app)?;
        let client = crate::net::http::shared();
        let resp = client
            .get(format!("{}/v1beta/models", super::BASE))
            .header("x-goog-api-key", &api_key)
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
