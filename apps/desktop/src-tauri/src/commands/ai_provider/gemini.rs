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
    bounded, friendly_api_error, model_entry, pagination_step, resolve_intent, single_shot_turn,
    split_system, AgentTurn, AiGenerateRequest, AiProvider, ChatMsg, Intent, ModelCapabilities,
    PaginationStep, ProviderId, RequestTrace, Role, SamplingProfile, StopReason, TokenParam,
    ToolCall, ToolSpec, Usage,
};

const BASE: &str = "https://generativelanguage.googleapis.com";

/// Safety bound on `list_models` pagination — real catalogues are a few dozen
/// models; this just prevents an unbounded loop if the API ever returns a
/// `nextPageToken` forever.
const MAX_LIST_MODELS_PAGES: usize = 50;

/// Requested embedding output size. `gemini-embedding-2`'s default (unspecified)
/// dimensionality is 3072 — 4x the retired text-embedding-004's 768. One of
/// the model's documented "Recommended" sizes; auto-normalized by the API
/// (see `embed_impl`'s call site for the full rationale).
const EMBED_OUTPUT_DIMENSIONALITY: i64 = 768;

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
fn validate_gemini_key(stored: Option<String>) -> AppResult<String> {
    match stored.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        Some(k) => Ok(k.to_string()),
        None => Err(AppError::Config(format!(
            "{}: invalid or unauthorized API key.",
            ProviderId::Gemini.as_str()
        ))),
    }
}

/// Resolve the stored Gemini key, rejecting a missing/blank one before any request.
fn require_gemini_key(app: &AppHandle) -> AppResult<String> {
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
fn parse_model_page(body: &Value) -> AppResult<(Vec<Value>, Option<String>)> {
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

/// Whether `model` is POSITIVELY recognized as a pre-v3 Gemini id: it carries
/// the `gemini-` family prefix (after stripping an optional `models/`
/// prefix) but is NOT [`gemini_is_v3_or_later`]. Used ONLY by
/// [`GeminiClient::sampling_profile`] to decide whether ITS OWN `0.7`
/// temperature default applies — NOT by [`gemini_effective_temperature`] or
/// any other gate in this file, which stay on [`gemini_is_v3_or_later`]
/// unchanged.
///
/// An id this app cannot even recognize as belonging to the Gemini family at
/// all (a typo, a future non-`gemini-`-prefixed line, a blank/garbage
/// string) is NOT "recognized pre-v3" — mirrors
/// `anthropic_supports_temperature`'s fail-safe (`anthropic.rs`): an
/// unclassified new id defaults to "no sampling params", the direction that
/// can never 400 or trigger Gemini's documented below-1.0 degradation,
/// rather than assuming it is safe, legacy pre-v3 behavior.
fn gemini_is_recognized_pre_v3(model: &str) -> bool {
    let m = model
        .strip_prefix("models/")
        .unwrap_or(model)
        .to_ascii_lowercase();
    m.starts_with("gemini-") && !gemini_is_v3_or_later(model)
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
/// `gemini-3.1-flash-lite` — the TEXT model (Stable, live), distinct from
/// `gemini-3.1-flash-lite-image` above — is deliberately absent from this
/// table, not an oversight: re-checked against the live thinking table
/// (`ai.google.dev/gemini-api/docs/thinking`, checked 2026-08-04) and it has
/// no row there at all, unlike every model listed above. Guessing it shares
/// its `-image` sibling's (or `gemini-3.5-flash-lite`'s) level set would be
/// exactly the "guessed value that could 400" this function exists to avoid
/// — it correctly falls through to the safe universal `["high"]` default
/// below until Google's table documents it.
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
        // Includes `gemini-3.1-flash-lite` (the text model, NOT `-image`) —
        // absent from the live thinking table as of the date above, so this
        // safe universal fallback is the correct answer, not a gap. See the
        // doc comment above before "fixing" this by guessing its levels.
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

/// Whether Gemini's deprecated sampling knobs — `temperature`, `topP`, and
/// `topK` (never wired up in this file — `AiGenerateRequest` has no `top_k`
/// field, so there is nothing to gate for it) — should be withheld for
/// `model`. ONE predicate decides all of them so a future model can't end up
/// gated for one and not another: that exact drift is how `topP` shipped
/// ungated in the first place while `temperature` already had this check.
///
/// Verified against two live Google references (fetched 2026-08-05), which
/// group all three identically rather than singling out `temperature`:
/// - `ai.google.dev/gemini-api/docs/whats-new-gemini-3.5`, "Parameter updates
///   and best practices in Gemini 3.x" — explicitly scoped to "all Gemini
///   3.x models", not just the newest releases: "`temperature`, `top_p`,
///   `top_k`: we strongly recommend not changing the default values."
/// - `ai.google.dev/gemini-api/docs/latest-model#sampling-parameter-deprecation`
///   (scoped to Gemini 3.6 Flash / 3.5 Flash-Lite "and all future Gemini
///   model releases"): "`temperature`, `top_p`, and `top_k` are deprecated
///   and ignored. In future model generations, supplying these parameters
///   returns an HTTP 400 error."
///
/// Reuses [`gemini_is_v3_or_later`] — the SAME boundary `thinkingLevel`
/// gates on — rather than an enumerated model list, so a future Gemini
/// 4/5/… release inherits the gate with no code change.
fn gemini_omits_sampling_params(model: &str) -> bool {
    gemini_is_v3_or_later(model)
}

/// Effective `temperature` to send, or `None` to omit the field entirely.
/// `explicit` (the user's own choice, if any) ALWAYS wins, on every model —
/// this never overrides a deliberate value; only the app's OWN hardcoded
/// `fallback` default is gated (see [`gemini_omits_sampling_params`]).
/// Absent an explicit value, every call site in this file used to fall back
/// to its own hardcoded default (`0.7` for chat/complete, `0.2` for
/// research) regardless of model. Google's live docs
/// (`ai.google.dev/gemini-api/docs/gemini-3`, fetched 2026-08-04): "For all
/// Gemini 3 models, we strongly recommend keeping the temperature parameter
/// at its default value of `1.0`... Changing the temperature (setting it
/// below 1.0) may lead to unexpected behavior, such as looping or degraded
/// performance, particularly in complex mathematical or reasoning tasks."
/// Injecting either hardcoded default put every Gemini 3+ call (chat,
/// complete, AND research/synthesis, which is exactly the "complex
/// reasoning task" case) into the documented degradation case, including
/// every new user via the onboarding default (`gemini-3.6-flash`, itself
/// Gemini 3+). So on a gated model with no explicit value, this omits the
/// field so the API applies its OWN 1.0 default, instead of `fallback`; an
/// ungated model keeps `fallback` unchanged.
fn gemini_effective_temperature(model: &str, explicit: Option<f64>, fallback: f64) -> Option<f64> {
    explicit.or_else(|| (!gemini_omits_sampling_params(model)).then_some(fallback))
}

/// Build the `streamGenerateContent` request body for a given
/// [`AiGenerateRequest`] + resolved [`SamplingProfile`] (already merged with
/// the request's explicit numeric overrides — see [`SamplingProfile::resolve`]).
/// Pure + unit-tested. `topP`/`frequencyPenalty`/`presencePenalty` are the
/// detector-resistance sampling knobs (RAID, ACL 2024) — the v1beta API
/// supports all three on `generationConfig`, each added only when `Some`
/// (never sent as `null`). `topP` additionally never reaches a gated model
/// (see [`gemini_omits_sampling_params`]) — unlike `temperature`, Google's
/// deprecation notice covers it unconditionally (no "explicit user intent" to
/// preserve there), so a gated model omits it regardless of what `sampling`
/// carries. `frequencyPenalty`/`presencePenalty` are NOT covered by Google's
/// deprecation and stay ungated — this adapter's [`GeminiClient::sampling_profile`]
/// never declares either (no vendor-recommended band for them, unlike
/// OpenAI's), so in practice they only ever carry an explicit override.
fn build_chat_stream_body(req: &AiGenerateRequest, sampling: SamplingProfile) -> Value {
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

    let mut generation_config = json!({});
    if let Some(t) = sampling.temperature {
        generation_config["temperature"] = json!(t);
    }
    if !gemini_omits_sampling_params(&req.model) {
        if let Some(top_p) = sampling.top_p {
            generation_config["topP"] = json!(top_p);
        }
    }
    if let Some(fp) = sampling.frequency_penalty {
        generation_config["frequencyPenalty"] = json!(fp);
    }
    if let Some(pp) = sampling.presence_penalty {
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
/// **Nesting matters — and is now self-verifying, not just argued from docs.**
/// The REST reference for `models.embedContent` lists a TOP-LEVEL
/// `outputDimensionality` field as `(deprecated)`, and `EmbedContentConfig`'s
/// own JSON representation gives the nested field name as `outputDimensionality`
/// (camelCase) — this shape has been read from the live reference twice now
/// by two different reviewers who reached opposite conclusions, which is
/// exactly the failure mode "trust whoever read the docs last" has. Rather
/// than argue it a third time: proto3-JSON transcoding tolerates an unknown
/// or misplaced field with NO error, so a wrong nesting here would silently
/// return the model's full default dimension instead of
/// [`EMBED_OUTPUT_DIMENSIONALITY`] — `embed_impl` checks the ACTUAL returned
/// vector length against it and fails loudly on any mismatch, so the wire
/// shape is verified by the code at runtime on every real call, not by
/// whoever last read the docs.
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

        let mut generation_config = json!({});
        if let Some(t) = gemini_effective_temperature(model, temperature, 0.7) {
            generation_config["temperature"] = json!(t);
        }
        let mut body = json!({
            "contents": [ { "role": "user", "parts": [{ "text": user }] } ],
            "generationConfig": generation_config,
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
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Gemini, status, &body_text));
        }
        let data: Value =
            crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .map_err(|e| format!("parse: {e}"))?;
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
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Gemini, status, &body_text));
        }
        let data: Value =
            crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        let vector: Vec<f64> = data
            .get("embedding")
            .and_then(|e| e.get("values"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
            .ok_or_else(|| {
                AppError::Provider("Gemini: missing embedding in response".to_string())
            })?;
        // Self-verify the wire shape rather than trust it: see
        // `build_embed_body`'s doc comment. If `outputDimensionality` were
        // ever silently ignored (wrong nesting, a future API change, proto3
        // tolerating an unknown field), the API would return its full
        // default dimension instead of what was requested — catch that HERE,
        // loudly, instead of silently storing an oversized vector.
        if vector.len() != EMBED_OUTPUT_DIMENSIONALITY as usize {
            return Err(AppError::Provider(format!(
                "Gemini: requested a {EMBED_OUTPUT_DIMENSIONALITY}-dim embedding but the API \
                 returned {} dims — outputDimensionality was not honored",
                vector.len()
            )));
        }
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

        // 0.2 favors precision over creativity for a research brief — but on
        // Gemini 3+ this is exactly the case Google's docs warn against (see
        // `gemini_effective_temperature`'s doc comment): research/synthesis is
        // itself a "complex reasoning task", so forcing a below-1.0 value here
        // was pushing v3+ research into the documented degradation case, not
        // improving its precision. Omitted entirely on v3+ (API applies its
        // own 1.0); kept for pre-v3 models where this concern doesn't apply.
        let mut generation_config = json!({});
        if let Some(t) = gemini_effective_temperature(model, None, 0.2) {
            generation_config["temperature"] = json!(t);
        }
        let body = json!({
            "contents": [ { "role": "user", "parts": [{ "text": user }] } ],
            "systemInstruction": { "parts": [{ "text": system }] },
            "generationConfig": generation_config,
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
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            tracing::warn!("gemini research {status}: {body_text}");
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
        Ok(join_parts_text(&data))
    }

    /// The full paginated `/v1beta/models` transport — no `AppHandle`, so
    /// it's directly testable against a `wiremock::MockServer` by passing
    /// its `uri()` as `base` (production always calls this with `BASE`).
    /// Mirrors [`OpenAiClient::list_models_transport`](super::openai::OpenAiClient::list_models_transport).
    ///
    /// Loops through `pageToken` pages up to `MAX_LIST_MODELS_PAGES`,
    /// bounded by a SINGLE cumulative `total_deadline` across every page
    /// (not a fresh timeout per request) — also a parameter (production
    /// always passes `timeouts::LIST_MODELS_TOTAL`) so a test can force it
    /// to expire in milliseconds instead of 30 real seconds.
    async fn list_models_transport(
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
                    return Err(AppError::Provider(format!(
                        "{name}: model catalogue has more than {MAX_LIST_MODELS_PAGES} pages — stopped early rather than return an incomplete list"
                    )))
                }
            }
        }
        Ok(all)
    }
}

#[async_trait]
impl AiProvider for GeminiClient {
    fn id(&self) -> ProviderId {
        ProviderId::Gemini
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        ModelCapabilities {
            // Verified, not assumed: stays `true` unconditionally, including
            // for Gemini 3+. This field's established meaning across every
            // provider in this crate (see `openai.rs`/`anthropic.rs`'s own
            // `supports_temperature` gates) is "does the API REJECT this
            // field" — OpenAI's o-series 400s on it, Anthropic's adaptive-
            // thinking models 400 on it, but Gemini 3+ does not: Google's own
            // docs (`ai.google.dev/gemini-api/docs/gemini-3`) only recommend
            // AGAINST changing it from 1.0 for quality reasons, never say the
            // field itself is rejected. That quality concern is handled at
            // the send site instead (`gemini_effective_temperature`), which
            // omits an INVENTED default rather than a user's real choice —
            // this flag isn't consulted there today (unlike OpenAI/
            // Anthropic's `caps.supports_temperature` gate) and has no
            // renderer consumer either, so changing its value here wouldn't
            // move anything user-visible; it would just make it inaccurate.
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

    /// Neutral on Gemini 3+ (Google: "Remove these parameters from all
    /// requests" — see [`gemini_omits_sampling_params`]) AND neutral on any
    /// id this adapter cannot positively recognize as pre-v3
    /// ([`gemini_is_recognized_pre_v3`]) — an unclassified id defaults to "no
    /// sampling params", never a guessed-safe legacy default. ONLY a
    /// recognized pre-v3 model keeps this adapter's long-standing `0.7`
    /// temperature default — the ONE number [`gemini_effective_temperature`]
    /// used to fall back to before the renderer stopped shipping a per-task
    /// value (that function itself is UNCHANGED — still gated on the wider
    /// [`gemini_omits_sampling_params`] for its own callers, e.g.
    /// `complete_impl`). `topP`/`frequencyPenalty`/`presencePenalty` were
    /// never adapter-defaulted here (pure request pass-through, see
    /// [`build_chat_stream_body`]) and still aren't — `intent` is therefore
    /// unused on this adapter, unlike OpenAI's.
    fn sampling_profile(&self, model: &str, _intent: Intent) -> SamplingProfile {
        SamplingProfile {
            temperature: gemini_is_recognized_pre_v3(model).then_some(0.7),
            ..SamplingProfile::default()
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

        let sampling = self
            .sampling_profile(&req.model, resolve_intent(req))
            .resolve(req);
        let body = build_chat_stream_body(req, sampling);

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
            let body_text = crate::net::http::read_text_capped(
                response,
                crate::net::http::DEFAULT_MAX_BODY_BYTES,
            )
            .await
            .unwrap_or_default();
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

    async fn list_models(&self, app: &AppHandle) -> AppResult<Vec<Value>> {
        // `/v1beta`, not `/v1` — every generation path here already uses
        // `/v1beta` (see the `endpoint_label`s elsewhere in this file), and
        // `/v1` omits `-preview`/experimental models (e.g. the curated
        // Pro-tier default in `provider-meta.ts` is a `-preview` id, which
        // would silently vanish from the picker on `/v1`).
        let api_key = require_gemini_key(app)?;
        self.list_models_transport(BASE, &api_key, timeouts::LIST_MODELS_TOTAL)
            .await
    }

    async fn test_key(&self, app: &AppHandle) -> AppResult<()> {
        let api_key = require_gemini_key(app)?;
        let client = crate::net::http::shared();
        let resp = client
            .get(format!("{BASE}/v1beta/models"))
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

        let mut generation_config = json!({});
        if let Some(t) = gemini_effective_temperature(model, temperature, 0.7) {
            generation_config["temperature"] = json!(t);
        }
        let mut body = json!({
            "contents": contents,
            "generationConfig": generation_config,
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
            let body_text =
                crate::net::http::read_text_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                    .await
                    .unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(ProviderId::Gemini, status, &body_text));
        }
        let data: Value =
            crate::net::http::read_json_capped(resp, crate::net::http::DEFAULT_MAX_BODY_BYTES)
                .await
                .map_err(|e| format!("parse: {e}"))?;
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
#[path = "gemini_tests.rs"]
mod tests;
