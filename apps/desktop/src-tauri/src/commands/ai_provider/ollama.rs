//! Ollama (local) provider.
//!
//! This is the ONLY module allowed to reference the Ollama host or its `/api/*`
//! endpoints. Everything Ollama-specific — chat, model list, pull, embeddings,
//! health — lives here so no hidden Ollama assumptions leak into the rest of the
//! codebase.

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;
use crate::error::{AppError, AppResult};
use crate::events::{emit_event, JobEvent, JOBS_EVENT};

use super::research::{self, SearchResult};
use super::stream::{stream_response, StreamPiece};
use super::timeouts;
use super::{
    single_shot_turn, AgentTurn, AiGenerateRequest, AiProvider, ChatMsg, ModelCapabilities,
    ProviderId, RequestTrace, StopReason, TokenParam, ToolCall, ToolSpec, Usage,
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

/// Whether a local Ollama model advertises tool-calling. Ollama's `/api/chat`
/// only honors a `tools` field on models trained for it; a model without support
/// silently ignores tools (no error, but also no calls), so gate on a conservative
/// allowlist of the known tool-calling families. Unknown names default to `false`,
/// so an agent turn degrades to a single-shot answer instead of a call-less stall.
fn ollama_supports_tools(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.contains("llama3.1")
        || m.contains("llama3.2")
        || m.contains("llama3.3")
        || m.contains("qwen2.5")
        || m.contains("qwen3")
        || m.contains("mistral")
        || m.contains("mixtral")
        || m.contains("command-r")
        || m.contains("firefunction")
        || m.contains("hermes")
        || m.contains("granite")
}

/// Parse a non-streaming `/api/chat` response into an [`AgentTurn`]:
/// `message.content` is the text, each `message.tool_calls[]` maps to a
/// [`ToolCall`] (Ollama returns `function.arguments` as an already-decoded JSON
/// object, and no call id — synthesize `name-index`), and `done_reason` maps the
/// stop (`length`→Length even with tool calls present — the arguments may be
/// truncated JSON, so length wins over the tool-call signal; else any tool call ⇒
/// ToolUse, else End). Pure + unit-tested.
fn parse_ollama_turn(data: &Value) -> AgentTurn {
    let message = data.get("message");
    let text = message
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or_default()
        .to_string();
    let tool_calls: Vec<ToolCall> = message
        .and_then(|m| m.get("tool_calls"))
        .and_then(|c| c.as_array())
        .map(|calls| {
            calls
                .iter()
                .enumerate()
                .filter_map(|(i, c)| {
                    let func = c.get("function")?;
                    let name = func.get("name").and_then(|n| n.as_str())?.to_string();
                    let args = func.get("arguments").cloned().unwrap_or_else(|| json!({}));
                    Some(ToolCall {
                        id: format!("{name}-{i}"),
                        name,
                        args,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let stop = if data.get("done_reason").and_then(|r| r.as_str()) == Some("length") {
        // A length-truncated turn's tool-call arguments may be truncated /
        // half-serialized JSON — length must win over the tool-call signal.
        StopReason::Length
    } else if !tool_calls.is_empty() {
        StopReason::ToolUse
    } else {
        StopReason::End
    };
    AgentTurn {
        text,
        tool_calls,
        stop,
        usage: parse_ollama_usage(data).unwrap_or_default(),
    }
}

/// Extract `prompt_eval_count`/`eval_count` — Ollama's real input/output token
/// counts, present at the top level of both the non-streaming `/api/chat`
/// response and the final (`done: true`) streamed object. `None` when NEITHER
/// field is present (an absent/malformed response never fabricates a zero
/// that looks like a real reported value). Pure + unit-tested.
fn parse_ollama_usage(data: &Value) -> Option<Usage> {
    if data.get("prompt_eval_count").is_none() && data.get("eval_count").is_none() {
        return None;
    }
    Some(Usage {
        input_tokens: data
            .get("prompt_eval_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: data.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
    })
}

// ── Provider impl ───────────────────────────────────────────────────────────────

pub struct OllamaClient;

#[async_trait]
impl AiProvider for OllamaClient {
    fn id(&self) -> ProviderId {
        ProviderId::Ollama
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        ModelCapabilities {
            supports_temperature: true,
            supports_system_role: true,
            supports_streaming: true,
            supports_reasoning: false,
            // Per-model: only tool-calling families advertise it (see the allowlist);
            // unknown models stay `false` so an agent turn degrades safely.
            supports_tools: ollama_supports_tools(model),
            supports_json_mode: true,
            supports_embeddings: true,
            // Attempts research via the Ollama Web Search API (account-key
            // gated at call time, not statically known here).
            supports_web_search: true,
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
        complete_impl(model, system, user, temperature)
            .await
            .map(|(text, _)| text)
    }

    async fn complete_with_usage(
        &self,
        _app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
        complete_impl(model, system, user, temperature).await
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

    #[allow(clippy::too_many_arguments)]
    async fn research_salary(
        &self,
        app: &AppHandle,
        model: &str,
        role: &str,
        company: &str,
        location: &str,
        country: &str,
        currency: &str,
    ) -> AppResult<String> {
        ollama_research_salary(app, self, model, role, company, location, country, currency).await
    }

    async fn research_answer(
        &self,
        app: &AppHandle,
        model: &str,
        question: &str,
        role: &str,
        company: &str,
    ) -> AppResult<String> {
        ollama_research_answer(app, self, model, question, role, company).await
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
            .timeout(timeouts::LIST_MODELS)
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

    async fn chat_with_tools(
        &self,
        app: &AppHandle,
        model: &str,
        messages: &[ChatMsg],
        tools: &[ToolSpec],
        temperature: Option<f64>,
    ) -> AppResult<AgentTurn> {
        // Only tool-capable local models attempt native tool-calling; the rest
        // degrade to a single-shot answer.
        if !self.capabilities(model).supports_tools {
            return single_shot_turn(self, app, model, messages, temperature).await;
        }
        let base = host();
        let endpoint = format!("{base}/api/chat");
        let trace = RequestTrace::begin(ProviderId::Ollama, model, "/api/chat tools", &base, false);

        let wire_messages: Vec<Value> = messages
            .iter()
            .map(|m| json!({ "role": m.role.wire(), "content": m.content }))
            .collect();
        let tool_specs: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.schema,
                    },
                })
            })
            .collect();

        let mut body = json!({
            "model": model,
            "stream": false,
            "messages": wire_messages,
            "tools": tool_specs,
        });
        if let Some(t) = temperature {
            body["options"] = json!({ "temperature": t });
        }
        body["keep_alive"] = json!(crate::performance::ollama_keep_alive());

        let resp = match super::retry::send_with_retry(|| {
            crate::net::http::shared()
                .post(&endpoint)
                .timeout(timeouts::OLLAMA_COMPLETION)
                .json(&body)
        })
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
        Ok(parse_ollama_turn(&data))
    }
}

// ── Shared Ollama helpers (used by the AI commands, health, embeddings) ─────────

/// `{ name }` list from `/api/tags`.
pub async fn list_tag_models() -> Vec<Value> {
    let resp = match crate::net::http::shared()
        .get(format!("{}/api/tags", host()))
        .timeout(timeouts::LIST_MODELS)
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
        .timeout(timeouts::HEALTH)
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
    // Defensive char-boundary-safe cap for any direct caller (avoids panics on
    // multi-byte input). The shared `embed_text` path already caps to the provider's
    // `max_embedding_input_chars` (8000 for Ollama), so this is a no-op there.
    let truncated: String = text.chars().take(8000).collect();
    let body = json!({ "model": model, "prompt": truncated, "keep_alive": crate::performance::ollama_keep_alive() });
    let endpoint = format!("{}/api/embeddings", host());
    let resp = super::retry::send_with_retry(|| {
        crate::net::http::shared()
            .post(&endpoint)
            .timeout(timeouts::OLLAMA_EMBED)
            .json(&body)
    })
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
        .timeout(timeouts::OLLAMA_WEB_SEARCH)
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

/// Shared search step for every Ollama-family research facet: resolve the
/// account key and run the Ollama Web Search API. Returns an empty `Vec` when
/// the key is missing or the search fails, so callers degrade to `""` without
/// each re-implementing the key-check + trace boilerplate.
async fn ollama_search(app: &AppHandle, model: &str, query: &str) -> Vec<SearchResult> {
    let key = get_provider_key(app, ACCOUNT_KEY).unwrap_or_default();
    if key.trim().is_empty() {
        return Vec::new();
    }
    let trace = RequestTrace::begin(
        ProviderId::OllamaCloud,
        model,
        "/api/web_search",
        "https://ollama.com",
        false,
    );
    match ollama_web_search(&key, query, 5).await {
        Ok(r) => {
            trace.end(Some(200), true);
            r
        }
        Err(e) => {
            trace.end(None, false);
            tracing::warn!("ollama web_search failed: {e}");
            Vec::new()
        }
    }
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
    let results = ollama_search(app, model, &research::search_query(company)).await;
    if results.is_empty() {
        return Ok(String::new());
    }
    let user = research::synth_user(company, role, &results);
    provider
        .complete(app, model, research::SYNTH_SYSTEM, &user, Some(0.2))
        .await
}

/// Salary-range sibling of [`ollama_research`]: same search-then-synthesize
/// shape, but the salary query/prompts (compact JSON contract, see
/// `research::salary_system`). Returns `""` when the key is missing or the
/// search yields nothing, so the lookup degrades gracefully. `country`/
/// `currency` ground the report in the job's actual currency — see
/// `AiProvider::research_salary`.
#[allow(clippy::too_many_arguments)]
pub async fn ollama_research_salary(
    app: &AppHandle,
    provider: &dyn AiProvider,
    model: &str,
    role: &str,
    company: &str,
    location: &str,
    country: &str,
    currency: &str,
) -> AppResult<String> {
    let query = research::salary_search_query(role, company, location, country, currency);
    let results = ollama_search(app, model, &query).await;
    if results.is_empty() {
        return Ok(String::new());
    }
    let user = research::salary_synth_user(role, company, location, country, currency, &results);
    provider
        .complete(
            app,
            model,
            &research::salary_system(currency),
            &user,
            Some(0.2),
        )
        .await
}

/// Application-answer sibling of [`ollama_research`]: same search-then-
/// synthesize shape, scoped to a single application question (see
/// `research::answer_search_query`) instead of a general company brief.
/// Returns `""` when the key is missing or the search yields nothing, so the
/// lookup degrades gracefully.
pub async fn ollama_research_answer(
    app: &AppHandle,
    provider: &dyn AiProvider,
    model: &str,
    question: &str,
    role: &str,
    company: &str,
) -> AppResult<String> {
    let query = research::answer_search_query(question, role, company);
    let results = ollama_search(app, model, &query).await;
    if results.is_empty() {
        return Ok(String::new());
    }
    let user = research::answer_synth_user(question, role, company, &results);
    provider
        .complete(app, model, research::ANSWER_SYNTH_SYSTEM, &user, Some(0.2))
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
        .timeout(timeouts::OLLAMA_SHOW)
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
        .timeout(timeouts::MODEL_PULL)
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
        // Walk by a `consumed` offset and drain the parsed prefix once after the
        // inner loop, instead of reallocating the whole tail per line (O(n²)).
        let mut consumed = 0;
        while let Some(rel) = line_buf[consumed..].find('\n') {
            let nl = consumed + rel;
            let line = line_buf[consumed..nl].trim().to_string();
            consumed = nl + 1;
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
            emit_event(
                app,
                JOBS_EVENT,
                JobEvent {
                    r#type: "job.stream".to_string(),
                    job_id: job_id.to_string(),
                    data: Some(
                        json!({ "status": status, "p": p, "completed": completed, "total": total, "digest": digest }),
                    ),
                    ts: crate::db::now_ms() as i64,
                },
            );
            if status == "success" {
                return Ok(());
            }
        }
        // Drop the fully-parsed prefix once; the partial trailing line stays buffered.
        line_buf.drain(..consumed);
    }
    Ok(())
}

// ── Chat streaming ──────────────────────────────────────────────────────────────

/// Drain complete newline-delimited JSON objects from the accumulated stream
/// buffer into [`StreamPiece`]s, leaving any partial trailing line for the next
/// chunk. Each object carries an optional `message.thinking` (structured
/// reasoning from DeepSeek-R1/Qwen3) and `message.content` answer text; the
/// object with `done: true` yields a terminal sentinel carrying the final content
/// delta. Pure + unit-tested; this is Ollama's `parse` closure, so its NDJSON
/// framing lives here only.
fn parse_ollama_frames(buf: &mut String) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    // Walk by a `consumed` offset and `drain(..consumed)` once at the end, instead
    // of reallocating the whole tail per line (O(n²) on a big frame).
    let mut consumed = 0;
    while let Some(rel) = buf[consumed..].find('\n') {
        let nl = consumed + rel;
        let line = buf[consumed..nl].trim().to_string();
        consumed = nl + 1;
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
        let thinking = message
            .and_then(|m| m.get("thinking"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let done = event.get("done").and_then(|d| d.as_bool()).unwrap_or(false);
        if !thinking.is_empty() {
            out.push(StreamPiece::thinking(thinking));
        }
        if done {
            // Final object — carry its content on the terminal sentinel so the
            // shared loop emits it then completes (matches the original framing).
            // Real token usage (`prompt_eval_count`/`eval_count`) lives on this
            // same final object, so it rides along on the sentinel piece too.
            buf.drain(..consumed);
            let mut sentinel = StreamPiece::done(delta);
            sentinel.usage = parse_ollama_usage(&event);
            out.push(sentinel);
            return out;
        }
        if !delta.is_empty() {
            out.push(StreamPiece::text(delta));
        }
    }
    // Drop the fully-parsed prefix once; the partial trailing line stays buffered.
    buf.drain(..consumed);
    out
}

/// Build the `/api/chat` streaming request body for a given [`AiGenerateRequest`].
/// Pure + unit-tested. `options.top_p`/`options.repeat_penalty` are the
/// detector-resistance sampling knobs (RAID, ACL 2024) the renderer sets only
/// for prose generation surfaces, added only when `Some` (never sent as
/// `null`). `repeat_penalty` uses Ollama's own field/semantics — it is NEVER a
/// remap of `frequency_penalty` (different math, different field).
fn build_chat_stream_body(req: &AiGenerateRequest) -> Value {
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
    if let Some(top_p) = req.top_p {
        options.insert("top_p".to_string(), json!(top_p));
    }
    if let Some(rp) = req.repeat_penalty {
        options.insert("repeat_penalty".to_string(), json!(rp));
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
    body["keep_alive"] = json!(crate::performance::ollama_keep_alive());
    body
}

async fn stream_chat(app: &AppHandle, job_id: &str, req: &AiGenerateRequest) -> AppResult<()> {
    let base = host();
    let endpoint = format!("{base}/api/chat");
    let trace = RequestTrace::begin(ProviderId::Ollama, &req.model, "/api/chat", &base, true);

    let body = build_chat_stream_body(req);

    let response = crate::net::http::shared()
        .post(&endpoint)
        .timeout(timeouts::STREAM)
        .json(&body)
        .send()
        .await;

    let response = match response {
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

    // The shared loop owns cancel-check + chunk read + emit + complete; the closure
    // is the only Ollama-specific part (newline-delimited JSON framing). Structured
    // reasoning from thinking models rides on `message.thinking`; models that embed
    // <think>…</think> in `content` are split renderer-side. We do not force Ollama's
    // `think` flag here — it 400s on non-thinking models.
    stream_response(
        app,
        job_id,
        &trace,
        response,
        status.as_u16(),
        ProviderId::Ollama,
        &req.model,
        &base,
        parse_ollama_frames,
    )
    .await
}

/// Shared body of [`AiProvider::complete`]/[`AiProvider::complete_with_usage`]
/// for [`OllamaClient`]: one non-streaming `/api/chat` call, parsed once into
/// `(text, usage)` so the two trait methods never duplicate the HTTP
/// round-trip. A free function (no `&self`) since `OllamaClient` is a unit
/// struct with no other state.
async fn complete_impl(
    model: &str,
    system: &str,
    user: &str,
    temperature: Option<f64>,
) -> AppResult<(String, Usage)> {
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
    body["keep_alive"] = json!(crate::performance::ollama_keep_alive());

    let resp = match super::retry::send_with_retry(|| {
        crate::net::http::shared()
            .post(&endpoint)
            .timeout(timeouts::OLLAMA_COMPLETION)
            .json(&body)
    })
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
    let text = data
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(String::from)
        .ok_or_else(|| AppError::Provider("Ollama: unexpected response shape".to_string()))?;
    Ok((text, parse_ollama_usage(&data).unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::{
        build_chat_stream_body, normalize_show, ollama_supports_tools, parse_ollama_frames,
        parse_ollama_turn, parse_ollama_usage, parse_web_search, StreamPiece,
    };
    use crate::commands::ai_provider::{AiGenerateRequest, StopReason, ToolCall};
    use crate::ipc_contracts::ai::AiGenerateRequestMessage;
    use serde_json::json;

    fn base_request() -> AiGenerateRequest {
        AiGenerateRequest {
            model: "llama3.1:8b".to_string(),
            messages: vec![AiGenerateRequestMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
            }],
            locale: "en".to_string(),
            temperature: Some(0.8),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            repeat_penalty: None,
            max_tokens: None,
            context_window: None,
            provider: None,
            base_url: None,
            effort: None,
        }
    }

    #[test]
    fn chat_stream_body_serializes_top_p_and_repeat_penalty_when_set() {
        let mut req = base_request();
        req.top_p = Some(0.95);
        req.repeat_penalty = Some(1.15);
        let body = build_chat_stream_body(&req);
        assert_eq!(body["options"]["top_p"], json!(0.95));
        assert_eq!(body["options"]["repeat_penalty"], json!(1.15));
        // frequency_penalty is never remapped into Ollama's repeat_penalty field.
        assert!(body["options"].get("frequency_penalty").is_none());
    }

    #[test]
    fn chat_stream_body_omits_sampling_options_when_none() {
        let body = build_chat_stream_body(&base_request());
        assert!(body["options"].get("top_p").is_none());
        assert!(body["options"].get("repeat_penalty").is_none());
    }

    #[test]
    fn parse_ollama_frames_splits_thinking_and_content() {
        let mut buf = String::from(
            "{\"message\":{\"thinking\":\"hmm\"},\"done\":false}\n\
             {\"message\":{\"content\":\"hello\"},\"done\":false}\n",
        );
        let pieces = parse_ollama_frames(&mut buf);
        assert_eq!(
            pieces,
            vec![StreamPiece::thinking("hmm"), StreamPiece::text("hello")]
        );
        assert!(buf.is_empty());
    }

    #[test]
    fn parse_ollama_frames_done_carries_final_content() {
        // The `done:true` object becomes a sentinel carrying its final content.
        let mut buf = String::from("{\"message\":{\"content\":\"end\"},\"done\":true}\n");
        assert_eq!(
            parse_ollama_frames(&mut buf),
            vec![StreamPiece::done("end")]
        );
    }

    #[test]
    fn parse_ollama_frames_done_carries_real_token_usage() {
        let mut buf = String::from(
            "{\"message\":{\"content\":\"end\"},\"done\":true,\"prompt_eval_count\":123,\"eval_count\":45}\n",
        );
        let pieces = parse_ollama_frames(&mut buf);
        assert_eq!(pieces.len(), 1);
        let usage = pieces[0].usage.expect("done piece must carry usage");
        assert_eq!(usage.input_tokens, 123);
        assert_eq!(usage.output_tokens, 45);
    }

    #[test]
    fn parse_usage_reads_prompt_eval_and_eval_counts() {
        let data = json!({ "prompt_eval_count": 10, "eval_count": 20 });
        let usage = parse_ollama_usage(&data).expect("usage present");
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 20);
    }

    #[test]
    fn parse_usage_is_none_when_absent() {
        assert!(parse_ollama_usage(&json!({})).is_none());
    }

    #[test]
    fn parse_ollama_frames_buffers_partial_line() {
        // A partial trailing JSON line is left for the next chunk.
        let mut buf = String::from("{\"message\":{\"content\":\"hi\"},\"done\":false}\n{\"mess");
        let pieces = parse_ollama_frames(&mut buf);
        assert_eq!(pieces, vec![StreamPiece::text("hi")]);
        assert_eq!(buf, "{\"mess");
    }

    #[test]
    fn parse_ollama_frames_skips_blank_and_unparseable_lines() {
        let mut buf = String::from("\nnot-json\n");
        assert!(parse_ollama_frames(&mut buf).is_empty());
    }

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

    #[test]
    fn tool_support_gate_is_conservative() {
        for m in [
            "llama3.1:8b",
            "llama3.3:70b",
            "qwen2.5:7b",
            "mistral-nemo",
            "command-r-plus",
        ] {
            assert!(ollama_supports_tools(m), "{m} should advertise tools");
        }
        // Unknown / non-tool families default off so the turn degrades safely.
        for m in [
            "llama2",
            "phi3",
            "gemma2",
            "nomic-embed-text",
            "deepseek-coder",
        ] {
            assert!(!ollama_supports_tools(m), "{m} must default to no tools");
        }
    }

    #[test]
    fn parse_turn_reads_object_arguments_and_content() {
        // Ollama returns arguments as an already-decoded object (NOT a JSON string).
        let data = json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{ "function": { "name": "match_resume", "arguments": { "resumeId": "r1", "jobId": "j1" } } }]
            },
            "done": true,
            "done_reason": "stop"
        });
        let turn = parse_ollama_turn(&data);
        assert_eq!(turn.stop, StopReason::ToolUse);
        assert_eq!(
            turn.tool_calls,
            vec![ToolCall {
                id: "match_resume-0".to_string(),
                name: "match_resume".to_string(),
                args: json!({ "resumeId": "r1", "jobId": "j1" }),
            }]
        );
    }

    #[test]
    fn parse_turn_plain_answer_has_no_tool_calls() {
        let data = json!({
            "message": { "role": "assistant", "content": "The answer." },
            "done": true,
            "done_reason": "stop"
        });
        let turn = parse_ollama_turn(&data);
        assert_eq!(turn.text, "The answer.");
        assert!(turn.tool_calls.is_empty());
        assert_eq!(turn.stop, StopReason::End);
    }

    #[test]
    fn parse_turn_tool_calls_with_length_done_reason_maps_to_length_not_tool_use() {
        // `done_reason: "length"` means the arguments may be truncated JSON — this
        // must win over the tool-call signal, never `ToolUse`.
        let data = json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{ "function": { "name": "match_resume", "arguments": { "resumeId": "r1" } } }]
            },
            "done": true,
            "done_reason": "length"
        });
        let turn = parse_ollama_turn(&data);
        assert_eq!(turn.stop, StopReason::Length);
    }
}
