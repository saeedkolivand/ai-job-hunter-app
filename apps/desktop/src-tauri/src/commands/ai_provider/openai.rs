//! OpenAI and OpenAI-compatible providers (LM Studio, vLLM, OpenRouter, Groq,
//! Together, DeepSeek, Azure-style gateways…). One client, parameterized by the
//! `ProviderId` and an optional base URL.

use async_trait::async_trait;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai::get_provider_key;

use crate::error::{AppError, AppResult};

use super::research;
use super::retry::send_with_retry;
use super::stream::{stream_response, StreamPiece};
use super::timeouts;
use super::{
    friendly_api_error, model_entry, single_shot_turn, AgentTurn, AiGenerateRequest, AiProvider,
    ChatMsg, ModelCapabilities, ProviderId, RequestTrace, StopReason, TokenParam, ToolCall,
    ToolSpec, Usage,
};

const DEFAULT_BASE: &str = "https://api.openai.com/v1";

/// Concatenate the assistant text from a Responses API result. The `output`
/// array interleaves `web_search_call` items with the final `message`; we take
/// the `output_text` blocks of message items. Pure + unit-tested.
fn join_responses_text(data: &Value) -> String {
    data.get("output")
        .and_then(|o| o.as_array())
        .map(|items| {
            items
                .iter()
                .filter(|it| it.get("type").and_then(|t| t.as_str()) == Some("message"))
                .filter_map(|it| it.get("content").and_then(|c| c.as_array()))
                .flatten()
                .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Parse a non-streaming Chat Completions response into an [`AgentTurn`]:
/// `choices[0].message.content` is the text (may be null when tool calls are
/// present), each `choices[0].message.tool_calls[]` maps to a [`ToolCall`] (its
/// `function.arguments` is a JSON *string* — decoded here; malformed → `{}`), and
/// `finish_reason` maps to the stop reason (`tool_calls`→ToolUse, `stop`→End,
/// `length`→Length, else Other). Pure + unit-tested.
fn parse_openai_turn(data: &Value) -> AgentTurn {
    let choice = data.get("choices").and_then(|c| c.get(0));
    let message = choice.and_then(|c| c.get("message"));
    let text = message
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_string();
    let tool_calls = message
        .and_then(|m| m.get("tool_calls"))
        .and_then(|c| c.as_array())
        .map(|calls| {
            calls
                .iter()
                .filter_map(|c| {
                    let func = c.get("function")?;
                    let name = func.get("name").and_then(|n| n.as_str())?.to_string();
                    let args = func
                        .get("arguments")
                        .and_then(|a| a.as_str())
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        .unwrap_or_else(|| json!({}));
                    Some(ToolCall {
                        id: c
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        name,
                        args,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let stop = match choice
        .and_then(|c| c.get("finish_reason"))
        .and_then(|f| f.as_str())
    {
        Some("tool_calls") => StopReason::ToolUse,
        Some("stop") => StopReason::End,
        Some("length") => StopReason::Length,
        _ => StopReason::Other,
    };
    AgentTurn {
        text,
        tool_calls,
        stop,
        usage: parse_openai_usage(data).unwrap_or_default(),
    }
}

/// Strip the query string / fragment from a failed request's URL before it
/// reaches an error message or log line. Some OpenAI-compatible gateways put
/// the API key in the base URL's own query string (see
/// [`OpenAiClient::endpoint_url`]'s doc comment) — `reqwest::Error`'s own
/// `Display` embeds the request URL verbatim and only ever strips userinfo,
/// never query or fragment; confirmed via `reqwest::Error::without_url`'s own
/// doc: "If the URL contains sensitive information (e.g. an API key as a
/// query parameter), be sure to remove it." Verified empirically (see
/// `openai_tests.rs`) that even a CORRECTLY built [`OpenAiClient::endpoint_url`]
/// still carries the secret into a genuine transport-failure `Display` —
/// fixing the URL construction alone does not stop the leak. Clears only the
/// query/fragment (via `url_mut`), not the whole URL, so scheme/host/path
/// stay visible for diagnosing a wrong-path bug.
fn scrub_url_secret(mut e: reqwest::Error) -> reqwest::Error {
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
fn should_list_model(provider: ProviderId, id: &str) -> bool {
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
fn resolve_openai_key(provider: ProviderId, stored: Option<String>) -> AppResult<Option<String>> {
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
fn parse_model_list(provider: ProviderId, body: &Value) -> AppResult<Vec<Value>> {
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

/// OpenAI reasoning families (the `o`-series: o1, o3, o4, … and future `o`N)
/// reject `temperature` and require `max_completion_tokens` instead of
/// `max_tokens`. Matched by the `o`+digit convention so new o-series models are
/// handled without a code change.
///
/// This predicate is the `supports_temperature`/`token_param` gate ONLY — it
/// does NOT cover OpenAI's current gpt-5.x reasoning line (see
/// [`is_gpt5_or_later_reasoning_family`]), which accepts a normal
/// `temperature`/`max_tokens` unlike the o-series. Reusing this for the
/// `reasoning_effort` gate would silently exclude gpt-5.x — the two gates
/// answer different questions and must stay separate.
fn is_reasoning_model(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    let mut bytes = m.bytes();
    matches!((bytes.next(), bytes.next()), (Some(b'o'), Some(d)) if d.is_ascii_digit())
}

/// OpenAI's CURRENT reasoning-model line — gpt-5 and later (verified against
/// the live reasoning guide, `platform.openai.com/docs/guides/reasoning`,
/// fetched 2026-08-04: "Start with `gpt-5.6` for most reasoning workloads");
/// `docs/models/gpt-5.6.md`-style model pages confirm `reasoning_effort`
/// support on `/v1/chat/completions`. Distinct from (and additive to)
/// [`is_reasoning_model`]'s legacy o-series gate — a REQUEST SCHEMA
/// (`CreateChatCompletionRequest`) never carries a model list, so this is
/// verified against the provider's model/capability docs, not the schema.
///
/// Matches any `gpt-`+digit-major≥5 id (`gpt-5`, `gpt-5-mini`, `gpt-5.4`,
/// `gpt-5.5`, `gpt-5.6` and its `-sol`/`-terra`/`-luna` aliases) so a NEW
/// gpt-5.x variant — or a later numbered major line, should OpenAI keep this
/// convention — is picked up with no code change, EXCEPT the `-chat-latest`
/// family (`gpt-5-chat-latest`, `gpt-5.1-chat-latest`, …): OpenAI's
/// non-reasoning conversational variant of each gpt-5.x generation (mirrors
/// the older `chatgpt-4o-latest` naming), confirmed in the live
/// `ModelIdsShared` enum — explicitly excluded.
fn is_gpt5_or_later_reasoning_family(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    if m.contains("chat-latest") {
        return false;
    }
    let Some(rest) = m.strip_prefix("gpt-") else {
        return false;
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().is_ok_and(|major| major >= 5)
}

/// Split one streaming chunk into `(reasoning, content)` deltas.
///
/// OpenAI-compatible servers that expose chain-of-thought put it on
/// `delta.reasoning_content` (DeepSeek-R1, vLLM, LM Studio, Ollama's OpenAI
/// shim) or `delta.reasoning` (OpenRouter); the visible answer stays on
/// `delta.content`. Either may be empty/absent. Pure + unit-tested so the
/// streaming loop stays a thin emitter.
///
/// Honest limitation: OpenAI's own o-series hide their reasoning text over Chat
/// Completions, so there is nothing to surface there — only the answer streams.
/// Extract `usage.{prompt_tokens,completion_tokens}` from an OpenAI Chat
/// Completions response/chunk — always present on the non-streaming response,
/// and (with `stream_options.include_usage: true`, set by
/// [`build_chat_stream_body`]) on ONE extra streamed chunk carrying no delta,
/// emitted right before `[DONE]`. `None` on every other streamed chunk. Pure +
/// unit-tested.
fn parse_openai_usage(data: &Value) -> Option<Usage> {
    let usage = data.get("usage")?;
    Some(Usage {
        input_tokens: usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
    })
}

/// Extract real token usage from an OpenAI `/embeddings` response:
/// `usage.prompt_tokens` (falling back to `usage.total_tokens`, which some
/// OpenAI-compatible servers send instead), and `output_tokens: 0` — an
/// embedding call has no completion tokens. Zero on both fields when `usage`
/// is entirely absent (never fabricated). Pure + unit-tested.
fn parse_openai_embed_usage(data: &Value) -> Usage {
    let usage = data.get("usage");
    let input_tokens = usage
        .and_then(|u| u.get("prompt_tokens").or_else(|| u.get("total_tokens")))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    Usage {
        input_tokens,
        output_tokens: 0,
    }
}

fn parse_openai_delta(event: &Value) -> (&str, &str) {
    let delta = event
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("delta"));
    let reasoning = delta
        .and_then(|d| d.get("reasoning_content").or_else(|| d.get("reasoning")))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let content = delta
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("");
    (reasoning, content)
}

/// Extract a streamed chunk's `finish_reason`, when present and non-null.
/// Most streamed chunks carry `finish_reason: null`; only the terminal
/// content-bearing chunk (typically right before `data: [DONE]`) sets it.
/// Ollama Cloud (routed through this same client, see `ollama_cloud.rs`) uses
/// the identical Chat Completions streaming shape. Reuses the SAME mapping
/// [`parse_openai_turn`] already uses for the non-streaming path, so callers
/// never need a second vocabulary. Pure + unit-tested.
fn parse_openai_finish_reason(event: &Value) -> Option<StopReason> {
    let reason = event
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("finish_reason"))
        .and_then(|f| f.as_str())?;
    Some(match reason {
        "tool_calls" => StopReason::ToolUse,
        "stop" => StopReason::End,
        "length" => StopReason::Length,
        _ => StopReason::Other,
    })
}

/// Drain complete `data:`-prefixed SSE lines from the accumulated stream buffer
/// into [`StreamPiece`]s, leaving any partial trailing line for the next chunk.
/// `data: [DONE]` yields a terminal sentinel; other lines split into reasoning +
/// content via [`parse_openai_delta`], plus a `stop_reason` piece whenever a
/// chunk carries a non-null `finish_reason` (see
/// [`parse_openai_finish_reason`]). Pure + unit-tested; this is the `parse`
/// closure handed to [`stream_response`], so OpenAI's SSE framing lives here only.
fn parse_openai_frames(buf: &mut String) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    // Walk by a `consumed` offset and `drain(..consumed)` once at the end, instead
    // of reallocating the whole tail per line (O(n²) on a big frame).
    let mut consumed = 0;
    while let Some(rel) = buf[consumed..].find('\n') {
        let nl = consumed + rel;
        let line = buf[consumed..nl].trim().to_string();
        consumed = nl + 1;

        let data = match line.strip_prefix("data: ") {
            Some(d) => d.trim(),
            None => continue,
        };
        if data == "[DONE]" {
            buf.drain(..consumed);
            out.push(StreamPiece::done(""));
            return out;
        }
        let event: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(usage) = parse_openai_usage(&event) {
            out.push(StreamPiece::usage(usage));
        }
        if let Some(reason) = parse_openai_finish_reason(&event) {
            out.push(StreamPiece::stop_reason(reason));
        }
        let (reasoning, delta) = parse_openai_delta(&event);
        if !reasoning.is_empty() {
            out.push(StreamPiece::thinking(reasoning));
        }
        if !delta.is_empty() {
            out.push(StreamPiece::text(delta));
        }
    }
    // Drop the fully-parsed prefix once; the partial trailing line stays buffered.
    buf.drain(..consumed);
    out
}

/// Levels `reasoning_effort` accepts on every reasoning-capable model this
/// adapter recognizes (native OpenAI's o-series + gpt-5.x, and Ollama
/// Cloud's thinking family — see [`OpenAiClient::supports_reasoning_effort`]).
/// Verified against the live OpenAPI schema
/// (`raw.githubusercontent.com/openai/openai-openapi/master/openapi.yaml`,
/// `ReasoningEffort` schema, checked 2026-08-04): the real wire enum has
/// grown to SEVEN values (`none, minimal, low, medium, high, xhigh, max`),
/// and the live reasoning guide (`platform.openai.com/docs/guides/reasoning`,
/// same date) states plainly: "Some models support only a subset of these
/// values, so check the relevant model page" — genuinely per-model, the SAME
/// class of variance Gemini's `thinkingLevel` and Anthropic's
/// `output_config.effort` have (see `gemini_effort_levels` / `anthropic_effort_levels`).
///
/// This adapter deliberately exposes only the THREE values every recognized
/// reasoning model accepts with no further per-model check. Unlike
/// Gemini/Anthropic, OpenAI's guide has no single closed table mapping value
/// -> supporting models — it defers to each individual model's own page, and
/// [`is_gpt5_or_later_reasoning_family`] deliberately matches ANY `gpt-5.x`+
/// id (including snapshots that predate `xhigh`/`max`, which the guide
/// frames as a recent addition alongside GPT-5.6's reasoning-mode overhaul).
/// Enumerating a real per-model-id table here would mean checking each
/// model's own page individually — a materially larger, separate piece of
/// work, not a same-shaped fix as the Gemini/Anthropic tables (flag as a
/// follow-up, don't guess it here). `low`/`medium`/`high` carry no per-model
/// caveat in either source, so they stay the safe universal baseline.
///
/// Still gated with `.contains(&effort)` on the send path below, not just
/// the `supports_reasoning` boolean — the same protection Gemini/Anthropic
/// use, so this stays correct with zero further change the day a follow-up
/// DOES expose a richer, genuinely per-model set here (`effort` is stored
/// PER PROVIDER, not per model — `preferences-store.ts`).
const OPENAI_EFFORT_LEVELS: [&str; 3] = ["low", "medium", "high"];

/// Build the `/chat/completions` streaming request body for a given
/// [`AiGenerateRequest`] + capability matrix. Pure + unit-tested — this is the
/// shared body shape for native OpenAI, any OpenAI-compatible gateway, and
/// Ollama Cloud (all backed by [`OpenAiClient`]). `top_p`/`frequency_penalty`/
/// `presence_penalty` are the detector-resistance sampling knobs (RAID, ACL
/// 2024) the renderer sets only for prose generation surfaces — each is only
/// ever added when `Some` (never sent as `null`), and skipped entirely on
/// reasoning models that reject `temperature`.
fn build_chat_stream_body(req: &AiGenerateRequest, caps: ModelCapabilities) -> Value {
    let messages = req
        .messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect::<Vec<_>>();

    let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
    // Real per-call token usage (AI-spend visibility, `crate::spend`): asks for
    // one extra streamed chunk carrying `usage` right before `[DONE]`. Every
    // OpenAI-compatible server this client talks to (native OpenAI, LM
    // Studio/vLLM/OpenRouter/Groq/…, Ollama Cloud) either honors this or
    // silently ignores the unknown field — never a 400.
    body["stream_options"] = json!({ "include_usage": true });
    if caps.supports_temperature {
        body["temperature"] = json!(req.temperature.unwrap_or(0.7));
        if let Some(top_p) = req.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(fp) = req.frequency_penalty {
            body["frequency_penalty"] = json!(fp);
        }
        if let Some(pp) = req.presence_penalty {
            body["presence_penalty"] = json!(pp);
        }
    }
    if let Some(mt) = req.max_tokens {
        let field = match caps.token_param {
            TokenParam::MaxCompletionTokens => "max_completion_tokens",
            _ => "max_tokens",
        };
        body[field] = json!(mt);
    }
    // Only ever sent when it's one of `OPENAI_EFFORT_LEVELS` (see its doc
    // comment) AND `caps.supports_reasoning` is true (native OpenAI's
    // o-series, or Ollama Cloud's thinking-family models — see
    // `OpenAiClient::supports_reasoning_effort`) — a wrong/guessed value 400s.
    if caps.supports_reasoning {
        if let Some(effort) = req
            .effort
            .as_deref()
            .map(str::trim)
            .filter(|e| !e.is_empty())
        {
            if OPENAI_EFFORT_LEVELS.contains(&effort) {
                body["reasoning_effort"] = json!(effort);
            }
        }
    }
    body
}

pub struct OpenAiClient {
    id: ProviderId,
    base_url: String,
}

impl OpenAiClient {
    pub fn new(id: ProviderId, base_url: Option<String>) -> Self {
        Self {
            id,
            base_url: base_url
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_BASE.to_string()),
        }
    }

    /// Build a URL for `path` (a `/`-separated relative endpoint, e.g.
    /// `"models"` or `"chat/completions"`) on `self.base_url`, preserving any
    /// existing query string / fragment the base carries untouched. Some
    /// OpenAI-compatible gateways (Cloudflare AI Gateway, several self-hosted
    /// proxies) authenticate via the base URL's own query string — e.g.
    /// `https://gw.example.com/v1?api-key=SECRET`. Plain
    /// `format!("{base}/{path}")` string concatenation sends THAT case to the
    /// wrong path (the string reparses as path `/v1`, query
    /// `api-key=SECRET/path`) and corrupts the key. `Url::join` is not a safe
    /// drop-in either — verified empirically (see `openai_tests.rs`): a plain
    /// relative reference like `"models"` carries no query of its own, and
    /// WHATWG relative-URL resolution defines that as "clear the query" on
    /// join, so it would silently DROP a working gateway's auth query string
    /// rather than construct a malformed URL. `path_segments_mut` (with
    /// `pop_if_empty` so a base with OR without a trailing slash both resolve
    /// correctly, never a double slash) only appends path segments and
    /// leaves scheme/host/query/fragment untouched — the correct primitive
    /// for "hit a sibling endpoint on the same base".
    fn endpoint_url(&self, path: &str) -> AppResult<reqwest::Url> {
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

    /// Whether this client's provider id exposes OpenAI's native `web_search`
    /// tool — only native OpenAI does; every OpenAI-compatible gateway can't be
    /// assumed to support it, and Ollama Cloud overrides `research()`/
    /// `research_salary()` on its own client. Factored to a pure, `AppHandle`-free
    /// predicate purely so the gate stays unit-testable (this crate has no
    /// `tauri::test` mock-app harness to drive `web_search_complete` itself end
    /// to end — see the same note on `salary_research::SalaryResearch::enrich`).
    fn supports_web_search(&self) -> bool {
        self.id == ProviderId::OpenAi
    }

    /// Whether this client's provider id + model accepts the `reasoning_effort`
    /// field on `/chat/completions` (verified against the provider's live
    /// model/capability docs — a REQUEST SCHEMA like
    /// `CreateChatCompletionRequest` never carries a model list, so a gate
    /// like this one is checked against OpenAI's reasoning guide + model
    /// pages, not the schema — and Ollama's OpenAI-compatibility reference,
    /// fetched 2026-08-04). Native OpenAI: the legacy o-series
    /// ([`is_reasoning_model`]) OR the current gpt-5.x line
    /// ([`is_gpt5_or_later_reasoning_family`]) — two SEPARATE gates ORed
    /// together, not one reused, because gpt-5.x accepts a normal
    /// `temperature` unlike the o-series (see `is_reasoning_model`'s doc
    /// comment). Ollama Cloud: a DIFFERENT gate —
    /// [`ollama::ollama_family_supports_thinking`], the same
    /// thinking-model-family classifier local Ollama's native `think` field
    /// uses — its `/v1` endpoint is OpenAI-compatible but its model CATALOG is
    /// Ollama's own (e.g. `gpt-oss:120b` doesn't match the `o`+digit or
    /// `gpt-5`+ conventions). Every other OpenAI-compatible gateway (LM
    /// Studio, OpenRouter, generic `openai-compatible`) is an unknown catalog
    /// — never guessed, so a wrong value can't 400 a gateway this adapter
    /// knows nothing about.
    fn supports_reasoning_effort(&self, model: &str) -> bool {
        match self.id {
            ProviderId::OpenAi => {
                is_reasoning_model(model) || is_gpt5_or_later_reasoning_family(model)
            }
            ProviderId::OllamaCloud => super::ollama::ollama_family_supports_thinking(model),
            _ => false,
        }
    }

    /// Shared body of `complete`/`complete_with_usage`: one non-streaming
    /// `/chat/completions` call, parsed once into `(text, usage)` so the two
    /// trait methods never duplicate the HTTP round-trip.
    async fn complete_impl(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let caps = self.capabilities(model);
        let endpoint = self.endpoint_url("chat/completions")?;
        let trace = RequestTrace::begin(self.id, model, "/chat/completions", &self.base_url, false);

        let mut body = json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
            "stream": false,
        });
        if caps.supports_temperature {
            body["temperature"] = json!(temperature.unwrap_or(0.7));
        }

        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(endpoint.clone())
                .timeout(timeouts::COMPLETION)
                .bearer_auth(&api_key)
                .json(&body)
        })
        .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!(
                    "{} unreachable: {}",
                    self.id.as_str(),
                    scrub_url_secret(e)
                )));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(self.id, status, &body_text));
        }
        let data: Value = match crate::net::http::read_json_capped(
            resp,
            crate::net::http::DEFAULT_MAX_BODY_BYTES,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                trace.end(Some(status.as_u16()), false);
                return Err(AppError::Message(format!("parse: {e}")));
            }
        };
        trace.end(Some(status.as_u16()), true);
        let text = data
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .map(String::from)
            .ok_or_else(|| {
                AppError::Provider(format!("{}: unexpected response shape", self.id.as_str()))
            })?;
        let usage = parse_openai_usage(&data).unwrap_or_default();
        Ok((text, usage))
    }

    /// Shared body of `embed`/`embed_with_usage`: one `/embeddings` call,
    /// parsed once into `(vector, usage)` so the two trait methods never
    /// duplicate the HTTP round-trip.
    async fn embed_impl(
        &self,
        app: &AppHandle,
        model: &str,
        text: &str,
    ) -> AppResult<(Vec<f64>, Usage)> {
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let endpoint = self.endpoint_url("embeddings")?;
        let trace = RequestTrace::begin(self.id, model, "/embeddings", &self.base_url, false);
        let body = json!({ "model": model, "input": text });
        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(endpoint.clone())
                .timeout(timeouts::EMBED)
                .bearer_auth(&api_key)
                .json(&body)
        })
        .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Message(format!(
                    "{} unreachable: {}",
                    self.id.as_str(),
                    scrub_url_secret(e)
                )));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(self.id, status, &body_text));
        }
        let data: Value = match crate::net::http::read_json_capped(
            resp,
            crate::net::http::DEFAULT_MAX_BODY_BYTES,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                trace.end(Some(status.as_u16()), false);
                return Err(AppError::Message(format!("parse: {e}")));
            }
        };
        trace.end(Some(status.as_u16()), true);
        let vector: Vec<f64> = data
            .get("data")
            .and_then(|d| d.get(0))
            .and_then(|e| e.get("embedding"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
            .ok_or_else(|| {
                AppError::Provider(format!(
                    "{}: missing embedding in response",
                    self.id.as_str()
                ))
            })?;
        Ok((vector, parse_openai_embed_usage(&data)))
    }

    /// Shared transport for every `research*` facet: the Responses API with the
    /// native `web_search` tool, `system`/`user` supplied by the caller. Every
    /// non-OpenAI id degrades to `""`, exactly like a missing key or a failed
    /// call.
    async fn web_search_complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
    ) -> AppResult<String> {
        if !self.supports_web_search() {
            return Ok(String::new());
        }
        let api_key = match get_provider_key(app, self.id.credential_key()) {
            Some(k) if !k.trim().is_empty() => k,
            _ => return Ok(String::new()),
        };
        self.web_search_transport(&api_key, model, system, user)
            .await
    }

    /// The `/responses` HTTP transport itself — no `AppHandle`/keychain, so it's
    /// directly testable against a `wiremock::MockServer` (see the tests below).
    /// Behavior-preserving extraction from `web_search_complete`: a transport
    /// failure, a non-2xx status, and a non-JSON body all degrade to `""` (never
    /// an error) — the same gentle-degrade contract the caller already promises.
    async fn web_search_transport(
        &self,
        api_key: &str,
        model: &str,
        system: &str,
        user: &str,
    ) -> AppResult<String> {
        let endpoint = match self.endpoint_url("responses") {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!("openai research: {e}");
                return Ok(String::new());
            }
        };
        let trace = RequestTrace::begin(
            self.id,
            model,
            "/responses web_search",
            &self.base_url,
            false,
        );

        let body = json!({
            "model": model,
            "instructions": system,
            "input": user,
            "tools": [{ "type": "web_search" }],
        });
        let resp = crate::net::http::shared()
            .post(endpoint)
            .timeout(timeouts::WEB_SEARCH)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                tracing::warn!("openai research unreachable: {}", scrub_url_secret(e));
                return Ok(String::new());
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            tracing::warn!("openai research {status}: {body_text}");
            return Ok(String::new());
        }
        let data: Value = match crate::net::http::read_json_capped(
            resp,
            crate::net::http::DEFAULT_MAX_BODY_BYTES,
        )
        .await
        {
            Ok(v) => v,
            Err(_) => {
                trace.end(Some(status.as_u16()), false);
                return Ok(String::new());
            }
        };
        trace.end(Some(status.as_u16()), true);
        Ok(join_responses_text(&data))
    }

    /// Build the `GET {base_url}/models` request, attaching the bearer header
    /// only when a key is present — never an empty `Authorization: Bearer`
    /// value for a keyless `OpenAiCompatible` deployment (some gateways
    /// reject a malformed/empty header rather than ignoring it). Shared by
    /// `list_models_transport` and `test_key`.
    fn list_models_request(&self, api_key: Option<&str>) -> AppResult<reqwest::RequestBuilder> {
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
    /// directly testable against a `wiremock::MockServer` (see the tests below),
    /// mirroring [`web_search_transport`](Self::web_search_transport).
    async fn list_models_transport(&self, api_key: Option<&str>) -> AppResult<Vec<Value>> {
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
}

#[async_trait]
impl AiProvider for OpenAiClient {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        // Rejecting `temperature` is an o-series-ONLY quirk — distinct from
        // "accepts reasoning_effort" (Ollama Cloud's gpt-oss/deepseek/qwen3
        // models accept both temperature AND reasoning_effort), so these are
        // two separate gates, not one reused variable.
        let rejects_temperature = is_reasoning_model(model);
        ModelCapabilities {
            supports_temperature: !rejects_temperature,
            supports_system_role: true,
            supports_streaming: true,
            supports_reasoning: self.supports_reasoning_effort(model),
            supports_tools: true,
            supports_json_mode: true,
            supports_embeddings: true,
            // Only native OpenAI exposes the `web_search` tool; any
            // OpenAI-compatible gateway (LM Studio, OpenRouter, …) can't be
            // assumed to — see `supports_web_search()`.
            supports_web_search: self.supports_web_search(),
            token_param: if rejects_temperature {
                TokenParam::MaxCompletionTokens
            } else {
                TokenParam::MaxTokens
            },
        }
    }

    fn effort_levels(&self, model: &str) -> Vec<&'static str> {
        if self.supports_reasoning_effort(model) {
            OPENAI_EFFORT_LEVELS.to_vec()
        } else {
            Vec::new()
        }
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let caps = self.capabilities(&req.model);
        let endpoint = self.endpoint_url("chat/completions")?;
        let trace = RequestTrace::begin(
            self.id,
            &req.model,
            "/chat/completions",
            &self.base_url,
            true,
        );

        let body = build_chat_stream_body(req, caps);

        let response = crate::net::http::shared()
            .post(endpoint)
            .timeout(timeouts::STREAM)
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!(
                    "{} unreachable: {}",
                    self.id.as_str(),
                    scrub_url_secret(e)
                )));
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body_text = crate::net::http::read_text_capped(
                response,
                crate::net::http::DEFAULT_MAX_BODY_BYTES,
            )
            .await
            .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(self.id, status, &body_text));
        }

        // The shared loop owns cancel-check + chunk read + emit + complete; this
        // closure is the only OpenAI-specific part (its `data:`-prefixed SSE framing).
        stream_response(
            app,
            job_id,
            &trace,
            response,
            status.as_u16(),
            self.id,
            &req.model,
            &self.base_url,
            parse_openai_frames,
        )
        .await
    }

    async fn complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<String> {
        self.complete_impl(app, model, system, user, temperature)
            .await
            .map(|(text, _)| text)
    }

    async fn complete_with_usage(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
        self.complete_impl(app, model, system, user, temperature)
            .await
    }

    async fn research(
        &self,
        app: &AppHandle,
        model: &str,
        company: &str,
        role: &str,
    ) -> AppResult<String> {
        self.web_search_complete(
            app,
            model,
            research::NATIVE_SYSTEM,
            &research::native_user(company, role),
        )
        .await
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
        self.web_search_complete(
            app,
            model,
            &research::salary_system(currency),
            &research::salary_user(role, company, location, country, currency),
        )
        .await
    }

    async fn research_answer(
        &self,
        app: &AppHandle,
        model: &str,
        question: &str,
        role: &str,
        company: &str,
    ) -> AppResult<String> {
        self.web_search_complete(
            app,
            model,
            research::ANSWER_SYSTEM,
            &research::answer_user(question, role, company),
        )
        .await
    }

    async fn embed(&self, app: &AppHandle, model: &str, text: &str) -> AppResult<Vec<f64>> {
        self.embed_impl(app, model, text).await.map(|(v, _)| v)
    }

    async fn embed_with_usage(
        &self,
        app: &AppHandle,
        model: &str,
        text: &str,
    ) -> AppResult<(Vec<f64>, Usage)> {
        self.embed_impl(app, model, text).await
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        Some("text-embedding-3-small")
    }

    fn max_embedding_input_chars(&self) -> usize {
        // text-embedding-3-* enforce a hard 8191-TOKEN limit and ERROR (no
        // auto-truncate) when exceeded. The old 32k-char cap assumed ~4 chars/token
        // (English); for token-dense scripts (CJK ≈ 1 char/token) 32k chars ≈ 32k
        // tokens — far over 8191 — so the request would FAIL. Cap at 8000 chars
        // PER CHUNK: in the worst case (≈1 char/token) that stays under 8191
        // tokens for every language. A document longer than this is split into
        // multiple chunks and mean-pooled by `embed_adaptive` — never silently
        // truncated away.
        8_000
    }

    async fn list_models(&self, app: &AppHandle) -> AppResult<Vec<Value>> {
        let api_key = resolve_openai_key(self.id, get_provider_key(app, self.id.credential_key()))?;
        self.list_models_transport(api_key.as_deref()).await
    }

    async fn test_key(&self, app: &AppHandle) -> AppResult<()> {
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

    async fn chat_with_tools(
        &self,
        app: &AppHandle,
        model: &str,
        messages: &[ChatMsg],
        tools: &[ToolSpec],
        temperature: Option<f64>,
    ) -> AppResult<AgentTurn> {
        let caps = self.capabilities(model);
        if !caps.supports_tools {
            return single_shot_turn(self, app, model, messages, temperature).await;
        }
        let api_key = get_provider_key(app, self.id.credential_key()).unwrap_or_default();
        let endpoint = self.endpoint_url("chat/completions")?;
        let trace = RequestTrace::begin(
            self.id,
            model,
            "/chat/completions tools",
            &self.base_url,
            false,
        );

        let wire_messages: Vec<Value> = messages
            .iter()
            .map(|m| json!({ "role": m.role.wire(), "content": m.content }))
            .collect();
        // OpenAI function-tool shape. The schema is trusted, fixed input — never
        // built from scraped/model text.
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
            "messages": wire_messages,
            "stream": false,
            "tools": tool_specs,
            "tool_choice": "auto",
        });
        if caps.supports_temperature {
            body["temperature"] = json!(temperature.unwrap_or(0.7));
        }

        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(endpoint.clone())
                .timeout(timeouts::COMPLETION)
                .bearer_auth(&api_key)
                .json(&body)
        })
        .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!(
                    "{} unreachable: {}",
                    self.id.as_str(),
                    scrub_url_secret(e)
                )));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(self.id, status, &body_text));
        }
        let data: Value = match crate::net::http::read_json_capped(
            resp,
            crate::net::http::DEFAULT_MAX_BODY_BYTES,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                trace.end(Some(status.as_u16()), false);
                return Err(AppError::Message(format!("parse: {e}")));
            }
        };
        trace.end(Some(status.as_u16()), true);
        Ok(parse_openai_turn(&data))
    }
}

#[cfg(test)]
#[path = "openai_tests.rs"]
mod tests;
