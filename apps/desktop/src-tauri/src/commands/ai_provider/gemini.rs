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
    ToolSpec, Usage,
};

const BASE: &str = "https://generativelanguage.googleapis.com";

/// Requested embedding output size. `gemini-embedding-2`'s default (unspecified)
/// dimensionality is 3072 — 4x the retired text-embedding-004's 768. One of
/// the model's documented "Recommended" sizes; auto-normalized by the API
/// (see `embed_impl`'s call site for the full rationale).
const EMBED_OUTPUT_DIMENSIONALITY: i64 = 768;

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
        usage: parse_gemini_usage(data).unwrap_or_default(),
    }
}

/// Extract `usageMetadata.{promptTokenCount,candidatesTokenCount}` — present
/// on the non-streaming `generateContent` response and on every
/// `streamGenerateContent` chunk once Gemini starts reporting it (each repeats
/// the running total for the whole response so far, so the LAST one is
/// authoritative — the shared loop already keeps the latest). `None` when
/// absent. Pure + unit-tested.
fn parse_gemini_usage(data: &Value) -> Option<Usage> {
    let um = data.get("usageMetadata")?;
    Some(Usage {
        input_tokens: um
            .get("promptTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: um
            .get("candidatesTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
    })
}

/// Extract real token usage from an `embedContent` response. Gemini's
/// embeddings endpoint does not document a `usageMetadata` field the way
/// `generateContent` does — defensively reads the same key name in case a
/// future/regional variant sends it, but degrades to zero (never fabricated)
/// when absent, exactly like every other "provider reports nothing" case in
/// this module. `output_tokens: 0` always — an embed call has no completion
/// tokens. Pure + unit-tested.
fn parse_gemini_embed_usage(data: &Value) -> Usage {
    let input_tokens = data
        .get("usageMetadata")
        .and_then(|u| u.get("promptTokenCount"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    Usage {
        input_tokens,
        output_tokens: 0,
    }
}

/// Whether to request `thinkingConfig.includeThoughts`. Gemini 1.5 and the GA
/// 2.0 (non-thinking) models reject `thinkingConfig` with a 400, so this
/// enables it for Gemini 3+ ([`gemini_is_v3_or_later`] — the SAME v3+
/// boundary the effort feature's `thinkingLevel` shape uses; a real Gemini 3
/// id like `gemini-3-pro-preview` matches neither `"2.5"` nor `"thinking"`
/// on its own), the 2.5 family, and any explicit `*-thinking-*` model.
/// Unknown pre-3 future models simply don't surface thoughts (a graceful
/// miss, never a broken request).
fn gemini_supports_thinking(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    gemini_is_v3_or_later(model) || m.contains("2.5") || m.contains("thinking")
}

/// Whether `model` is Gemini 3 or later — the boundary where the newer
/// `thinkingConfig.thinkingLevel` enum (`MINIMAL`/`LOW`/`MEDIUM`/`HIGH`) takes
/// over from the older `thinkingConfig.thinkingBudget` integer. Verified
/// against the live REST reference (`ai.google.dev/api/generate-content`,
/// fetched 2026-08-03): "`thinkingLevel` ... Recommended for Gemini 3 or
/// later models. Use with earlier models results in an error." Parses the
/// major version number right after the `gemini-` prefix (`gemini-3-pro-
/// preview`, `gemini-3.5-flash`, `gemini-3.6-flash`, …) so a future Gemini
/// 4/5/… release is recognized with no code change, unlike a growing
/// enumerated list.
fn gemini_is_v3_or_later(model: &str) -> bool {
    let m = model
        .strip_prefix("models/")
        .unwrap_or(model)
        .to_ascii_lowercase();
    let Some(rest) = m.strip_prefix("gemini-") else {
        return false;
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().is_ok_and(|major| major >= 3)
}

/// Reasoning-effort levels Gemini 3.x accepts, PER MODEL — Google's live
/// table (`ai.google.dev/gemini-api/docs/thinking`, fetched 2026-08-04)
/// shows the accepted subset genuinely varies by model TIER, not just by
/// version (`gemini-3.1-flash-lite-image` supports only `minimal`/`high`;
/// `gemini-3.1-pro-preview` supports `low`/`medium`/`high`) — unlike every
/// other predicate in this crate, there is no clean shape rule to derive
/// this from the model id, so this is a genuine per-model lookup sourced
/// directly from that table:
///
/// | model                       | levels                          |
/// |------------------------------|---------------------------------|
/// | gemini-3.1-pro-preview         | low, medium, high                |
/// | gemini-3.1-flash-lite-image    | minimal, high                    |
/// | gemini-3-flash-preview, gemini-3.5-flash(-lite), gemini-3.6-flash | minimal, low, medium, high |
/// | gemini-3-pro-preview (SHUT DOWN — `ai.google.dev/gemini-api/docs/models`, checked 2026-08-04) | low, high |
///
/// Level acceptance is enforced POST-auth (proto/shape validation accepts
/// any `ThinkingLevel` enum member on every model — a request 400s only
/// later, on the model-specific check), so this table could not be probed
/// live without a key; treat Google's docs table as authoritative.
///
/// `gemini-3-pro-preview`'s row is kept even though the model itself is
/// shut down: a user with an already-saved config (or who types a model id
/// manually — the field is free text) still gets its real historical
/// levels instead of the generic `["high"]` fallback below, and a genuinely
/// wrong 400 is strictly worse than an accurate answer for a dead model
/// either way (the actual `embedContent`/`generateContent` call still fails
/// the SAME way regardless of what this function returns). Dropping the row
/// would be equally defensible — re-litigate if it becomes confusing rather
/// than helpful.
///
/// A model that passes [`gemini_is_v3_or_later`] but is NOT one of the rows
/// above is a genuinely new/unreleased id — falls back to `["high"]`, the
/// one level every row in the current table accepts (never a guessed value
/// that could 400; `effort: high` is also the documented no-op-equivalent
/// default on every provider in this crate, so it degrades gracefully as
/// "no override"). Pre-3 models (including the 2.5 family — see
/// `build_chat_stream_body`'s doc comment) get no levels at all.
fn gemini_effort_levels(model: &str) -> Vec<&'static str> {
    if !gemini_is_v3_or_later(model) {
        return Vec::new();
    }
    let m = model
        .strip_prefix("models/")
        .unwrap_or(model)
        .to_ascii_lowercase();
    if m.contains("gemini-3.1-flash-lite-image") {
        vec!["minimal", "high"]
    } else if m.contains("gemini-3.1-pro-preview") {
        vec!["low", "medium", "high"]
    } else if m.contains("gemini-3-pro-preview") {
        // SHUT DOWN as of 2026-08-04 (`ai.google.dev/gemini-api/docs/models`)
        // — kept for a saved/manually-typed id, see the doc comment above.
        vec!["low", "high"]
    } else if m.contains("gemini-3-flash-preview")
        || m.contains("gemini-3.5-flash")
        || m.contains("gemini-3.6-flash")
    {
        vec!["minimal", "low", "medium", "high"]
    } else {
        vec!["high"]
    }
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
                if let Some(usage) = parse_gemini_usage(&event) {
                    out.push(StreamPiece::usage(usage));
                }
            }
            state.pending.clear();
        }
    }
    out
}

/// Build the `streamGenerateContent` request body for a given
/// [`AiGenerateRequest`]. Pure + unit-tested. `topP`/`frequencyPenalty`/
/// `presencePenalty` are the detector-resistance sampling knobs (RAID, ACL
/// 2024) the renderer sets only for prose generation surfaces — the v1beta API
/// supports all three on `generationConfig`, each added only when `Some`
/// (never sent as `null`).
fn build_chat_stream_body(req: &AiGenerateRequest) -> Value {
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
    if let Some(top_p) = req.top_p {
        generation_config["topP"] = json!(top_p);
    }
    if let Some(fp) = req.frequency_penalty {
        generation_config["frequencyPenalty"] = json!(fp);
    }
    if let Some(pp) = req.presence_penalty {
        generation_config["presencePenalty"] = json!(pp);
    }
    if let Some(mt) = req.max_tokens {
        generation_config["maxOutputTokens"] = json!(mt);
    }
    // Both fields live under the SAME `thinkingConfig` object — build it
    // incrementally so setting one never clobbers the other.
    let mut thinking_config = serde_json::Map::new();
    // Ask thinking-capable models to stream their reasoning as `thought` parts.
    if gemini_supports_thinking(&req.model) {
        thinking_config.insert("includeThoughts".to_string(), json!(true));
    }
    // `thinkingLevel` is gated on Gemini 3+ ([`gemini_is_v3_or_later`]) per
    // THIS endpoint's own REST reference (`ThinkingConfig.thinkingLevel`,
    // `generateContent`/`streamGenerateContent` — what this file calls):
    // "Recommended for Gemini 3 or later models. Use with earlier models
    // results in an error." Google's newer Interactions API separately
    // publishes a level vocabulary for the 2.5 family too, but that is a
    // DIFFERENT API this app does not call — going by the reference for the
    // endpoint actually in use here, pre-3 models (2.5 and earlier) simply
    // don't get an effort-driven thinking config.
    //
    // GATED ON THE PER-MODEL LEVEL SET (`gemini_effort_levels`), not just
    // "is this Gemini 3+" — `effort` is stored PER PROVIDER, not per model
    // (`preferences-store.ts`), and nothing clears it on a model switch:
    // picking `medium` on `gemini-3.1-pro-preview` (valid there) then
    // switching to `gemini-3-pro-preview` (same provider, only accepts
    // low/high) must not ship `thinkingLevel: "MEDIUM"` to a model that
    // 400s on it. `gemini_effort_levels` already returns `[]` for a pre-3
    // model, so this one check covers both gates — an invalid/stale level
    // for the CURRENT model is silently omitted (the request still sends,
    // just without an effort override) rather than sent and rejected.
    if let Some(effort) = req
        .effort
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
    {
        if gemini_effort_levels(&req.model).contains(&effort) {
            thinking_config.insert(
                "thinkingLevel".to_string(),
                json!(effort.to_ascii_uppercase()),
            );
        }
    }
    if !thinking_config.is_empty() {
        generation_config["thinkingConfig"] = Value::Object(thinking_config);
    }
    let mut body = json!({ "contents": contents, "generationConfig": generation_config });
    if !system_text.is_empty() {
        body["systemInstruction"] = json!({ "parts": [{ "text": system_text }] });
    }
    body
}

/// Build the `embedContent` request body. Pure + unit-tested — the ONLY place
/// `outputDimensionality` is set, so a request can never silently drop it.
/// `m` is the model id with any `models/` prefix already stripped (mirrors
/// `embed_impl`'s own stripping, done once there for the endpoint label too).
///
/// Without it, `gemini-embedding-2` returns its default 3072-dim vector — 4x
/// the retired text-embedding-004's 768 — stored as JSON f64 text in both
/// `vectors` and `posting_vectors` at ~4x the space. 768 keeps ~99.5% of
/// full-dimension MTEB quality (Google's own published numbers) and is one
/// of the model's documented "Recommended" sizes; `gemini-embedding-2`
/// auto-normalizes a truncated-dimension embedding (verified in the live
/// Gemini API docs), so this needs no extra normalization step on our side.
///
/// **Nesting matters.** The REST reference for `models.embedContent` lists a
/// TOP-LEVEL `outputDimensionality` field as `(deprecated)` — "Please use
/// `EmbedContentConfig.output_dimensionality` instead" — and
/// `EmbedContentConfig`'s own JSON representation confirms the nested field
/// name is `outputDimensionality` (camelCase) — both verified against the
/// live `/api/embeddings` REST reference, not memory. The wire field is
/// therefore nested camelCase JSON: `embedContentConfig: { outputDimensionality }`,
/// NOT a snake_case field at the request root — sending the deprecated
/// top-level form would risk the API silently ignoring it (proto3-JSON
/// transcoding may accept an unknown/deprecated field with no error),
/// silently storing 3072-dim vectors again with no visible failure.
fn build_embed_body(m: &str, text: &str) -> Value {
    json!({
        "model": format!("models/{m}"),
        "content": { "parts": [{ "text": text }] },
        "embedContentConfig": { "outputDimensionality": EMBED_OUTPUT_DIMENSIONALITY },
    })
}

pub struct GeminiClient;

impl GeminiClient {
    /// Shared body of `complete`/`complete_with_usage`: one non-streaming
    /// `generateContent` call, parsed once into `(text, usage)` so the two
    /// trait methods never duplicate the HTTP round-trip.
    async fn complete_impl(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
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
        let usage = parse_gemini_usage(&data).unwrap_or_default();
        Ok((text, usage))
    }

    /// Shared body of `embed`/`embed_with_usage`: one `embedContent` call,
    /// parsed once into `(vector, usage)` so the two trait methods never
    /// duplicate the HTTP round-trip.
    async fn embed_impl(
        &self,
        app: &AppHandle,
        model: &str,
        text: &str,
    ) -> AppResult<(Vec<f64>, Usage)> {
        let api_key = require_gemini_key(app)?;
        let m = model.strip_prefix("models/").unwrap_or(model);
        let endpoint_label = format!("/v1beta/models/{m}:embedContent");
        let trace = RequestTrace::begin(ProviderId::Gemini, model, &endpoint_label, BASE, false);
        let body = build_embed_body(m, text);
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
        let vector: Vec<f64> = data
            .get("embedding")
            .and_then(|e| e.get("values"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
            .ok_or_else(|| {
                AppError::Provider("Gemini: missing embedding in response".to_string())
            })?;
        Ok((vector, parse_gemini_embed_usage(&data)))
    }

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

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        ModelCapabilities {
            supports_temperature: true,
            supports_system_role: true, // mapped to systemInstruction
            supports_streaming: true,
            supports_reasoning: gemini_is_v3_or_later(model),
            supports_tools: true,
            supports_json_mode: true,
            supports_embeddings: true,
            // Native Google Search grounding tool (account-key gated at call time).
            supports_web_search: true,
            token_param: TokenParam::MaxOutputTokens,
        }
    }

    fn effort_levels(&self, model: &str) -> Vec<&'static str> {
        gemini_effort_levels(model)
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

        let body = build_chat_stream_body(req);

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
        stream_response(
            app,
            job_id,
            &trace,
            response,
            status.as_u16(),
            ProviderId::Gemini,
            &req.model,
            BASE,
            move |buf| parse_gemini_frames(buf, &mut state),
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
        // text-embedding-004 was retired (shutdown Jan 14, 2026 — the exact
        // error this app was seeing). Google's own deprecation table names
        // `gemini-embedding-2` as the migration target for every retired
        // embedding model (verified via the live Gemini API docs, not memory).
        Some("gemini-embedding-2")
    }

    fn max_embedding_input_chars(&self) -> usize {
        // gemini-embedding-2's documented input limit is 8,192 tokens (~4
        // chars/token ≈ 32000 chars for English). Cap conservatively at 8000
        // chars: in the worst case (token-dense scripts, ~1 char/token) that
        // still stays under 8,192 tokens for every language. This is the
        // per-CHUNK size `embed_adaptive` uses — a document longer than this
        // is split into multiple chunks and mean-pooled (never truncated
        // away), and `embed_chunk_adaptive` halves-and-retries a single chunk
        // on an actual context-length error, so this default only needs to be
        // a safe starting point, not a perfect guess.
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
        build_chat_stream_body, build_embed_body, gemini_effort_levels, gemini_is_v3_or_later,
        gemini_supports_thinking, join_parts_text, parse_gemini_embed_usage, parse_gemini_frames,
        parse_gemini_parts, parse_gemini_turn, parse_gemini_usage, validate_gemini_key, AiProvider,
        GeminiClient, GeminiScanner, StreamPiece, EMBED_OUTPUT_DIMENSIONALITY,
    };
    use crate::commands::ai_provider::{AiGenerateRequest, StopReason, ToolCall};
    use crate::error::AppError;
    use crate::ipc_contracts::ai::AiGenerateRequestMessage;
    use serde_json::json;

    fn base_request() -> AiGenerateRequest {
        AiGenerateRequest {
            model: "gemini-1.5-flash".to_string(),
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
            effort: None,
        }
    }

    #[test]
    fn chat_stream_body_serializes_sampling_params_when_set() {
        let mut req = base_request();
        req.top_p = Some(0.95);
        req.frequency_penalty = Some(0.3);
        req.presence_penalty = Some(0.2);
        let body = build_chat_stream_body(&req);
        let config = &body["generationConfig"];
        assert_eq!(config["topP"], json!(0.95));
        assert_eq!(config["frequencyPenalty"], json!(0.3));
        assert_eq!(config["presencePenalty"], json!(0.2));
    }

    #[test]
    fn chat_stream_body_omits_sampling_params_when_none() {
        let body = build_chat_stream_body(&base_request());
        let config = &body["generationConfig"];
        assert!(config.get("topP").is_none());
        assert!(config.get("frequencyPenalty").is_none());
        assert!(config.get("presencePenalty").is_none());
    }

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
    fn default_embedding_model_is_not_the_retired_text_embedding_004() {
        // text-embedding-004 was retired by Google (shutdown Jan 14, 2026) —
        // the exact "model or endpoint not found" error this app was seeing.
        let model = GeminiClient.default_embedding_model().unwrap();
        assert_ne!(model, "text-embedding-004");
        assert_eq!(model, "gemini-embedding-2");
    }

    #[test]
    fn embed_body_requests_the_reduced_output_dimensionality() {
        // Without this, gemini-embedding-2 defaults to 3072 dims (4x the
        // retired text-embedding-004's 768), quadrupling stored-vector size
        // for no accuracy benefit this app uses. Must be NESTED inside
        // `embedContentConfig` (camelCase) — the top-level `outputDimensionality`
        // field is deprecated per the live REST reference and may be silently
        // ignored.
        let body = build_embed_body("gemini-embedding-2", "hello");
        assert_eq!(
            body["embedContentConfig"]["outputDimensionality"],
            json!(EMBED_OUTPUT_DIMENSIONALITY)
        );
        assert_eq!(EMBED_OUTPUT_DIMENSIONALITY, 768);
        // Never at the deprecated top-level location.
        assert!(body.get("output_dimensionality").is_none());
        assert!(body.get("outputDimensionality").is_none());
        assert_eq!(body["model"], json!("models/gemini-embedding-2"));
        assert_eq!(body["content"]["parts"][0]["text"], json!("hello"));
    }

    #[test]
    fn embedding_cap_is_within_the_documented_token_limit_range() {
        // Drift-pinning only, NOT proof of token safety on its own (that would
        // need a real tokenizer, which this crate doesn't have) — it just
        // pins the char cap to a sane range relative to gemini-embedding-2's
        // documented 8,192-token limit, so a future edit can't silently set
        // it absurdly high or low. The real per-language safety net is
        // `embed_chunk_adaptive`'s halve-and-retry on an actual provider
        // context-length error, which this test does not exercise.
        let cap = GeminiClient.max_embedding_input_chars();
        assert!(cap <= 8192, "char cap {cap} can exceed 8192 tokens");
        assert!(cap >= 4_000, "cap {cap} truncates too aggressively");
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
            // Real Gemini 3 ids match neither "2.5" nor "thinking" on their
            // own — must be reached via the v3+ boundary, not the substrings.
            "gemini-3-pro-preview",
            "gemini-3-flash-preview",
            "gemini-3.6-flash",
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
    fn parse_usage_reads_prompt_and_candidates_token_counts() {
        let data = json!({ "usageMetadata": { "promptTokenCount": 55, "candidatesTokenCount": 22, "totalTokenCount": 77 } });
        let usage = parse_gemini_usage(&data).expect("usage present");
        assert_eq!(usage.input_tokens, 55);
        assert_eq!(usage.output_tokens, 22);
    }

    #[test]
    fn parse_usage_is_none_when_absent() {
        assert!(parse_gemini_usage(&json!({})).is_none());
    }

    #[test]
    fn parse_embed_usage_reads_prompt_token_count_when_present() {
        let data = json!({ "usageMetadata": { "promptTokenCount": 7 } });
        let usage = parse_gemini_embed_usage(&data);
        assert_eq!(usage.input_tokens, 7);
        assert_eq!(usage.output_tokens, 0);
    }

    #[test]
    fn parse_embed_usage_zero_when_absent() {
        // Gemini's embedContent response typically carries no usageMetadata —
        // must degrade to zero, never fabricate a token count.
        let usage = parse_gemini_embed_usage(&json!({ "embedding": { "values": [0.1] } }));
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
    }

    #[test]
    fn frames_emit_a_usage_piece_when_usage_metadata_is_present() {
        let obj = r#"{"candidates":[{"content":{"parts":[{"text":"answer"}]}}],"usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":5}}"#;
        let mut state = GeminiScanner::default();
        let mut buf = String::from(obj);
        let pieces = parse_gemini_frames(&mut buf, &mut state);
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0], StreamPiece::text("answer"));
        let usage_piece = &pieces[1];
        assert!(usage_piece.usage.is_some());
        let usage = usage_piece.usage.unwrap();
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 5);
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

    #[test]
    fn v3_gate_recognizes_the_current_and_future_gemini_3_family() {
        for m in [
            "gemini-3-pro-preview",
            "gemini-3-flash-preview",
            "gemini-3.1-pro-preview",
            "gemini-3.5-flash",
            "gemini-3.6-flash",
            "models/gemini-3-pro-preview",
            "gemini-4-pro", // not-yet-released — must degrade forward, not backward
        ] {
            assert!(gemini_is_v3_or_later(m), "{m} should be v3+");
        }
        for m in [
            "gemini-1.5-pro",
            "gemini-1.5-flash",
            "gemini-2.0-flash",
            "gemini-2.5-pro",
            "gemini-2.5-flash",
            "not-a-gemini-model",
        ] {
            assert!(!gemini_is_v3_or_later(m), "{m} should not be v3+");
        }
    }

    #[test]
    fn capabilities_supports_reasoning_mirrors_the_v3_gate() {
        assert!(
            GeminiClient
                .capabilities("gemini-3-pro-preview")
                .supports_reasoning
        );
        assert!(
            !GeminiClient
                .capabilities("gemini-2.5-pro")
                .supports_reasoning
        );
    }

    #[test]
    fn chat_stream_body_sends_thinking_level_for_a_v3_model_with_effort_set() {
        // gemini-3.1-pro-preview — LIVE, Preview status
        // (`ai.google.dev/gemini-api/docs/models`, checked 2026-08-04).
        let mut req = base_request();
        req.model = "gemini-3.1-pro-preview".to_string();
        req.effort = Some("low".to_string());
        let body = build_chat_stream_body(&req);
        assert_eq!(
            body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
            json!("LOW")
        );
    }

    #[test]
    fn chat_stream_body_omits_thinking_level_invalid_for_the_current_model_tier() {
        // The reported model-switch scenario: `effort` is stored PER PROVIDER
        // (`preferences-store.ts`), not per model, and nothing clears it on a
        // model switch. "medium" is valid for gemini-3.1-pro-preview but NOT
        // for gemini-3.1-flash-lite-image (minimal/high only — LIVE, Stable
        // status, `ai.google.dev/gemini-api/docs/models`, checked
        // 2026-08-04) — both are Gemini 3+, so gating on
        // `gemini_is_v3_or_later` alone would ship an invalid level and 400.
        // Must omit `thinkingLevel` entirely rather than send a level the
        // CURRENT model rejects.
        let mut req = base_request();
        req.model = "gemini-3.1-flash-lite-image".to_string();
        req.effort = Some("medium".to_string());
        let body = build_chat_stream_body(&req);
        assert!(
            body["generationConfig"]["thinkingConfig"]
                .get("thinkingLevel")
                .is_none(),
            "medium is invalid for gemini-3.1-flash-lite-image (minimal/high only) — must not be sent"
        );
    }

    #[test]
    fn chat_stream_body_omits_thinking_level_for_a_pre_v3_model_even_with_effort_set() {
        // `thinkingLevel` is documented "Recommended for Gemini 3 or later
        // models. Use with earlier models results in an error" on THIS
        // endpoint's own REST reference — a pre-v3 model (2.5 and earlier)
        // must never get thinkingLevel/thinkingBudget.
        let mut req = base_request();
        req.model = "gemini-2.5-pro".to_string();
        req.effort = Some("low".to_string());
        let body = build_chat_stream_body(&req);
        assert!(body["generationConfig"]["thinkingConfig"]
            .get("thinkingLevel")
            .is_none());
    }

    #[test]
    fn chat_stream_body_omits_thinking_config_for_a_non_thinking_model_with_no_effort() {
        let mut req = base_request();
        // Pre-v3, non-2.5, non-"*-thinking-*" — genuinely no thinking support.
        req.model = "gemini-2.0-flash".to_string();
        let body = build_chat_stream_body(&req);
        assert!(body["generationConfig"].get("thinkingConfig").is_none());
    }

    #[test]
    fn chat_stream_body_keeps_include_thoughts_alongside_thinking_level() {
        // A real Gemini 3 id matches BOTH the (now-widened) "thinking" display
        // gate and the v3 effort gate — both fields must land in the SAME
        // thinkingConfig object, not one clobbering the other. "high" is the
        // one level every row in `gemini_effort_levels`'s table accepts, so
        // it's valid for gemini-3.1-pro-preview specifically too.
        // gemini-3.1-pro-preview — LIVE, Preview status
        // (`ai.google.dev/gemini-api/docs/models`, checked 2026-08-04).
        let mut req = base_request();
        req.model = "gemini-3.1-pro-preview".to_string();
        req.effort = Some("high".to_string());
        let body = build_chat_stream_body(&req);
        let tc = &body["generationConfig"]["thinkingConfig"];
        assert_eq!(tc["includeThoughts"], json!(true));
        assert_eq!(tc["thinkingLevel"], json!("HIGH"));
    }

    #[test]
    fn effort_levels_are_looked_up_per_model_tier_not_per_provider() {
        // gemini-3-pro-preview is SHUT DOWN (checked 2026-08-04) — this
        // assertion is intentional, not stale: it locks in the row's kept
        // historical value (see the doc comment on `gemini_effort_levels`),
        // not a claim the model is selectable.
        assert_eq!(
            gemini_effort_levels("gemini-3-pro-preview"),
            vec!["low", "high"]
        );
        assert_eq!(
            gemini_effort_levels("gemini-3.1-pro-preview"),
            vec!["low", "medium", "high"]
        );
        assert_eq!(
            gemini_effort_levels("gemini-3.1-flash-lite-image"),
            vec!["minimal", "high"]
        );
        assert_eq!(
            gemini_effort_levels("gemini-3-flash-preview"),
            vec!["minimal", "low", "medium", "high"]
        );
        assert_eq!(
            gemini_effort_levels("gemini-3.6-flash"),
            vec!["minimal", "low", "medium", "high"]
        );
        // An unrecognized future v3+ id falls back to the one universally-safe
        // level, never a guess that could 400.
        assert_eq!(gemini_effort_levels("gemini-4-pro"), vec!["high"]);
        // Pre-v3 models get no levels at all.
        assert!(gemini_effort_levels("gemini-2.5-pro").is_empty());
        assert!(gemini_effort_levels("gemini-1.5-flash").is_empty());
    }

    #[test]
    fn capabilities_effort_levels_matches_the_free_function() {
        assert_eq!(
            GeminiClient.effort_levels("gemini-3-pro-preview"),
            gemini_effort_levels("gemini-3-pro-preview")
        );
    }
}
