//! Google Gemini provider — generateContent (streaming) API.

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
    friendly_api_error, single_shot_turn, split_system, AgentTurn, AiGenerateRequest, AiProvider,
    ChatMsg, ModelCapabilities, ProviderId, RequestTrace, Role, StopReason, TokenParam, ToolCall,
    ToolSpec,
};

const BASE: &str = "https://generativelanguage.googleapis.com";

/// Validate the key the keychain returned, rejecting a missing/blank one early.
///
/// Pure (no `AppHandle`) so it's unit-testable. Several call paths previously
/// defaulted a missing key to `""` and still issued the request, sending an empty
/// `x-goog-api-key` header — a guaranteed 401 round-trip. This fails fast with the
/// same unauthorized error `friendly_api_error` maps a real 401/403 to, so the
/// message stays consistent.
fn validate_gemini_key(stored: Option<String>) -> AppResult<String> {
    match stored {
        Some(k) if !k.trim().is_empty() => Ok(k),
        _ => Err(AppError::Config(format!(
            "{}: invalid or unauthorized API key.",
            ProviderId::Gemini.as_str()
        ))),
    }
}

/// Resolve the stored Gemini key, rejecting a missing/blank one before any request.
fn require_gemini_key(app: &AppHandle) -> AppResult<String> {
    validate_gemini_key(get_provider_key(app, ProviderId::Gemini.credential_key()))
}

/// Concatenate every `parts[].text` of the first candidate (non-streaming
/// `generateContent`, incl. grounded responses) into one string. Pure +
/// unit-tested.
fn join_parts_text(data: &Value) -> String {
    data.get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Parse a non-streaming `generateContent` response into an [`AgentTurn`]: join
/// the first candidate's `parts[].text` for the visible text, map every
/// `parts[].functionCall` to a [`ToolCall`] (Gemini has no call id, so synthesize
/// `name-index` for our own bookkeeping — `functionResponse` matches by name), and
/// set the stop reason (any functionCall ⇒ ToolUse, else `MAX_TOKENS`→Length /
/// `STOP`→End). `finishReason: "MALFORMED_FUNCTION_CALL"` — Gemini's signal that a
/// tool call was truncated/cut off by the output-token limit — always wins and maps
/// to `Length` too, even if a (possibly half-serialized) functionCall part is
/// present, so those args never reach a tool handler. Pure + unit-tested.
fn parse_gemini_turn(data: &Value) -> AgentTurn {
    let text = join_parts_text(data);
    let finish_reason = data
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("finishReason"))
        .and_then(|f| f.as_str());
    let tool_calls: Vec<ToolCall> = data
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .enumerate()
                .filter_map(|(i, part)| {
                    let fc = part.get("functionCall")?;
                    let name = fc.get("name").and_then(|n| n.as_str())?.to_string();
                    let args = fc.get("args").cloned().unwrap_or_else(|| json!({}));
                    Some(ToolCall {
                        id: format!("{name}-{i}"),
                        name,
                        args,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let stop = if finish_reason == Some("MALFORMED_FUNCTION_CALL") {
        StopReason::Length
    } else if !tool_calls.is_empty() {
        // Gemini reports `finishReason: "STOP"` even when returning a functionCall,
        // so the presence of a call is the authoritative "wants tools back" signal.
        StopReason::ToolUse
    } else {
        match finish_reason {
            Some("MAX_TOKENS") => StopReason::Length,
            Some("STOP") => StopReason::End,
            _ => StopReason::Other,
        }
    };
    AgentTurn {
        text,
        tool_calls,
        stop,
    }
}

/// Whether to request `thinkingConfig.includeThoughts`. Gemini 1.5 and the GA
/// 2.0 models reject `thinkingConfig` with a 400, so we only enable it for the
/// 2.5 family and any explicit `*-thinking-*` model. Unknown future models simply
/// don't surface thoughts (a graceful miss, never a broken request).
fn gemini_supports_thinking(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.contains("2.5") || m.contains("thinking")
}

/// Extract a Gemini chunk's streamed parts as `(is_thought, text)` pairs. 2.5
/// thinking models flag reasoning parts with `"thought": true`; the rest are
/// normal answer text. Pure + unit-tested so the streaming loop stays a thin
/// emitter.
fn parse_gemini_parts(event: &Value) -> Vec<(bool, &str)> {
    event
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| {
                    let text = part.get("text").and_then(|t| t.as_str())?;
                    let thought = part
                        .get("thought")
                        .and_then(|t| t.as_bool())
                        .unwrap_or(false);
                    Some((thought, text))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Brace-depth scanner state carried across chunk boundaries while reading
/// Gemini's streamed JSON **array** for complete top-level objects. `pending`
/// holds the bytes of the current (possibly partial) object accumulated so far;
/// `depth`/`in_string`/`escape` track where we are inside it. Array punctuation
/// (`[`, `,`, `]`, whitespace) seen at depth 0 before an object's opening `{` is
/// dropped, exactly as the original inline scanner did via its `trim_start`
/// `starts_with('{')` guard.
#[derive(Debug, Default)]
struct GeminiScanner {
    pending: String,
    depth: i32,
    in_string: bool,
    escape: bool,
}

/// Feed the freshly-arrived chunk in `buf` through the [`GeminiScanner`], emitting
/// a [`StreamPiece`] per non-empty `parts[].text` of every complete top-level
/// object. The caller pushes the new chunk into `buf`; this consumes it entirely
/// (leaving `buf` empty) and stashes any partial trailing object in `state.pending`
/// for the next chunk. Gemini has no in-band done sentinel — the stream ends with
/// the HTTP body — so this never yields a `done` piece (the shared loop completes
/// on end-of-body).
///
/// Behavior matches the original inline char scanner: an object is recognized when
/// brace depth returns to 0 *and* the accumulated text trims to something starting
/// with `{`; on a successful parse the accumulator is cleared and scanning
/// continues with the rest of the chunk. Pure + unit-tested; this is Gemini's
/// `parse` closure, so its JSON-array framing lives here only.
fn parse_gemini_frames(buf: &mut String, state: &mut GeminiScanner) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    let chunk = std::mem::take(buf);
    for ch in chunk.chars() {
        // Drop the JSON-array framing (`[`, `]`, `,`, whitespace) that appears at
        // depth 0 before an object's `{`; otherwise it pollutes `pending` and the
        // `starts_with('{')` guard never fires for `[{…}` / `,{…}`.
        if !state.in_string
            && state.depth == 0
            && state.pending.is_empty()
            && (matches!(ch, '[' | ']' | ',') || ch.is_whitespace())
        {
            continue;
        }
        if state.escape {
            state.escape = false;
            state.pending.push(ch);
            continue;
        }
        if ch == '\\' && state.in_string {
            state.escape = true;
            state.pending.push(ch);
            continue;
        }
        if ch == '"' {
            state.in_string = !state.in_string;
        }
        if !state.in_string {
            if ch == '{' {
                state.depth += 1;
            } else if ch == '}' {
                state.depth -= 1;
            }
        }
        state.pending.push(ch);

        if state.depth == 0
            && state.pending.trim_start().starts_with('{')
            && !state.pending.trim().is_empty()
        {
            if let Ok(event) = serde_json::from_str::<Value>(state.pending.trim()) {
                for (thought, text) in parse_gemini_parts(&event) {
                    if text.is_empty() {
                        continue;
                    }
                    out.push(if thought {
                        StreamPiece::thinking(text)
                    } else {
                        StreamPiece::text(text)
                    });
                }
            }
            state.pending.clear();
        }
    }
    out
}

pub struct GeminiClient;

impl GeminiClient {
    /// Shared transport for every `research*` facet: `generateContent` grounded
    /// with the native Google Search tool, `system`/`user` supplied by the
    /// caller. Degrades to `""` (never an error) on a missing key or any
    /// transport/response failure, so generation always proceeds.
    async fn web_search_complete(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
    ) -> AppResult<String> {
        let api_key = match get_provider_key(app, self.id().credential_key()) {
            Some(k) if !k.trim().is_empty() => k,
            _ => return Ok(String::new()),
        };
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:generateContent");
        let trace = RequestTrace::begin(
            ProviderId::Gemini,
            model,
            "/generateContent google_search",
            BASE,
            false,
        );

        let body = json!({
            "contents": [ { "role": "user", "parts": [{ "text": user }] } ],
            "systemInstruction": { "parts": [{ "text": system }] },
            "generationConfig": { "temperature": 0.2 },
            "tools": [{ "google_search": {} }],
        });
        let url = format!("{BASE}{endpoint_label}");
        let resp = crate::net::http::shared()
            .post(&url)
            .timeout(timeouts::WEB_SEARCH)
            .header("x-goog-api-key", &api_key)
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                tracing::warn!("gemini research unreachable: {e}");
                return Ok(String::new());
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            tracing::warn!("gemini research {status}: {body_text}");
            return Ok(String::new());
        }
        let data: Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => {
                trace.end(Some(status.as_u16()), false);
                return Ok(String::new());
            }
        };
        trace.end(Some(status.as_u16()), true);
        Ok(join_parts_text(&data))
    }
}

#[async_trait]
impl AiProvider for GeminiClient {
    fn id(&self) -> ProviderId {
        ProviderId::Gemini
    }

    fn capabilities(&self, _model: &str) -> ModelCapabilities {
        ModelCapabilities {
            supports_temperature: true,
            supports_system_role: true, // mapped to systemInstruction
            supports_streaming: true,
            supports_reasoning: false,
            supports_tools: true,
            supports_json_mode: true,
            supports_embeddings: true,
            // Native Google Search grounding tool (account-key gated at call time).
            supports_web_search: true,
            token_param: TokenParam::MaxOutputTokens,
        }
    }

    async fn chat_stream(
        &self,
        app: &AppHandle,
        job_id: &str,
        req: &AiGenerateRequest,
    ) -> AppResult<()> {
        let api_key = require_gemini_key(app)?;
        let endpoint_label = format!("/v1beta/models/{}:streamGenerateContent", req.model);
        let trace =
            RequestTrace::begin(ProviderId::Gemini, &req.model, &endpoint_label, BASE, true);

        let temperature = req.temperature.unwrap_or(0.7);
        let system_text: String = req
            .messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let contents: Vec<Value> = req
            .messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| {
                let role = if m.role == "assistant" {
                    "model"
                } else {
                    "user"
                };
                json!({ "role": role, "parts": [{ "text": m.content }] })
            })
            .collect();

        let mut generation_config = json!({ "temperature": temperature });
        if let Some(mt) = req.max_tokens {
            generation_config["maxOutputTokens"] = json!(mt);
        }
        // Ask thinking-capable models to stream their reasoning as `thought` parts.
        if gemini_supports_thinking(&req.model) {
            generation_config["thinkingConfig"] = json!({ "includeThoughts": true });
        }
        let mut body = json!({ "contents": contents, "generationConfig": generation_config });
        if !system_text.is_empty() {
            body["systemInstruction"] = json!({ "parts": [{ "text": system_text }] });
        }

        let url = format!("{BASE}{endpoint_label}");
        let response = crate::net::http::shared()
            .post(&url)
            .timeout(timeouts::STREAM)
            .header("x-goog-api-key", &api_key)
            .json(&body)
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Gemini unreachable: {e}")));
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Gemini, status, &body_text));
        }

        // The shared loop owns cancel-check + chunk read + emit + complete; the
        // closure is the only Gemini-specific part — it scans the streamed JSON
        // array for complete top-level objects (`state` carries brace depth across
        // chunk boundaries). Gemini has no in-band done sentinel, so the loop
        // completes on end-of-body.
        let mut state = GeminiScanner::default();
        stream_response(app, job_id, &trace, response, status.as_u16(), move |buf| {
            parse_gemini_frames(buf, &mut state)
        })
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
        let api_key = require_gemini_key(app)?;
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:generateContent");
        let trace = RequestTrace::begin(ProviderId::Gemini, model, &endpoint_label, BASE, false);

        let mut body = json!({
            "contents": [ { "role": "user", "parts": [{ "text": user }] } ],
            "generationConfig": { "temperature": temperature.unwrap_or(0.7) },
        });
        if !system.is_empty() {
            body["systemInstruction"] = json!({ "parts": [{ "text": system }] });
        }

        let url = format!("{BASE}{endpoint_label}");
        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(&url)
                .timeout(timeouts::COMPLETION)
                .header("x-goog-api-key", &api_key)
                .json(&body)
        })
        .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Gemini unreachable: {e}")));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Gemini, status, &body_text));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        let text = join_parts_text(&data);
        if text.is_empty() {
            return Err(AppError::Provider(
                "Gemini: unexpected response shape".to_string(),
            ));
        }
        Ok(text)
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
        let api_key = require_gemini_key(app)?;
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:embedContent");
        let trace = RequestTrace::begin(ProviderId::Gemini, model, &endpoint_label, BASE, false);
        let body = json!({
            "model": format!("models/{m}"),
            "content": { "parts": [{ "text": text }] },
        });
        let url = format!("{BASE}{endpoint_label}");
        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(&url)
                .timeout(timeouts::EMBED)
                .header("x-goog-api-key", &api_key)
                .json(&body)
        })
        .await
        .map_err(|e| format!("Gemini unreachable: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Gemini, status, &body_text));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        data.get("embedding")
            .and_then(|e| e.get("values"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
            .ok_or_else(|| AppError::Provider("Gemini: missing embedding in response".to_string()))
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        Some("text-embedding-004")
    }

    fn max_embedding_input_chars(&self) -> usize {
        // text-embedding-004 accepts 2048 tokens (~4 chars/token ≈ 8000 chars). This
        // matches the conservative default but is stated explicitly so Gemini's real
        // cap is documented at the adapter and won't drift if the default changes.
        8_000
    }

    async fn list_models(&self, app: &AppHandle) -> Vec<Value> {
        // Returns `Vec` (no `AppResult`), so a blank key can't surface the
        // unauthorized error — short-circuit to the empty "no models" result
        // instead of wasting a 401 round-trip with an empty header.
        let api_key = match get_provider_key(app, self.id().credential_key()) {
            Some(k) if !k.trim().is_empty() => k,
            _ => return vec![],
        };
        let client = crate::net::http::shared();
        let resp = client
            .get(format!("{BASE}/v1/models"))
            .header("x-goog-api-key", &api_key)
            .timeout(timeouts::LIST_MODELS)
            .send()
            .await;
        if let Ok(r) = resp {
            if let Ok(body) = r.json::<Value>().await {
                if let Some(models) = body.get("models").and_then(|d| d.as_array()) {
                    return models
                        .iter()
                        .filter_map(|m| m.get("name").and_then(|id| id.as_str()))
                        .filter(|id| id.starts_with("models/"))
                        .map(|id| json!({ "name": id.strip_prefix("models/").unwrap_or(id) }))
                        .collect();
                }
            }
        }
        vec![]
    }

    async fn test_key(&self, app: &AppHandle) -> AppResult<()> {
        let api_key = require_gemini_key(app)?;
        let client = crate::net::http::shared();
        let resp = client
            .get(format!("{BASE}/v1/models"))
            .header("x-goog-api-key", &api_key)
            .timeout(timeouts::LIST_MODELS)
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(AppError::Provider(format!(
                "API returned status: {}",
                resp.status()
            )))
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
        if !self.capabilities(model).supports_tools {
            return single_shot_turn(self, app, model, messages, temperature).await;
        }
        let api_key = require_gemini_key(app)?;
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:generateContent");
        let trace = RequestTrace::begin(ProviderId::Gemini, model, &endpoint_label, BASE, false);

        let (system, rest) = split_system(messages);
        let contents: Vec<Value> = rest
            .iter()
            .map(|msg| {
                // Gemini's assistant role is "model"; user + (folded) tool results are "user".
                let role = if msg.role == Role::Assistant {
                    "model"
                } else {
                    "user"
                };
                json!({ "role": role, "parts": [{ "text": msg.content }] })
            })
            .collect();
        // Trusted, fixed function declarations — never built from scraped/model text.
        let function_declarations: Vec<Value> = tools
            .iter()
            .map(
                |t| json!({ "name": t.name, "description": t.description, "parameters": t.schema }),
            )
            .collect();

        let mut body = json!({
            "contents": contents,
            "generationConfig": { "temperature": temperature.unwrap_or(0.7) },
            "tools": [{ "functionDeclarations": function_declarations }],
        });
        if !system.is_empty() {
            body["systemInstruction"] = json!({ "parts": [{ "text": system }] });
        }

        let url = format!("{BASE}{endpoint_label}");
        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(&url)
                .timeout(timeouts::COMPLETION)
                .header("x-goog-api-key", &api_key)
                .json(&body)
        })
        .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Gemini unreachable: {e}")));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Gemini, status, &body_text));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        let turn = parse_gemini_turn(&data);
        // Mirror `complete()`'s empty-response guard: a missing/blocked candidate
        // (e.g. a safety block with no `candidates`) parses to blank text and no
        // tool calls. Exclude `Length` — a `MAX_TOKENS`/`MALFORMED_FUNCTION_CALL`
        // turn can legitimately have no usable text or calls yet, and that is
        // already handled by the controller's truncation path, not an error here.
        if turn.text.is_empty() && turn.tool_calls.is_empty() && turn.stop != StopReason::Length {
            return Err(AppError::Provider(
                "Gemini: unexpected response shape".to_string(),
            ));
        }
        Ok(turn)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        gemini_supports_thinking, join_parts_text, parse_gemini_frames, parse_gemini_parts,
        parse_gemini_turn, validate_gemini_key, GeminiScanner, StreamPiece,
    };
    use crate::commands::ai_provider::{StopReason, ToolCall};
    use crate::error::AppError;
    use serde_json::json;

    #[test]
    fn blank_or_missing_key_is_rejected_with_unauthorized() {
        // A missing key, an empty string, and whitespace-only must all fail fast with
        // the same unauthorized message `friendly_api_error` maps a real 401/403 to —
        // never sending an empty `x-goog-api-key` header for a wasted round-trip.
        for stored in [None, Some(String::new()), Some("   \n\t".to_string())] {
            match validate_gemini_key(stored) {
                Err(AppError::Config(msg)) => {
                    assert_eq!(msg, "gemini: invalid or unauthorized API key.")
                }
                other => panic!("expected unauthorized Config error, got {other:?}"),
            }
        }
    }

    #[test]
    fn present_key_passes_through_untrimmed() {
        // A real key is returned verbatim (surrounding content preserved, only blank
        // rejected) so the request uses exactly what the user stored.
        assert_eq!(
            validate_gemini_key(Some("AIza-secret".to_string())).unwrap(),
            "AIza-secret"
        );
    }

    #[test]
    fn join_parts_text_concatenates_first_candidate_parts() {
        let data = json!({
            "candidates": [{
                "content": { "parts": [{ "text": "Acme is " }, { "text": "a widget maker." }] },
                "groundingMetadata": { "webSearchQueries": ["Acme"] }
            }]
        });
        assert_eq!(join_parts_text(&data), "Acme is a widget maker.");
        assert_eq!(join_parts_text(&json!({})), "");
        assert_eq!(join_parts_text(&json!({ "candidates": [] })), "");
    }

    #[test]
    fn thinking_gate_enables_only_known_models() {
        for m in [
            "gemini-2.5-pro",
            "gemini-2.5-flash",
            "gemini-2.0-flash-thinking",
        ] {
            assert!(gemini_supports_thinking(m), "{m} should enable thinking");
        }
        for m in ["gemini-1.5-pro", "gemini-1.5-flash", "gemini-2.0-flash"] {
            assert!(
                !gemini_supports_thinking(m),
                "{m} must not request thinkingConfig (it 400s)"
            );
        }
    }

    #[test]
    fn parse_parts_splits_thought_from_answer() {
        let ev = json!({
            "candidates": [{
                "content": { "parts": [
                    { "text": "reasoning…", "thought": true },
                    { "text": "the answer" }
                ] }
            }]
        });
        assert_eq!(
            parse_gemini_parts(&ev),
            vec![(true, "reasoning…"), (false, "the answer")]
        );
    }

    #[test]
    fn parse_parts_empty_without_candidates() {
        assert!(parse_gemini_parts(&json!({})).is_empty());
        assert!(parse_gemini_parts(&json!({ "candidates": [] })).is_empty());
    }

    #[test]
    fn frames_parse_a_single_object_to_pieces() {
        // A self-contained object (no array wrapper) — the scanner finds it when
        // depth returns to 0 and the accumulated text starts with `{`.
        let obj = r#"{"candidates":[{"content":{"parts":[{"text":"reasoning","thought":true},{"text":"answer"}]}}]}"#;
        let mut state = GeminiScanner::default();
        let mut buf = String::from(obj);
        let pieces = parse_gemini_frames(&mut buf, &mut state);
        assert_eq!(
            pieces,
            vec![
                StreamPiece::thinking("reasoning"),
                StreamPiece::text("answer")
            ]
        );
        // The buffer is fully consumed and no partial object remains.
        assert!(buf.is_empty());
        assert!(state.pending.is_empty());
    }

    #[test]
    fn frames_reassemble_object_split_across_chunks() {
        // An object delivered in two chunks is buffered in `state.pending` until
        // complete, then emitted exactly once.
        let mut state = GeminiScanner::default();
        let mut buf = String::from(r#"{"candidates":[{"content":{"parts":[{"text":"hel"#);
        assert!(parse_gemini_frames(&mut buf, &mut state).is_empty());
        assert!(!state.pending.is_empty());
        buf.push_str(r#"lo"}]}}]}"#);
        assert_eq!(
            parse_gemini_frames(&mut buf, &mut state),
            vec![StreamPiece::text("hello")]
        );
    }

    #[test]
    fn frames_handle_braces_inside_strings() {
        // Braces inside a string value must not move the depth counter.
        let obj = r#"{"candidates":[{"content":{"parts":[{"text":"a } b { c"}]}}]}"#;
        let mut state = GeminiScanner::default();
        let mut buf = String::from(obj);
        assert_eq!(
            parse_gemini_frames(&mut buf, &mut state),
            vec![StreamPiece::text("a } b { c")]
        );
    }

    #[test]
    fn frames_emit_both_objects_in_a_json_array_payload() {
        // A realistic streamed array (`[{…},{…}]`) split across two chunks: the
        // depth-0 framing (`[`, `,`, `]`, whitespace) must be dropped so the
        // `starts_with('{')` guard fires for the second object too. Both objects'
        // text deltas must be emitted in order.
        let mut state = GeminiScanner::default();
        let mut buf =
            String::from(r#"[{"candidates":[{"content":{"parts":[{"text":"Hello"}]}}]}, {"candi"#);
        let first = parse_gemini_frames(&mut buf, &mut state);
        assert_eq!(first, vec![StreamPiece::text("Hello")]);

        buf.push_str(r#"dates":[{"content":{"parts":[{"text":" world"}]}}]}]"#);
        let second = parse_gemini_frames(&mut buf, &mut state);
        assert_eq!(second, vec![StreamPiece::text(" world")]);
        assert!(buf.is_empty());
        assert!(state.pending.is_empty());
    }

    #[test]
    fn parse_turn_extracts_function_calls_alongside_text() {
        // Gemini reports finishReason "STOP" even when it emits a functionCall — the
        // call's presence, not the finishReason, is the "wants tools back" signal.
        let data = json!({
            "candidates": [{
                "content": { "parts": [
                    { "text": "Looking up the company." },
                    { "functionCall": { "name": "research_company", "args": { "company": "Acme" } } }
                ] },
                "finishReason": "STOP"
            }]
        });
        let turn = parse_gemini_turn(&data);
        assert_eq!(turn.text, "Looking up the company.");
        assert_eq!(turn.stop, StopReason::ToolUse);
        assert_eq!(
            turn.tool_calls,
            vec![ToolCall {
                id: "research_company-1".to_string(),
                name: "research_company".to_string(),
                args: json!({ "company": "Acme" }),
            }]
        );
    }

    #[test]
    fn parse_turn_plain_answer_maps_stop_reason() {
        let data = json!({
            "candidates": [{
                "content": { "parts": [{ "text": "Final answer." }] },
                "finishReason": "STOP"
            }]
        });
        let turn = parse_gemini_turn(&data);
        assert_eq!(turn.text, "Final answer.");
        assert!(turn.tool_calls.is_empty());
        assert_eq!(turn.stop, StopReason::End);

        let truncated = json!({
            "candidates": [{ "content": { "parts": [{ "text": "..." }] }, "finishReason": "MAX_TOKENS" }]
        });
        assert_eq!(parse_gemini_turn(&truncated).stop, StopReason::Length);
    }

    #[test]
    fn parse_turn_malformed_function_call_maps_to_length_not_tool_use() {
        // A tool call truncated by the output-token limit comes back with
        // `finishReason: "MALFORMED_FUNCTION_CALL"` (NOT `MAX_TOKENS`) — it must
        // route through the same non-executable/truncated path as `MAX_TOKENS`, so
        // the (possibly half-serialized) args never reach a tool handler.
        let data = json!({
            "candidates": [{
                "content": { "parts": [
                    { "functionCall": { "name": "research_company", "args": { "company": "Ac" } } }
                ] },
                "finishReason": "MALFORMED_FUNCTION_CALL"
            }]
        });
        let turn = parse_gemini_turn(&data);
        assert_eq!(turn.stop, StopReason::Length);
    }
}
