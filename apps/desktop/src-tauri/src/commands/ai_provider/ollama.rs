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
    model_entry, parse_rfc3339_millis, resolve_intent, single_shot_turn, AgentTurn,
    AiGenerateRequest, AiProvider, ChatMsg, Intent, ModelCapabilities, ProviderId, RequestTrace,
    SamplingProfile, StopReason, TokenParam, ToolCall, ToolSpec, Usage, DETERMINISTIC_TEMPERATURE,
    PROSE_GROUNDED_TEMPERATURE, PROSE_REPEAT_PENALTY, PROSE_TEMPERATURE, PROSE_TOP_P,
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

/// Ollama's family of thinking-capable models — shared by local Ollama's
/// native `/api/chat` `think` field (this file, [`build_chat_stream_body`])
/// and Ollama Cloud's OpenAI-compatible `reasoning_effort` field
/// (`openai.rs`, gated on `ProviderId::OllamaCloud`): both wire shapes gate
/// on the SAME model catalog, so the classifier lives once here rather than
/// drifting into two copies. Per Ollama's docs
/// (`docs/capabilities/thinking.mdx`, fetched 2026-08-03): Qwen 3, GPT-OSS,
/// DeepSeek-v3.1, and DeepSeek R1 are the currently-documented thinking
/// families. Unknown models default to `false` (a graceful miss — the
/// `think`/`reasoning_effort` field 400s on a non-thinking model, so
/// guessing wrong is never safe).
///
/// The `qwen3-coder` exclusion is scoped to the `qwen3` branch specifically
/// (not a blanket "any model containing `coder`" check) — `qwen3-coder`/
/// `qwen3-coder-plus` are separate, non-thinking models despite matching the
/// `qwen3` substring, but a hypothetical future thinking-capable coder model
/// in the gpt-oss/deepseek families must not be swept out by an unrelated
/// name collision.
pub(super) fn ollama_family_supports_thinking(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    if m.contains("qwen3") {
        return !m.contains("coder");
    }
    m.contains("gpt-oss") || m.contains("deepseek-r1") || m.contains("deepseek-v3.1")
}

/// Levels `think` accepts on every thinking-family model
/// ([`ollama_family_supports_thinking`]) — Ollama's `docs/capabilities/
/// thinking.mdx` (fetched 2026-08-03) documents one uniform `low`/`medium`/
/// `high` string enum, not a per-model-tier table like Gemini's
/// `thinkingLevel`. Still gated with `.contains(&effort)` on the send path
/// below, not just the family-membership boolean — the same protection
/// Gemini/Anthropic/OpenAI use (`effort` is stored PER PROVIDER, not per
/// model — `preferences-store.ts` — so a stale/unrecognized value must never
/// ship just because the CURRENT model happens to be in the thinking
/// family). A no-op today since every thinking-family model shares this same
/// set, but it keeps this call site correct with zero further change the day
/// a future model needs a narrower one.
const OLLAMA_EFFORT_LEVELS: [&str; 3] = ["low", "medium", "high"];

/// Ollama's own `done_reason`, from the final `/api/chat` object of EITHER the
/// streaming or the non-streaming path, mapped to a [`StopReason`]. `None` when
/// the field is absent (any frame that isn't the final one).
///
/// Shared by [`parse_ollama_turn`] and [`parse_ollama_frames`] on purpose: the
/// two paths read the same field off the same object shape, and a second
/// hand-written copy of this mapping is exactly the duplicated-heuristic defect
/// this codebase keeps re-learning. Callers layer their own precedence on top
/// (a turn lets `Length` outrank a tool call); this only reports what Ollama
/// said.
fn ollama_done_reason(data: &Value) -> Option<StopReason> {
    match data.get("done_reason").and_then(|r| r.as_str())? {
        "length" => Some(StopReason::Length),
        "stop" => Some(StopReason::End),
        // Open-typed on purpose — a future/unknown reason must not be silently
        // reported as a clean end.
        _ => Some(StopReason::Other),
    }
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
    let stop = if ollama_done_reason(data) == Some(StopReason::Length) {
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
            supports_reasoning: ollama_family_supports_thinking(model),
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

    fn effort_levels(&self, model: &str) -> Vec<&'static str> {
        if ollama_family_supports_thinking(model) {
            OLLAMA_EFFORT_LEVELS.to_vec()
        } else {
            Vec::new()
        }
    }

    /// Declares real values for every intent, on every model, no gating —
    /// unlike every cloud adapter, Ollama has no "this family 400s" split to
    /// fail-safe against, so there is no unknown-model case to stay neutral
    /// for. Omitting is NOT a safe default here: `/api/chat` falls back to
    /// the model's own Modelfile, a file this app cannot see or control at
    /// request time. Confirmed empirically against this machine's live local
    /// Ollama (not vendor docs): `qwen3.6:27b-q4_K_M`'s Modelfile defaults to
    /// `temperature: 1, presence_penalty: 1.5, top_p: 0.95`;
    /// `gemma4:31b-it-q4_K_M` defaults to `temperature: 1`. Both are
    /// currently-default-tier local models — a résumé/analysis JSON call
    /// omitting `temperature` on either would run at a creative-writing
    /// temperature (and, on the first, a presence-penalty HIGHER than
    /// anything this app ever sent), not a "sane default". Reuses the SAME
    /// per-intent targets every other accepting adapter does (this app's
    /// pre-fix renderer sent identical numbers to Ollama as every cloud
    /// provider) — `repeat_penalty` is Ollama's own field (see
    /// `build_chat_stream_body`'s doc comment), never a `frequency_penalty`
    /// remap, and this app has no way to forward `presence_penalty` to
    /// Ollama at all (its wire body has no such field — see
    /// `build_chat_stream_body`), so `Intent::ProseGrounded`'s
    /// presence-penalty distinction from `Intent::Prose` has no Ollama
    /// equivalent to withhold, exactly like Anthropic's — but the two
    /// intents still land on different temperatures here
    /// (`PROSE_TEMPERATURE` vs `PROSE_GROUNDED_TEMPERATURE` below), pinned
    /// by `ollama_tests.rs`'s
    /// `wire_body_prose_grounded_carries_a_lower_temperature_than_prose`.
    fn sampling_profile(&self, _model: &str, intent: Intent) -> SamplingProfile {
        match intent {
            // `Default` (no declared intent) resolves the same as
            // `Deterministic` — see `Intent`'s own doc comment
            // (`commands/ai_provider/mod.rs`).
            Intent::Deterministic | Intent::Default => SamplingProfile {
                temperature: Some(DETERMINISTIC_TEMPERATURE),
                ..SamplingProfile::default()
            },
            Intent::Prose => SamplingProfile {
                temperature: Some(PROSE_TEMPERATURE),
                top_p: Some(PROSE_TOP_P),
                repeat_penalty: Some(PROSE_REPEAT_PENALTY),
                ..SamplingProfile::default()
            },
            Intent::ProseGrounded => SamplingProfile {
                temperature: Some(PROSE_GROUNDED_TEMPERATURE),
                top_p: Some(PROSE_TOP_P),
                repeat_penalty: Some(PROSE_REPEAT_PENALTY),
                ..SamplingProfile::default()
            },
        }
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        let sampling = self
            .sampling_profile(&req.model, resolve_intent(req))
            .resolve(req);
        stream_chat(app, job_id, req, sampling).await
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

    async fn list_models(&self, _app: &AppHandle) -> AppResult<Vec<Value>> {
        fetch_tag_models().await
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
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(AppError::Provider(format!("Ollama {status}: {body_text}")));
        }
        let data: Value =
            crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .map_err(|e| format!("Ollama parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        Ok(parse_ollama_turn(&data))
    }
}

// ── Shared Ollama helpers (used by the AI commands, health, embeddings) ─────────

/// Parse the `/api/tags` response body into `{name, createdAt?}` entries.
/// Pure so it's unit-testable without a network mock.
///
/// `/api/tags` returns `modified_at` (RFC3339, possibly with a non-UTC
/// offset) — normalized to epoch millis via [`parse_rfc3339_millis`], the
/// convention every `createdAt` field in this codebase uses. Neither
/// `displayName` nor `contextLength` is ever populated: `/api/tags` reports
/// neither (a model's context length is only on `/api/show`, a different
/// endpoint this function doesn't call).
fn parse_model_list(body: &Value) -> AppResult<Vec<Value>> {
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
async fn fetch_tag_models() -> AppResult<Vec<Value>> {
    let resp = crate::net::http::shared()
        .get(format!("{}/api/tags", host()))
        .timeout(timeouts::LIST_MODELS)
        .send()
        .await
        .map_err(|e| AppError::Network(format!("Ollama unreachable: {e}")))?;
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

/// `(reachable, first_model_name)` for the system health probe.
pub async fn reachable_model() -> (bool, Option<String>) {
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
///
/// Deliberately sends `text` AS GIVEN — no internal length cap. This used to
/// `chars().take(8000)` here "defensively", reasoning that the shared
/// `embed_text` path already caps to `max_embedding_input_chars` (8000 for
/// Ollama) so it would be a no-op. That stopped being true once
/// `bounded_split_cap` (`embed.rs`) started deliberately GROWING a chunk past
/// 8000 for a document needing more than `MAX_CHUNKS_PER_DOCUMENT` chunks at
/// the nominal size — a grown ~9,375-char chunk arrived here and this cap
/// silently dropped its last ~1,375 chars, no error, every time: the exact
/// silent-truncation defect this whole adaptive-embedding feature exists to
/// fix, reintroduced one layer down. `embed_with` has exactly one caller
/// (`OllamaClient::embed`, reached only through `embed_adaptive`'s chunking),
/// so there is no direct caller to protect — length policy belongs entirely
/// to `embed_adaptive`'s chunk-and-halve ladder now: if Ollama's real request
/// limit is smaller than what's sent, it rejects with a real error
/// (`is_context_length_error` already recognizes Ollama's wording), which
/// `embed_chunk_adaptive` retries at a smaller size — the ladder doing
/// exactly what it was built for, instead of this cap quietly mutating the
/// caller's input first.
/// Build the `/api/embeddings` request body. Pure + unit-tested — the ONLY
/// place `prompt` is set, so a regression that reintroduces a length cap
/// here (see `embed_with`'s doc comment) is caught at the body-construction
/// level, the same way `gemini::build_embed_body`/`openai`'s chat body
/// builders are — no HTTP mock needed to prove `text` reaches the wire whole.
fn build_ollama_embed_body(model: &str, text: &str) -> Value {
    json!({ "model": model, "prompt": text, "keep_alive": crate::performance::ollama_keep_alive() })
}

pub async fn embed_with(model: &str, text: &str) -> AppResult<Vec<f64>> {
    let body = build_ollama_embed_body(model, text);
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
        let body_text =
            crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .unwrap_or_default();
        return Err(AppError::Provider(format!("Ollama {status}: {body_text}")));
    }
    let data: Value =
        crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
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
        let body =
            crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .unwrap_or_default();
        return Err(AppError::Network(format!(
            "ollama web_search {status}: {body}"
        )));
    }
    let body: Value =
        crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
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
    match crate::net::http::read_json_capped::<Value>(
        resp,
        crate::net::http::DEFAULT_MAX_BODY_BYTES,
    )
    .await
    {
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
        let body =
            crate::net::http::read_text_capped(response, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .unwrap_or_default();
        return Err(AppError::Provider(format!("Ollama {status}: {body}")));
    }

    let mut line_buf = String::new();
    // Bytes from a read that ended mid-UTF-8-sequence — see `stream::push_utf8`.
    let mut carry: Vec<u8> = Vec::new();
    while let Some(bytes) = response.chunk().await.map_err(|e| e.to_string())? {
        super::stream::push_utf8(&mut line_buf, &mut carry, &bytes);
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
            sentinel.stop_reason = ollama_done_reason(&event);
            // `done_reason` rides on this same final object. Mapping it makes
            // `stream::finish`'s length diagnosis ("ran out of output budget
            // before producing any answer") work for LOCAL Ollama too, instead
            // of only for the openai-compatible providers — a local reasoning
            // model that burns its whole budget thinking is exactly the case
            // that produced an unexplained empty generation.
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

/// Build the `/api/chat` streaming request body for a given
/// [`AiGenerateRequest`] + resolved [`SamplingProfile`] (already merged with
/// the request's explicit numeric overrides — see [`SamplingProfile::resolve`]).
/// Pure + unit-tested. Every `options.*` sampling field (`temperature`,
/// `top_p`, `repeat_penalty`) is added only when `sampling` carries `Some`
/// (never sent as `null`) — an unset field lets `/api/chat` fall back to the
/// model's own Modelfile default, which is NOT a safe default this app
/// controls (see [`AiProvider::sampling_profile`]'s doc on [`OllamaClient`]
/// for empirical evidence: some current default-tier local models default to
/// `temperature: 1`, one as high as `presence_penalty: 1.5`). `repeat_penalty`
/// uses Ollama's own field/semantics — it is NEVER a remap of
/// `frequency_penalty` (different math, different field).
fn build_chat_stream_body(req: &AiGenerateRequest, sampling: SamplingProfile) -> Value {
    let messages = serde_json::to_value(
        req.messages
            .iter()
            .map(|m| json!({ "role": m.role, "content": m.content }))
            .collect::<Vec<_>>(),
    )
    .unwrap_or(json!([]));

    let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
    // `think` is a top-level request field (NOT nested under `options`), and only
    // safe to send when it's one of `OLLAMA_EFFORT_LEVELS` (see its doc comment)
    // — it 400s on a non-thinking model (see `ollama_family_supports_thinking`'s
    // doc comment) or an unrecognized value.
    if let Some(effort) = req
        .effort
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
    {
        if ollama_family_supports_thinking(&req.model) && OLLAMA_EFFORT_LEVELS.contains(&effort) {
            body["think"] = json!(effort);
        }
    }
    let mut options = serde_json::Map::new();
    if let Some(t) = sampling.temperature {
        options.insert("temperature".to_string(), json!(t));
    }
    if let Some(top_p) = sampling.top_p {
        options.insert("top_p".to_string(), json!(top_p));
    }
    if let Some(rp) = sampling.repeat_penalty {
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

async fn stream_chat(
    app: &AppHandle,
    job_id: &str,
    req: &AiGenerateRequest,
    sampling: SamplingProfile,
) -> AppResult<()> {
    let base = host();
    let endpoint = format!("{base}/api/chat");
    let trace = RequestTrace::begin(ProviderId::Ollama, &req.model, "/api/chat", &base, true);

    let body = build_chat_stream_body(req, sampling);

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
        let body_text =
            crate::net::http::read_text_capped(response, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .unwrap_or_default();
        trace.end(Some(status.as_u16()), false);
        return Err(AppError::Provider(format!("Ollama {status}: {body_text}")));
    }

    // The shared loop owns cancel-check + chunk read + emit + complete; the closure
    // is the only Ollama-specific part (newline-delimited JSON framing). Structured
    // reasoning from thinking models rides on `message.thinking`; models that embed
    // <think>…</think> in `content` are split renderer-side. `think` is only sent
    // (by `build_chat_stream_body`) when the caller set an effort AND the model is
    // in the known thinking family — it 400s on a non-thinking model otherwise.
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
        let body_text =
            crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .unwrap_or_default();
        trace.end(Some(status.as_u16()), false);
        return Err(AppError::Provider(format!("Ollama {status}: {body_text}")));
    }
    let data: Value =
        crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
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
#[path = "ollama_tests.rs"]
mod tests;
