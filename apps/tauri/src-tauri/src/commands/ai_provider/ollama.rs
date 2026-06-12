//! Ollama (local) provider.
//!
//! This is the ONLY module allowed to reference the Ollama host or its `/api/*`
//! endpoints. Everything Ollama-specific — chat, model list, pull, embeddings,
//! health — lives here so no hidden Ollama assumptions leak into the rest of the
//! codebase.

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::commands::ai::get_provider_key;
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, AI_STREAM, JOBS_EVENT};
use crate::jobs::JobTracker;

use super::research::{self, SearchResult};
use super::{
    AiGenerateRequest, AiProvider, ModelCapabilities, ProviderId, RequestTrace, TokenParam,
};

const EMBED_MODEL: &str = "nomic-embed-text";
/// Ollama's first-party Web Search API (cloud) — authenticated with the Ollama
/// account key (`ai:ollama-cloud`), independent of the local daemon host.
const WEB_SEARCH_URL: &str = "https://ollama.com/api/web_search";
/// Credential slot for the Ollama account key shared by Ollama Cloud chat and
/// Ollama Web Search. Local Ollama has no chat key but still needs this to search.
pub const ACCOUNT_KEY: &str = "ollama-cloud";

/// Resolve the Ollama host (env override or localhost default).
pub fn host() -> String {
    crate::platform::config::ollama_host()
}

// ── Provider impl ───────────────────────────────────────────────────────────────

pub struct OllamaClient;

#[async_trait]
impl AiProvider for OllamaClient {
    fn id(&self) -> ProviderId {
        ProviderId::Ollama
    }

    fn capabilities(&self, _model: &str) -> ModelCapabilities {
        ModelCapabilities {
            supports_temperature: true,
            supports_system_role: true,
            supports_streaming: true,
            supports_reasoning: false,
            supports_tools: false,
            supports_json_mode: true,
            supports_embeddings: true,
            token_param: TokenParam::NumPredict,
        }
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        stream_chat(app, job_id, req).await
    }

    async fn complete(
        &self,
        _app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<String> {
        let base = host();
        let endpoint = format!("{base}/api/chat");
        let trace = RequestTrace::begin(ProviderId::Ollama, model, "/api/chat", &base, false);

        let mut body = json!({
            "model": model,
            "stream": false,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
        });
        if let Some(t) = temperature {
            body["options"] = json!({ "temperature": t });
        }

        let resp = match crate::net::http::shared()
            .post(&endpoint)
            .timeout(std::time::Duration::from_secs(300))
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Ollama unreachable: {e}")));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(AppError::Provider(format!("Ollama {status}: {body_text}")));
        }
        let data: Value = resp
            .json()
            .await
            .map_err(|e| format!("Ollama parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        data.get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(String::from)
            .ok_or_else(|| AppError::Provider("Ollama: unexpected response shape".to_string()))
    }

    async fn research(
        &self,
        app: &AppHandle,
        model: &str,
        company: &str,
        role: &str,
    ) -> AppResult<String> {
        // Local Ollama can't search itself — it uses the Ollama Web Search API
        // (needs the account key) then synthesizes via the local model.
        ollama_research(app, self, model, company, role).await
    }

    async fn embed(&self, _app: &AppHandle, model: &str, text: &str) -> AppResult<Vec<f64>> {
        embed_with(model, text).await
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        Some(EMBED_MODEL)
    }

    async fn list_models(&self, _app: &AppHandle) -> Vec<Value> {
        list_tag_models().await
    }

    async fn test_key(&self, _app: &AppHandle) -> AppResult<()> {
        // Ollama needs no key — a reachable host counts as healthy.
        let client = crate::net::http::shared();
        match client
            .get(format!("{}/api/tags", host()))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) => Err(AppError::Provider(format!(
                "Ollama returned status: {}",
                r.status()
            ))),
            Err(e) => Err(AppError::Network(format!("Ollama unreachable: {e}"))),
        }
    }
}

// ── Shared Ollama helpers (used by the AI commands, health, embeddings) ─────────

/// `{ name }` list from `/api/tags`.
pub async fn list_tag_models() -> Vec<Value> {
    let resp = match crate::net::http::shared()
        .get(format!("{}/api/tags", host()))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        _ => return vec![],
    };
    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    body.get("models")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                .map(|name| json!({ "name": name }))
                .collect()
        })
        .unwrap_or_default()
}

/// `(reachable, first_model_name)` for the system health probe.
pub async fn reachable_model() -> (bool, Option<String>) {
    match crate::net::http::shared()
        .get(format!("{}/api/tags", host()))
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => {
            let body: Value = r.json().await.unwrap_or_default();
            let model = body
                .get("models")
                .and_then(|m| m.as_array())
                .and_then(|arr| arr.first())
                .and_then(|m| m.get("name"))
                .and_then(|n| n.as_str())
                .map(String::from);
            (true, model)
        }
        _ => (false, None),
    }
}

/// Embed `text` with a specific Ollama embedding model. Returns a clear error
/// (not `None`) so callers can surface why embedding failed.
pub async fn embed_with(model: &str, text: &str) -> AppResult<Vec<f64>> {
    // Char-boundary-safe truncation (avoids panics on multi-byte input).
    let truncated: String = text.chars().take(8000).collect();
    let body = json!({ "model": model, "prompt": truncated });
    let resp = crate::net::http::shared()
        .post(format!("{}/api/embeddings", host()))
        .timeout(std::time::Duration::from_secs(15))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(AppError::Provider(format!("Ollama {status}: {body_text}")));
    }
    let data: Value = resp
        .json()
        .await
        .map_err(|e| format!("Ollama parse: {e}"))?;
    let arr = data
        .get("embedding")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "Ollama: missing embedding in response".to_string())?;
    Ok(arr.iter().filter_map(|v| v.as_f64()).collect())
}

// ── Company research (Ollama Web Search) ────────────────────────────────────────

/// Call Ollama's Web Search API and return up to `limit` result snippets. `key`
/// is the Ollama account key (`ai:ollama-cloud`). Pure transport — any error is
/// surfaced for the caller to swallow, so a missing/invalid key never breaks
/// generation.
pub async fn ollama_web_search(
    key: &str,
    query: &str,
    limit: usize,
) -> AppResult<Vec<SearchResult>> {
    let resp = crate::net::http::shared()
        .post(WEB_SEARCH_URL)
        .timeout(std::time::Duration::from_secs(15))
        .bearer_auth(key)
        .json(&json!({ "query": query, "max_results": limit.min(10) }))
        .send()
        .await
        .map_err(|e| format!("ollama web_search request: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Network(format!(
            "ollama web_search {status}: {body}"
        )));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("ollama web_search parse: {e}"))?;
    Ok(parse_web_search(&body, limit))
}

/// Map an Ollama `web_search` response (`{ results: [{title,url,content}] }`) to
/// `SearchResult`. Pure + unit-tested.
fn parse_web_search(body: &Value, limit: usize) -> Vec<SearchResult> {
    body.get("results")
        .and_then(|r| r.as_array())
        .map(|arr| {
            arr.iter()
                .take(limit)
                .map(|item| SearchResult {
                    title: item
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string(),
                    snippet: item
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string(),
                    url: item
                        .get("url")
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Shared Ollama-family research: search via the Ollama Web Search API (account
/// key), then synthesize the brief with `provider` — the local daemon for
/// [`OllamaClient`], `ollama.com/v1` for Ollama Cloud. Returns `""` when the key
/// is missing or the search yields nothing, so research degrades gracefully.
pub async fn ollama_research(
    app: &AppHandle,
    provider: &dyn AiProvider,
    model: &str,
    company: &str,
    role: &str,
) -> AppResult<String> {
    let key = get_provider_key(app, ACCOUNT_KEY).unwrap_or_default();
    if key.trim().is_empty() {
        return Ok(String::new());
    }
    let trace = RequestTrace::begin(
        ProviderId::OllamaCloud,
        model,
        "/api/web_search",
        "https://ollama.com",
        false,
    );
    let results = match ollama_web_search(&key, &research::search_query(company), 5).await {
        Ok(r) => {
            trace.end(Some(200), true);
            r
        }
        Err(e) => {
            trace.end(None, false);
            tracing::warn!("ollama web_search failed: {e}");
            return Ok(String::new());
        }
    };
    if results.is_empty() {
        return Ok(String::new());
    }
    let user = research::synth_user(company, role, &results);
    provider
        .complete(app, model, research::SYNTH_SYSTEM, &user, Some(0.2))
        .await
}

/// Inspect a local model via `/api/show` — its real trained context length and
/// size labels — normalized to the `ModelInspectResult` shape. Returns
/// `Value::Null` when Ollama is unreachable, errors, or returns nothing useful,
/// so the caller can surface "no info" without failing.
pub async fn show_model(model: &str) -> Value {
    let body = json!({ "model": model });
    let resp = match crate::net::http::shared()
        .post(format!("{}/api/show", host()))
        .timeout(std::time::Duration::from_secs(15))
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return Value::Null,
    };
    if !resp.status().is_success() {
        return Value::Null;
    }
    match resp.json::<Value>().await {
        Ok(data) => normalize_show(&data),
        Err(_) => Value::Null,
    }
}

/// Map an Ollama `/api/show` response to the `ModelInspectResult` shape
/// (camelCase keys), omitting fields the server didn't provide. Pure +
/// unit-tested. `model_info.*.context_length` is keyed by architecture (e.g.
/// `llama.context_length`, `qwen2.context_length`), so we scan for the first key
/// ending in `.context_length` rather than hardcoding an architecture. Returns
/// `Value::Null` when nothing usable is present.
fn normalize_show(data: &Value) -> Value {
    let context_length = data
        .get("model_info")
        .and_then(|mi| mi.as_object())
        .and_then(|obj| {
            obj.iter()
                .find(|(k, _)| k.ends_with(".context_length"))
                .and_then(|(_, v)| v.as_u64())
        });
    let details = data.get("details");
    let str_field = |key: &str| -> Option<String> {
        details
            .and_then(|d| d.get(key))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };

    let mut out = serde_json::Map::new();
    if let Some(c) = context_length {
        out.insert("contextLength".to_string(), json!(c));
    }
    if let Some(p) = str_field("parameter_size") {
        out.insert("parameterSize".to_string(), json!(p));
    }
    if let Some(q) = str_field("quantization_level") {
        out.insert("quantization".to_string(), json!(q));
    }
    if let Some(f) = str_field("family") {
        out.insert("family".to_string(), json!(f));
    }

    if out.is_empty() {
        Value::Null
    } else {
        Value::Object(out)
    }
}

/// Stream a model pull, emitting `jobs:event` progress. Returns when complete.
pub async fn pull(app: &AppHandle, job_id: &str, model: &str) -> AppResult<()> {
    let mut response = crate::net::http::shared()
        .post(format!("{}/api/pull", host()))
        .timeout(std::time::Duration::from_secs(3600))
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Provider(format!("Ollama {status}: {body}")));
    }

    let mut line_buf = String::new();
    while let Some(bytes) = response.chunk().await.map_err(|e| e.to_string())? {
        line_buf.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(nl) = line_buf.find('\n') {
            let line = line_buf[..nl].trim().to_string();
            line_buf = line_buf[nl + 1..].to_string();
            if line.is_empty() {
                continue;
            }
            let event: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let status = event.get("status").and_then(|s| s.as_str()).unwrap_or("");
            let completed = event
                .get("completed")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let total = event.get("total").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let digest = event.get("digest").and_then(|v| v.as_str()).unwrap_or("");
            let p = if total > 0.0 { completed / total } else { 0.0 };
            emit_event(app, JOBS_EVENT, json!({ "type": "job.stream", "jobId": job_id, "data": { "status": status, "p": p, "completed": completed, "total": total, "digest": digest } }));
            if status == "success" {
                return Ok(());
            }
        }
    }
    Ok(())
}

// ── Chat streaming ──────────────────────────────────────────────────────────────

async fn stream_chat(app: &AppHandle, job_id: &str, req: &AiGenerateRequest) -> AppResult<()> {
    let base = host();
    let endpoint = format!("{base}/api/chat");
    let trace = RequestTrace::begin(ProviderId::Ollama, &req.model, "/api/chat", &base, true);

    let messages = serde_json::to_value(
        req.messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect::<Vec<_>>(),
    )
    .unwrap_or(json!([]));

    let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
    let mut options = serde_json::Map::new();
    if let Some(t) = req.temperature {
        options.insert("temperature".to_string(), json!(t));
    }
    if let Some(mt) = req.max_tokens {
        options.insert("num_predict".to_string(), json!(mt));
    }
    // Context window (num_ctx) — large résumé/job-ad prompts overflow Ollama's
    // small default context and get silently truncated without this.
    if let Some(ctx) = req.context_window {
        options.insert("num_ctx".to_string(), json!(ctx));
    }
    if !options.is_empty() {
        body["options"] = Value::Object(options);
    }

    let response = crate::net::http::shared()
        .post(&endpoint)
        .timeout(std::time::Duration::from_secs(300))
        .json(&body)
        .send()
        .await;

    let mut response = match response {
        Ok(r) => r,
        Err(e) => {
            trace.end(None, false);
            return Err(AppError::Network(format!("Ollama unreachable: {e}")));
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        trace.end(Some(status.as_u16()), false);
        return Err(AppError::Provider(format!("Ollama {status}: {body_text}")));
    }

    let mut line_buf = String::new();
    loop {
        if let Some(job) = app.state::<Mutex<JobTracker>>().lock().get(job_id) {
            if job.status == crate::jobs::JobStatus::Cancelled {
                let _ = response.error_for_status_ref();
                trace.end(Some(status.as_u16()), false);
                return Err(AppError::Message("Job cancelled".to_string()));
            }
        }

        match response.chunk().await {
            Ok(Some(bytes)) => {
                line_buf.push_str(&String::from_utf8_lossy(&bytes));
                while let Some(nl) = line_buf.find('\n') {
                    let line = line_buf[..nl].trim().to_string();
                    line_buf = line_buf[nl + 1..].to_string();
                    if line.is_empty() {
                        continue;
                    }
                    let event: Value = match serde_json::from_str(&line) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    let message = event.get("message");
                    let delta = message
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("");
                    // Structured reasoning from thinking models (DeepSeek-R1, Qwen3)
                    // when the server populates `message.thinking`. Models that instead
                    // embed <think>…</think> in `content` are split renderer-side. We do
                    // not force Ollama's `think` flag here — it 400s on non-thinking
                    // models, so capability-gated enablement rides with the /api/show
                    // work (Part A).
                    let thinking = message
                        .and_then(|m| m.get("thinking"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    let done = event.get("done").and_then(|d| d.as_bool()).unwrap_or(false);
                    if !thinking.is_empty() {
                        emit_event(
                            app,
                            AI_STREAM,
                            json!({ "jobId": job_id, "delta": thinking, "done": false, "thinking": true }),
                        );
                    }
                    emit_event(
                        app,
                        AI_STREAM,
                        json!({ "jobId": job_id, "delta": delta, "done": done }),
                    );
                    if done {
                        app.state::<Mutex<JobTracker>>()
                            .lock()
                            .complete(job_id, json!({ "done": true }));
                        trace.end(Some(status.as_u16()), true);
                        return Ok(());
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                trace.end(Some(status.as_u16()), false);
                return Err(AppError::Network(format!("Stream error: {e}")));
            }
        }
    }

    emit_event(
        app,
        AI_STREAM,
        json!({ "jobId": job_id, "delta": "", "done": true }),
    );
    app.state::<Mutex<JobTracker>>()
        .lock()
        .complete(job_id, json!({ "done": true }));
    trace.end(Some(status.as_u16()), true);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{normalize_show, parse_web_search};
    use serde_json::json;

    #[test]
    fn parse_web_search_maps_results_and_caps_limit() {
        let body = json!({
            "results": [
                { "title": "Acme — Wikipedia", "url": "https://w/a", "content": "Acme makes widgets." },
                { "title": "Acme careers", "url": "https://a/c", "content": "Series B." },
                { "title": "extra", "url": "https://x", "content": "ignored by limit" },
            ]
        });
        let out = parse_web_search(&body, 2);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].title, "Acme — Wikipedia");
        assert_eq!(out[0].snippet, "Acme makes widgets.");
        assert_eq!(out[1].url, "https://a/c");
    }

    #[test]
    fn parse_web_search_tolerates_missing_fields_and_no_results() {
        assert!(parse_web_search(&json!({}), 5).is_empty());
        let out = parse_web_search(&json!({ "results": [{}] }), 5);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].title, "");
        assert_eq!(out[0].snippet, "");
    }

    #[test]
    fn normalize_extracts_context_and_details_by_architecture() {
        // `context_length` is keyed by architecture — scan for the suffix, not a
        // hardcoded `llama.` prefix, so qwen2/phi3/etc. all work unchanged.
        let data = json!({
            "model_info": { "qwen2.context_length": 32768, "qwen2.embedding_length": 3584 },
            "details": { "parameter_size": "7.6B", "quantization_level": "Q4_K_M", "family": "qwen2" }
        });
        let out = normalize_show(&data);
        assert_eq!(out["contextLength"], json!(32768));
        assert_eq!(out["parameterSize"], json!("7.6B"));
        assert_eq!(out["quantization"], json!("Q4_K_M"));
        assert_eq!(out["family"], json!("qwen2"));
    }

    #[test]
    fn normalize_omits_missing_fields() {
        let data = json!({
            "model_info": { "llama.context_length": 8192 },
            "details": { "parameter_size": "8B" }
        });
        let out = normalize_show(&data);
        assert_eq!(out["contextLength"], json!(8192));
        assert_eq!(out["parameterSize"], json!("8B"));
        // Absent fields are omitted (not null), so the TS optional schema accepts it.
        assert!(out.get("quantization").is_none());
        assert!(out.get("family").is_none());
    }

    #[test]
    fn normalize_returns_null_when_nothing_usable() {
        assert!(normalize_show(&json!({})).is_null());
        assert!(normalize_show(&json!({ "model_info": {}, "details": {} })).is_null());
    }
}
