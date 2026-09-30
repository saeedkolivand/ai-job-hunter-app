//! OpenAI-compatible key resolution + the `/models` listing transport
//! (endpoint construction, catalogue filtering, key liveness). Split out of
//! `openai.rs` (R8 LOC cap) — a pure move.

use serde_json::Value;
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;
use crate::error::{AppError, AppResult};

use super::super::{friendly_api_error, model_entry, timeouts, ProviderId};
use super::OpenAiClient;

/// Strip the query string / fragment from a failed request's URL before it
/// reaches an error message or log line. Some OpenAI-compatible gateways put
/// the API key in the base URL's own query string (see
/// [`OpenAiClient::endpoint_url`]'s doc comment) — `reqwest::Error`'s own
/// `Display` embeds the request URL verbatim and only ever strips userinfo,
/// never query or fragment; confirmed via `reqwest::Error::without_url`'s own
/// doc: "If the URL contains sensitive information (e.g. an API key as a
/// query parameter), be sure to remove it." Verified empirically (see the
/// tests) that even a CORRECTLY built [`OpenAiClient::endpoint_url`] still
/// carries the secret into a genuine transport-failure `Display` — fixing the
/// URL construction alone does not stop the leak. Clears only the
/// query/fragment (via `url_mut`), not the whole URL, so scheme/host/path
/// stay visible for diagnosing a wrong-path bug.
pub(super) fn scrub_url_secret(mut e: reqwest::Error) -> reqwest::Error {
    if let Some(url) = e.url_mut() {
        url.set_query(None);
        url.set_fragment(None);
    }
    e
}

/// Whether a model id returned by `/v1/models` should be offered in the picker.
/// Native OpenAI exposes a large non-chat catalog (embeddings, audio, image,
/// moderation…), so restrict it to chat-capable families. Every *other*
/// OpenAI-compatible backend (custom gateways, Ollama Cloud, …) returns a curated
/// catalog of its own models under arbitrary names, so pass those through
/// unfiltered — that way a new composed provider lists its full catalog with no
/// code change here.
pub(super) fn should_list_model(provider: ProviderId, id: &str) -> bool {
    provider != ProviderId::OpenAi
        || id.starts_with("gpt-")
        || id.starts_with("o1")
        || id.starts_with("o3")
        || id.starts_with("o4")
        || id.starts_with("chatgpt")
}

/// Resolve the stored key for `list_models`/`test_key`, TRIMMING the value it
/// returns — not just checking the trimmed form is non-empty and handing back
/// the original padded string. A pasted key with a trailing space/newline
/// would otherwise reach `bearer_auth` as-is: a trailing space just 401s; an
/// embedded `\n` makes the header value invalid and the request never builds
/// at all.
///
/// Missing/blank errors for every provider EXCEPT `OpenAiCompatible`: its
/// keyless self-hosted deployments (LM Studio, vLLM, …) are an explicitly
/// supported configuration (`mod.rs`'s `ProviderId::OpenAiCompatible` doc)
/// that already generates fine with no key — `chat_stream`/`chat_with_tools`
/// default a missing key to `""` and send it regardless — so hard-requiring
/// one here would cement "generates fine, listing/testing always errors" for
/// a working setup. `Ok(None)` means "build the request with no bearer
/// header" (never an empty `Authorization: Bearer` value — some gateways
/// reject a malformed header rather than ignoring it). Shared by
/// `list_models` and `test_key` so the two structurally agree on what counts
/// as "no key" (previously `test_key` alone accepted a whitespace-only key
/// and burned a round-trip on it). Pure (no `AppHandle`) so it's
/// unit-testable without a mock-app harness.
pub(super) fn resolve_openai_key(
    provider: ProviderId,
    stored: Option<String>,
) -> AppResult<Option<String>> {
    let trimmed = stored
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_string);
    if trimmed.is_some() || provider == ProviderId::OpenAiCompatible {
        Ok(trimmed)
    } else {
        Err(AppError::Config("No API key found".to_string()))
    }
}

/// Parse the `/models` response body into `{name, createdAt?}` entries,
/// applying [`should_list_model`]'s per-provider filter. Pure so it's
/// unit-testable without a network mock.
///
/// OpenAI's `/v1/models` (and every OpenAI-compatible gateway that mirrors
/// its schema — Ollama Cloud included) reports `created` as unix epoch
/// SECONDS — verified against the live docs, normalized to epoch millis (the
/// convention every `createdAt` field in this codebase uses) via a
/// `checked_mul`, never a bare `* 1000`: `created` is provider-controlled, so
/// an unchecked multiply can overflow `i64` (panics in debug, silently wraps
/// in release). Omit `createdAt` entirely on overflow — never a fabricated
/// timestamp. Neither `displayName` nor `contextLength` is ever populated:
/// OpenAI's catalogue endpoint doesn't return either.
pub(super) fn parse_model_list(provider: ProviderId, body: &Value) -> AppResult<Vec<Value>> {
    let data = body.get("data").and_then(|d| d.as_array()).ok_or_else(|| {
        AppError::Provider(format!(
            "{}: response missing `data` array",
            provider.as_str()
        ))
    })?;
    Ok(data
        .iter()
        .filter_map(|m| {
            let id = m.get("id").and_then(|v| v.as_str())?;
            if !should_list_model(provider, id) {
                return None;
            }
            let created_at_ms = m
                .get("created")
                .and_then(|v| v.as_i64())
                .and_then(|secs| secs.checked_mul(1000));
            Some(model_entry(id, None, created_at_ms, None))
        })
        .collect())
}

impl OpenAiClient {
    /// Build a URL for `path` (a `/`-separated relative endpoint, e.g.
    /// `"models"` or `"chat/completions"`) on `self.base_url`, preserving any
    /// existing query string / fragment the base carries untouched. Some
    /// OpenAI-compatible gateways (Cloudflare AI Gateway, several self-hosted
    /// proxies) authenticate via the base URL's own query string — e.g.
    /// `https://gw.example.com/v1?api-key=SECRET`. Plain
    /// `format!("{base}/{path}")` string concatenation sends THAT case to the
    /// wrong path (the string reparses as path `/v1`, query
    /// `api-key=SECRET/path`) and corrupts the key. `Url::join` is not a safe
    /// drop-in either — verified empirically (see the tests): a plain
    /// relative reference like `"models"` carries no query of its own, and
    /// WHATWG relative-URL resolution defines that as "clear the query" on
    /// join, so it would silently DROP a working gateway's auth query string
    /// rather than construct a malformed URL. `path_segments_mut` (with
    /// `pop_if_empty` so a base with OR without a trailing slash both resolve
    /// correctly, never a double slash) only appends path segments and
    /// leaves scheme/host/query/fragment untouched — the correct primitive
    /// for "hit a sibling endpoint on the same base".
    pub(super) fn endpoint_url(&self, path: &str) -> AppResult<reqwest::Url> {
        let mut url = reqwest::Url::parse(&self.base_url).map_err(|e| {
            AppError::Config(format!("{}: invalid base URL: {e}", self.id.as_str()))
        })?;
        url.path_segments_mut()
            .map_err(|()| {
                AppError::Config(format!(
                    "{}: base URL has no host to build an endpoint on",
                    self.id.as_str()
                ))
            })?
            .pop_if_empty()
            .extend(path.split('/'));
        Ok(url)
    }

    /// Build the `GET {base_url}/models` request, attaching the bearer header
    /// only when a key is present — never an empty `Authorization: Bearer`
    /// value for a keyless `OpenAiCompatible` deployment (some gateways
    /// reject a malformed/empty header rather than ignoring it). Shared by
    /// `list_models_transport` and `test_key`.
    pub(super) fn list_models_request(
        &self,
        api_key: Option<&str>,
    ) -> AppResult<reqwest::RequestBuilder> {
        let url = self.endpoint_url("models")?;
        let req = crate::net::http::shared()
            .get(url)
            .timeout(timeouts::LIST_MODELS);
        Ok(match api_key {
            Some(key) => req.bearer_auth(key),
            None => req,
        })
    }

    /// The `/models` HTTP transport itself — no `AppHandle`/keychain, so it's
    /// directly testable against a `wiremock::MockServer` (see the tests),
    /// mirroring [`Self::endpoint_url`].
    pub(super) async fn list_models_transport(
        &self,
        api_key: Option<&str>,
    ) -> AppResult<Vec<Value>> {
        let resp = self
            .list_models_request(api_key)?
            .send()
            .await
            .map_err(|e| {
                AppError::Network(format!(
                    "{}: request failed: {}",
                    self.id.as_str(),
                    scrub_url_secret(e)
                ))
            })?;
        let status = resp.status();
        if !status.is_success() {
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            return Err(friendly_api_error(self.id, status, &body_text));
        }
        let body: Value =
            crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .map_err(|e| AppError::Provider(format!("{}: parse: {}", self.id.as_str(), e)))?;
        // OpenAI proper: only chat-capable families. Every other OpenAI-compatible
        // backend (incl. Ollama Cloud) lists its own curated catalog, so pass those
        // through unfiltered — see `should_list_model`.
        parse_model_list(self.id, &body)
    }

    /// Key-liveness probe body of `AiProvider::test_key` — moved out of the
    /// trait method so it stays a thin delegator.
    pub(super) async fn test_key_impl(&self, app: &AppHandle) -> AppResult<()> {
        let api_key = resolve_openai_key(self.id, get_provider_key(app, self.id.credential_key()))?;
        let resp = self
            .list_models_request(api_key.as_deref())?
            .send()
            .await
            .map_err(|e| {
                AppError::Network(format!(
                    "{}: request failed: {}",
                    self.id.as_str(),
                    scrub_url_secret(e)
                ))
            })?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            Err(friendly_api_error(self.id, status, &body_text))
        }
    }
}
