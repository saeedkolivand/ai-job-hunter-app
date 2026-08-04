//! Anthropic provider — Messages API only.

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
    ChatMsg, ModelCapabilities, ProviderId, RequestTrace, StopReason, TokenParam, ToolCall,
    ToolSpec, Usage,
};

const BASE: &str = "https://api.anthropic.com/v1";
const VERSION: &str = "2023-06-01";

/// Whether a model should be sent the classic `thinking: {type:"enabled",
/// budget_tokens}` block (extended thinking).
///
/// Anthropic's extended-thinking mode forces `temperature=1.0` and consumes extra
/// output tokens; a model that does **not** support it answers a `thinking`
/// request with a 400. Only the Claude 3.7+ / 4.x families (incl. Haiku 4.5) use
/// this classic budget-token mechanism, so gate on the model id (mirrors
/// [`gemini_supports_thinking`](super::gemini)). Older 3.0–3.5 models are
/// excluded (never supported it).
///
/// `claude-opus-4-7` and `claude-opus-4-8` are **deliberately excluded** even
/// though they match the `claude-opus-4` substring below: per Anthropic's
/// thinking docs they are adaptive-only models ("Extended thinking: No" in the
/// per-model table) — see [`anthropic_uses_adaptive_thinking`] instead.
///
/// The Claude 5 family (`claude-opus-5`, `claude-sonnet-5`, `claude-fable-5`,
/// Mythos, and later adaptive families) is excluded too: it replaced classic
/// budget-token thinking with adaptive thinking, which this predicate does not
/// gate — see [`anthropic_uses_adaptive_thinking`]. Unknown future names
/// default to **off** — a graceful miss (no thinking) is always safe; a
/// wrongful `thinking` request 400s the whole generation.
fn anthropic_supports_thinking(model: &str) -> bool {
    let m = normalize_model_id(model);
    // Opus 4.7/4.8 are adaptive-only (see doc comment) — carve them out before
    // the "claude-opus-4" substring below would otherwise catch them.
    if contains_version_needle(&m, "opus-4-7") || contains_version_needle(&m, "opus-4-8") {
        return false;
    }
    // Claude 3.7 (the first extended-thinking model) and the 4.x families.
    // Deliberately does NOT match "claude-opus-5"/"claude-sonnet-5"/
    // "claude-fable-5"/mythos — the 5 family (and later adaptive families)
    // use adaptive thinking, not this mechanism. Do not widen this to a bare
    // "claude-" match.
    contains_version_needle(&m, "claude-3-7")
        || contains_version_needle(&m, "claude-4")
        || contains_version_needle(&m, "claude-opus-4")
        || contains_version_needle(&m, "claude-sonnet-4")
        || contains_version_needle(&m, "claude-haiku-4")
}

/// Whether a model should be sent Anthropic's **adaptive** thinking block
/// (`thinking: {"type":"adaptive","display":"summarized"}`): `claude-opus-4-7`,
/// `claude-opus-4-8` (adaptive-only per the extended-thinking per-model table),
/// and the Claude 5 family (`claude-opus-5`, `claude-sonnet-5`, `claude-fable-5`,
/// Mythos) and later adaptive families.
///
/// `display` defaults to `"omitted"` on every one of these models (per
/// Anthropic's thinking docs) — an empty `thinking` field, signature only. We
/// opt into `"summarized"` explicitly so the app's thinking view actually
/// receives text; without this, "sending nothing extra" silently regresses the
/// thinking view to blank on every adaptive model, even though the model is
/// still thinking (and billing for it) under the hood. Every model matched
/// here 400s on ANY non-default temperature/top_p/top_k, on every request,
/// not just while thinking (Anthropic's "Sampling parameters" note) — see
/// [`anthropic_supports_temperature`], which builds on this predicate.
///
/// New adaptive families need a new substring added here — this predicate IS
/// the adapter's model-classification layer (the zero-change rule protects
/// business logic/callers, not this file). Unknown names default to **off** —
/// a graceful miss (no adaptive block, `display` stays omitted) is always
/// safe; guessing wrong never 400s *here* (unlike the classic gate above),
/// since adaptive thinking is already on by default on every model in this
/// set — but see [`anthropic_supports_temperature`] for the fail-safe that
/// keeps a wrong guess from 400ing on the sampling-parameter side either.
fn anthropic_uses_adaptive_thinking(model: &str) -> bool {
    let m = normalize_model_id(model);
    contains_version_needle(&m, "opus-4-7")
        || contains_version_needle(&m, "opus-4-8")
        || contains_version_needle(&m, "opus-5")
        || contains_version_needle(&m, "sonnet-5")
        || contains_version_needle(&m, "fable-5")
        || m.contains("mythos")
}

/// Shared normalization for the two thinking-mode predicates above:
/// **strip a vendor prefix** (an OpenRouter-style `anthropic/claude-...` id —
/// keep only the segment after the last `/`, so a vendor-prefixed id is
/// classified identically to its bare form on every predicate, including
/// [`anthropic_supports_temperature`]'s new-family fail-safe, which otherwise
/// silently disarms on a prefixed id since it no longer starts with
/// `"claude-"`), then lowercase, then collapse dot-form version separators to
/// dashes, so a model id spelled `claude-opus-4.7` (dot form) still matches
/// the `opus-4-7` needle instead of falling through to the classic
/// `claude-opus-4` gate and 400ing (adaptive models reject the classic
/// `thinking.enabled` shape).
fn normalize_model_id(model: &str) -> String {
    let bare = model.rsplit('/').next().unwrap_or(model);
    bare.to_ascii_lowercase().replace('.', "-")
}

/// Boundary-aware substring check for the version needles used by the
/// thinking-mode predicates above: `haystack` must contain `needle`, and the
/// character immediately following the match must be either end-of-string or
/// a non-digit. A raw [`str::contains`] has no such boundary — it would let
/// `opus-4-70`/`opus-4-71`/… wrongly match the `opus-4-7` needle, and
/// `sonnet-50`/`sonnet-58`/… wrongly match `sonnet-5`, exactly the class of
/// prefix-collision bug this file already patched once with the explicit
/// opus-4-7/4-8 carve-out above `claude-opus-4`.
fn contains_version_needle(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(idx, _)| {
        let after = idx + needle.len();
        haystack
            .as_bytes()
            .get(after)
            .is_none_or(|b| !b.is_ascii_digit())
    })
}

/// True for Anthropic's pre-thinking-era ids — Claude 1.x, 2.x, and 3.x below
/// 3.7 (`claude-3-7` itself already matches [`anthropic_supports_thinking`],
/// so anything reaching this check with a "claude-3" marker is guaranteed to
/// be below 3.7). This is a closed, historical set — Anthropic will never
/// ship a NEW model under these version numbers — so hardcoding it is safe
/// and needs no maintenance for future releases. Its only purpose is keeping
/// [`anthropic_supports_temperature`]'s new-family fail-safe from misfiring
/// on these long-shipped, well-understood models, which have always accepted
/// a normal `temperature` (they simply predate thinking, unlike a genuinely
/// unclassified NEW family).
fn anthropic_is_legacy_pre_thinking(model: &str) -> bool {
    let m = normalize_model_id(model);
    m.contains("claude-3")
        || m.contains("claude-2")
        || m.contains("claude-1")
        || m.contains("claude-instant")
}

/// Whether Anthropic will accept a non-default `temperature`/`top_p` on
/// `model` at all — the single source of truth for both
/// [`AnthropicClient::capabilities`]'s `supports_temperature` field AND every
/// request builder in this file (computed once here rather than re-derived
/// independently in two places that could drift out of sync).
///
/// `false` for every adaptive-thinking model ([`anthropic_uses_adaptive_thinking`]
/// — 400s on ANY non-default value, on every request). Also `false`, as a
/// **fail-safe**, for a `claude-`-prefixed id that matches NEITHER thinking
/// classification AND isn't a known [`anthropic_is_legacy_pre_thinking`]
/// model: that combination means a genuinely new Anthropic family this
/// adapter hasn't learned a needle for yet — since sending a non-default
/// temperature 400s on an adaptive model but omitting it is ALWAYS accepted,
/// defaulting an unclassified NEW Claude id to "no temperature" is the
/// direction that can never 400, which is what restores the zero-code-change
/// promise for a new model family. A legacy pre-thinking id (definitely safe
/// — proven by years of unchanged behavior) and a non-`claude-` id (never a
/// real Anthropic model id) both keep the old always-on behavior.
fn anthropic_supports_temperature(model: &str) -> bool {
    if anthropic_uses_adaptive_thinking(model) {
        return false;
    }
    if !anthropic_supports_thinking(model)
        && normalize_model_id(model).starts_with("claude-")
        && !anthropic_is_legacy_pre_thinking(model)
    {
        return false;
    }
    true
}

/// Whether `model` accepts the `output_config.effort` parameter — verified
/// against Anthropic's live docs
/// (`platform.claude.com/docs/en/build-with-claude/effort`, fetched
/// 2026-08-03): "The effort parameter is supported by Claude Fable 5, Claude
/// Mythos 5, Claude Opus 5, Claude Opus 4.8, Claude Mythos Preview, Claude
/// Opus 4.7, Claude Opus 4.6, Claude Sonnet 5, Claude Sonnet 4.6, and Claude
/// Opus 4.5." This is a DIFFERENT (larger) set than
/// [`anthropic_uses_adaptive_thinking`] — that same page: "On Claude Opus
/// 4.5, the only extended-thinking-only model that supports effort" — and
/// Opus/Sonnet 4.6 support effort without being adaptive-thinking models at
/// all, so effort support can't be derived from either existing thinking
/// predicate. A closed, explicitly-verified list (mirrors this file's other
/// version-needle gates) — an unrecognized future model defaults to `false`
/// (never a guessed value; `output_config.effort` 400s on a model that
/// doesn't support it). Every needle (including the two Mythos names below)
/// is boundary-checked via [`contains_version_needle`] — unlike
/// [`anthropic_uses_adaptive_thinking`]'s pre-existing bare `m.contains("mythos")`
/// (a different, broader predicate this one deliberately does NOT reuse),
/// this gate never guesses `true` for an unlisted/future model: only the
/// two Mythos names the effort page currently documents ("Claude Mythos 5",
/// "Claude Mythos Preview") match.
fn anthropic_supports_effort(model: &str) -> bool {
    let m = normalize_model_id(model);
    contains_version_needle(&m, "opus-4-5")
        || contains_version_needle(&m, "opus-4-6")
        || contains_version_needle(&m, "opus-4-7")
        || contains_version_needle(&m, "opus-4-8")
        || contains_version_needle(&m, "sonnet-4-6")
        || contains_version_needle(&m, "sonnet-5")
        || contains_version_needle(&m, "opus-5")
        || contains_version_needle(&m, "fable-5")
        || contains_version_needle(&m, "mythos-5")
        || contains_version_needle(&m, "mythos-preview")
}

/// Concatenate every `type:"text"` block in an Anthropic Messages `content` array
/// into one string (web-search responses interleave `server_tool_use` /
/// `web_search_tool_result` blocks, which have no `text` field and are skipped).
/// Pure + unit-tested.
fn join_text_blocks(data: &Value) -> String {
    data.get("content")
        .and_then(|c| c.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

/// Parse a non-streaming Anthropic Messages response into an [`AgentTurn`]:
/// concatenate the `type:"text"` blocks for the visible text, map every
/// `type:"tool_use"` block to a [`ToolCall`] (`id`, `name`, `input`→`args`), and
/// map `stop_reason` (`tool_use`→ToolUse, `end_turn`→End, `max_tokens`→Length,
/// else Other). Pure + unit-tested — this is the error-prone per-vendor shape, so
/// it lives here with no I/O.
fn parse_anthropic_turn(data: &Value) -> AgentTurn {
    let text = join_text_blocks(data);
    let tool_calls = data
        .get("content")
        .and_then(|c| c.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
                .filter_map(|b| {
                    let name = b.get("name").and_then(|n| n.as_str())?.to_string();
                    Some(ToolCall {
                        id: b
                            .get("id")
                            .and_then(|i| i.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        name,
                        args: b.get("input").cloned().unwrap_or_else(|| json!({})),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let stop = match data.get("stop_reason").and_then(|s| s.as_str()) {
        Some("tool_use") => StopReason::ToolUse,
        Some("end_turn") => StopReason::End,
        Some("max_tokens") => StopReason::Length,
        _ => StopReason::Other,
    };
    AgentTurn {
        text,
        tool_calls,
        stop,
        usage: parse_anthropic_usage(data),
    }
}

/// Drain complete SSE lines from the accumulated stream buffer into
/// [`StreamPiece`]s. Anthropic emits paired `event:`/`data:` lines; we track the
/// most recent `event:` in `last_event` (carried across chunk boundaries by the
/// caller). `message_stop` (by event name or embedded `type`) yields a terminal
/// sentinel; `thinking_delta` / `text_delta` map to reasoning / answer pieces.
///
/// Real token usage (`crate::spend`) arrives split across two events:
/// `message_start` carries `message.usage.input_tokens` (once, at the top of
/// the stream) and each `message_delta` carries a running `usage.output_tokens`
/// total (the LAST one is authoritative). `usage` is caller-carried mutable
/// state (like `last_event`) so the two halves combine into one [`Usage`]; a
/// [`StreamPiece::usage`] piece is emitted whenever either half updates.
///
/// Pure + unit-tested; this is the OpenAI-style `parse` closure for Anthropic, so
/// its SSE framing lives here only.
fn parse_anthropic_frames(
    buf: &mut String,
    last_event: &mut String,
    usage: &mut Usage,
) -> Vec<StreamPiece> {
    let mut out = Vec::new();
    // Walk the buffer by a `consumed` offset and `drain(..consumed)` once at the end,
    // instead of reallocating the whole tail per line (O(n²) on a big frame).
    let mut consumed = 0;
    while let Some(rel) = buf[consumed..].find('\n') {
        let nl = consumed + rel;
        let line = buf[consumed..nl].trim().to_string();
        consumed = nl + 1;

        if let Some(event) = line.strip_prefix("event: ") {
            *last_event = event.trim().to_string();
            continue;
        }
        let data = match line.strip_prefix("data: ") {
            Some(d) => d.trim(),
            None => continue,
        };
        if last_event == "message_stop" || data.contains("\"type\":\"message_stop\"") {
            buf.drain(..consumed);
            out.push(StreamPiece::done(""));
            return out;
        }
        let event: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match last_event.as_str() {
            "message_start" => {
                if let Some(input) = event
                    .get("message")
                    .and_then(|m| m.get("usage"))
                    .and_then(|u| u.get("input_tokens"))
                    .and_then(|v| v.as_u64())
                {
                    usage.input_tokens = input as u32;
                    out.push(StreamPiece::usage(*usage));
                }
            }
            "message_delta" => {
                if let Some(output) = event
                    .get("usage")
                    .and_then(|u| u.get("output_tokens"))
                    .and_then(|v| v.as_u64())
                {
                    usage.output_tokens = output as u32;
                    out.push(StreamPiece::usage(*usage));
                }
            }
            _ => {}
        }
        let delta_obj = event.get("delta");
        let delta_type = delta_obj
            .and_then(|d| d.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("");
        match delta_type {
            "thinking_delta" => {
                let thinking = delta_obj
                    .and_then(|d| d.get("thinking"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if !thinking.is_empty() {
                    out.push(StreamPiece::thinking(thinking));
                }
            }
            "text_delta" => {
                let text = delta_obj
                    .and_then(|d| d.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");
                if !text.is_empty() {
                    out.push(StreamPiece::text(text));
                }
            }
            _ => {}
        }
    }
    // Drop the fully-parsed prefix once; the partial trailing line stays buffered.
    buf.drain(..consumed);
    out
}

/// Build the `/messages` streaming request body for a given [`AiGenerateRequest`]
/// (mirrors `openai.rs`'s `build_chat_stream_body`'s `caps.supports_temperature`
/// gate, via [`anthropic_supports_temperature`]). Pure + unit-tested. `top_p`
/// is Anthropic's only sampling knob beyond temperature (no frequency/presence
/// penalty in this API) — set only when the caller supplied it (prose
/// surfaces), and only when [`anthropic_supports_temperature`] (false for
/// every adaptive-thinking model, and for an unrecognized `claude-`-prefixed
/// id — see its doc comment): those models reject *any* non-default
/// `temperature`/`top_p`/`top_k` on every request per Anthropic's docs
/// ("Sampling parameters"), and we don't know each model's own default value,
/// so omitting the field entirely is the only universally-safe choice.
/// Classic extended thinking also omits `temperature` (Anthropic forces it to
/// 1.0 internally; omitting IS that default, and is one fewer place to get
/// the number wrong) and never gets `top_p` (400s alongside `thinking`).
fn build_chat_stream_body(req: &AiGenerateRequest) -> Value {
    let max_tokens = req.max_tokens.unwrap_or(4096);

    let system_content: String = req
        .messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let messages: Vec<Value> = req
        .messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();

    // Thinking-token budget headroom: on BOTH classic and adaptive models,
    // thinking tokens are billed as output tokens and count toward
    // `max_tokens` alongside the visible response, so `max_tokens` must be
    // inflated to leave room for both.
    //
    // Classic thinking is gated on `max_tokens >= 2048` (a "big enough task to
    // warrant it" heuristic, not a support check) — below that, no `thinking`
    // key is sent and no inflation happens (always safe; a wrongful `thinking`
    // block 400s the whole generation on classic-only models).
    //
    // Adaptive thinking has NO such gate: it's on by default (several models
    // can't even disable it) and always counts toward `max_tokens` regardless
    // of how small the caller's budget is — a caller like the extension
    // bridge's answer-assist flow (`max_tokens: 1000`) would otherwise get
    // summarized-thinking tokens billed out of an un-inflated 1000-token cap
    // and come back with a short/empty draft. Reuses [`adaptive_max_tokens`]
    // so the streaming and non-streaming builders share one formula.
    let is_classic = anthropic_supports_thinking(&req.model);
    let is_adaptive = anthropic_uses_adaptive_thinking(&req.model);
    let classic_thinking_budget = if is_classic && max_tokens >= 2048 {
        max_tokens / 2
    } else {
        0
    };
    let actual_max_tokens = if is_adaptive {
        adaptive_max_tokens(&req.model, max_tokens)
    } else {
        max_tokens.saturating_add(classic_thinking_budget)
    };

    let mut body = json!({
        "model": req.model,
        "messages": messages,
        "max_tokens": actual_max_tokens,
        "stream": true,
    });
    // Adaptive checked FIRST: a future model id that (incorrectly) matches
    // both predicates must fail toward the safe adaptive shape, not toward
    // the classic shape that 400s on an adaptive-only model.
    if is_adaptive {
        // Opt into "summarized" display — it defaults to "omitted" (empty
        // thinking blocks) on every adaptive model, which would silently
        // blank the app's thinking view. `temperature`/`top_p` stay omitted
        // (see fn doc comment above; `anthropic_supports_temperature` is
        // false for every adaptive model).
        body["thinking"] = json!({ "type": "adaptive", "display": "summarized" });
    } else if is_classic && classic_thinking_budget > 0 {
        body["thinking"] = json!({ "type": "enabled", "budget_tokens": classic_thinking_budget });
    } else if anthropic_supports_temperature(&req.model) {
        body["temperature"] = json!(req.temperature.unwrap_or(0.7));
        if let Some(top_p) = req.top_p {
            body["top_p"] = json!(top_p);
        }
    }
    if !system_content.is_empty() {
        body["system"] = json!(system_content);
    }
    // `output_config.effort` is orthogonal to `thinking` (works with or
    // without it, per Anthropic's docs) — only ever sent on a model that
    // supports it (see `anthropic_supports_effort`'s doc comment).
    if let Some(effort) = req
        .effort
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty())
    {
        if anthropic_supports_effort(&req.model) {
            body["output_config"] = json!({ "effort": effort });
        }
    }
    body
}

/// Thinking-aware `max_tokens` inflation shared by every builder in this file:
/// [`build_chat_stream_body`] (whose `max_tokens` comes from the caller) and
/// the three non-streaming builders below (which hardcode a fixed cap).
/// Adaptive thinking is on by default and can't be turned off on several
/// models (Fable can't disable it at all) — it still counts toward
/// `max_tokens` even on paths that never send a `thinking` key (the three
/// non-streaming builders have no thinking-view display concern; default
/// `"omitted"` is correct there and gives faster time-to-first-text per
/// Anthropic's docs). Deliberately **not** gated on a size threshold: a small
/// caller-supplied cap (e.g. the extension bridge's `max_tokens: 1000`
/// answer-assist calls) or a fixed small cap (1024 for web search) still gets
/// thinking billed against it by default, so the inflation must apply
/// unconditionally.
///
/// The headroom is `max(base / 2, 1024)`, not a bare `base / 2`: a small cap
/// (e.g. 1000) would otherwise add less than the ~1024-token floor a model
/// typically needs to produce a useful summarized-thinking pass, leaving too
/// little room for both thinking and the visible response and risking a
/// short/empty draft even after "inflating". `saturating_add` guards `base`
/// being an unclamped caller-supplied IPC `u32` near `u32::MAX`.
fn adaptive_max_tokens(model: &str, base: u32) -> u32 {
    if anthropic_uses_adaptive_thinking(model) {
        base.saturating_add((base / 2).max(1024))
    } else {
        base
    }
}

/// Build the non-streaming `/messages` body shared by `complete`/
/// `complete_with_usage`. Pure + unit-tested — mirrors
/// [`build_chat_stream_body`]'s [`anthropic_supports_temperature`] gate but
/// never sends a `thinking` key at all (no thinking-view display concern on
/// this single-shot completion path).
fn build_complete_body(model: &str, system: &str, user: &str, temperature: Option<f64>) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": adaptive_max_tokens(model, 4096),
        "messages": [ { "role": "user", "content": user } ],
    });
    if anthropic_supports_temperature(model) {
        body["temperature"] = json!(temperature.unwrap_or(0.7));
    }
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    body
}

/// Build the non-streaming `/messages` body shared by every `research*`
/// facet (native `web_search` tool). Pure + unit-tested — same
/// [`anthropic_supports_temperature`] gate as [`build_complete_body`]; the
/// hardcoded `0.2` (favor precision over creativity for a research brief) is
/// simply skipped instead of overridden on adaptive models.
fn build_web_search_body(model: &str, system: &str, user: &str) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": adaptive_max_tokens(model, 1024),
        "system": system,
        "messages": [{ "role": "user", "content": user }],
        "tools": [{ "type": "web_search_20250305", "name": "web_search", "max_uses": 3 }],
    });
    if anthropic_supports_temperature(model) {
        body["temperature"] = json!(0.2);
    }
    body
}

/// Build the non-streaming `/messages` body for [`AnthropicClient::chat_with_tools`].
/// Pure + unit-tested — same [`anthropic_supports_temperature`] gate as
/// [`build_complete_body`].
fn build_tools_body(
    model: &str,
    system: &str,
    wire_messages: Vec<Value>,
    tool_specs: Vec<Value>,
    temperature: Option<f64>,
) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": adaptive_max_tokens(model, 4096),
        "messages": wire_messages,
        "tools": tool_specs,
    });
    if anthropic_supports_temperature(model) {
        body["temperature"] = json!(temperature.unwrap_or(0.7));
    }
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    body
}

/// Extract `usage.{input_tokens,output_tokens}` from a non-streaming Anthropic
/// Messages response — always present on a successful response. Pure +
/// unit-tested.
fn parse_anthropic_usage(data: &Value) -> Usage {
    let usage = data.get("usage");
    Usage {
        input_tokens: usage
            .and_then(|u| u.get("input_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        output_tokens: usage
            .and_then(|u| u.get("output_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
    }
}

pub struct AnthropicClient;

impl AnthropicClient {
    /// Shared body of `complete`/`complete_with_usage`: one non-streaming
    /// `/messages` call, parsed once into `(text, usage)` so the two trait
    /// methods never duplicate the HTTP round-trip.
    async fn complete_impl(
        &self,
        app: &AppHandle,
        model: &str,
        system: &str,
        user: &str,
        temperature: Option<f64>,
    ) -> AppResult<(String, Usage)> {
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let endpoint = format!("{BASE}/messages");
        let trace = RequestTrace::begin(ProviderId::Anthropic, model, "/messages", BASE, false);

        let body = build_complete_body(model, system, user, temperature);

        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(&endpoint)
                .timeout(timeouts::COMPLETION)
                .header("x-api-key", &api_key)
                .header("anthropic-version", VERSION)
                .json(&body)
        })
        .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Anthropic unreachable: {e}")));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(
                ProviderId::Anthropic,
                status,
                &body_text,
            ));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        let text = join_text_blocks(&data);
        if text.is_empty() {
            return Err(AppError::Provider(
                "Anthropic: unexpected response shape".to_string(),
            ));
        }
        Ok((text, parse_anthropic_usage(&data)))
    }

    /// Shared transport for every `research*` facet: a non-streaming Messages
    /// call with the server-side web-search tool, `system`/`user` supplied by the
    /// caller. Capped at 3 searches (a brief, not deep research); the enricher
    /// also bounds the whole call with a timeout. Requires the org to enable web
    /// search, and degrades to `""` (never an error) on any failure so
    /// generation always proceeds.
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
        let endpoint = format!("{BASE}/messages");
        let trace = RequestTrace::begin(
            ProviderId::Anthropic,
            model,
            "/messages web_search",
            BASE,
            false,
        );

        let body = build_web_search_body(model, system, user);

        let resp = crate::net::http::shared()
            .post(&endpoint)
            .timeout(timeouts::WEB_SEARCH)
            .header("x-api-key", &api_key)
            .header("anthropic-version", VERSION)
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                tracing::warn!("anthropic research unreachable: {e}");
                return Ok(String::new());
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            tracing::warn!("anthropic research {status}: {body_text}");
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
        Ok(join_text_blocks(&data))
    }
}

#[async_trait]
impl AiProvider for AnthropicClient {
    fn id(&self) -> ProviderId {
        ProviderId::Anthropic
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        // Every adaptive-thinking model 400s on ANY non-default temperature/
        // top_p/top_k on EVERY request, thinking or not (Anthropic's "Sampling
        // parameters" note), and an unrecognized `claude-`-prefixed id fails
        // safe to the same "no temperature" default — see
        // [`anthropic_supports_temperature`], the single source of truth this
        // field AND every request builder in this file both gate on.
        if !anthropic_supports_thinking(model) && !anthropic_uses_adaptive_thinking(model) {
            // Debug-only observability, never user-facing, never blocks the
            // call: this is either a legacy non-thinking model (nothing wrong
            // — it simply predates thinking) or an unrecognized new Anthropic
            // family this adapter hasn't learned a needle for yet, in which
            // case its thinking view (if any) stays blank until it's added.
            tracing::debug!(
                model,
                "anthropic: no thinking classification (legacy non-thinking model or \
                 unrecognized new family)"
            );
        }
        ModelCapabilities {
            supports_temperature: anthropic_supports_temperature(model),
            // Anthropic carries the system prompt as a top-level field, not a role.
            supports_system_role: false,
            supports_streaming: true,
            supports_reasoning: anthropic_supports_effort(model),
            supports_tools: true,
            supports_json_mode: false,
            supports_embeddings: false,
            // Native server-side web_search tool (account-key gated at call time).
            supports_web_search: true,
            token_param: TokenParam::MaxTokens,
        }
    }

    fn effort_levels(&self, model: &str) -> Vec<&'static str> {
        if anthropic_supports_effort(model) {
            vec!["low", "medium", "high"]
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
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let endpoint = format!("{BASE}/messages");
        let trace = RequestTrace::begin(ProviderId::Anthropic, &req.model, "/messages", BASE, true);

        let body = build_chat_stream_body(req);

        let response = crate::net::http::shared()
            .post(&endpoint)
            .timeout(timeouts::STREAM)
            .header("x-api-key", &api_key)
            .header("anthropic-version", VERSION)
            .json(&body)
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Anthropic unreachable: {e}")));
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(
                ProviderId::Anthropic,
                status,
                &body_text,
            ));
        }

        // The shared loop owns cancel-check + chunk read + emit + complete; the
        // closure is the only Anthropic-specific part (paired `event:`/`data:` SSE
        // framing, with `last_event`/`usage` carried across chunks).
        let mut last_event = String::new();
        let mut usage = Usage::default();
        stream_response(
            app,
            job_id,
            &trace,
            response,
            status.as_u16(),
            ProviderId::Anthropic,
            &req.model,
            BASE,
            move |buf| parse_anthropic_frames(buf, &mut last_event, &mut usage),
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

    async fn embed(&self, _app: &AppHandle, _model: &str, _text: &str) -> AppResult<Vec<f64>> {
        Err(AppError::Provider(
            "Anthropic has no embeddings API. Use OpenAI, Gemini, or Ollama for embeddings."
                .to_string(),
        ))
    }

    fn default_embedding_model(&self) -> Option<&'static str> {
        None
    }

    async fn list_models(&self, app: &AppHandle) -> Vec<Value> {
        let api_key = match get_provider_key(app, self.id().credential_key()) {
            Some(k) => k,
            None => return vec![],
        };
        let client = crate::net::http::shared();
        let resp = client
            .get(format!("{BASE}/models"))
            .header("x-api-key", &api_key)
            .header("anthropic-version", VERSION)
            .timeout(timeouts::LIST_MODELS)
            .send()
            .await;
        if let Ok(r) = resp {
            if let Ok(body) = r.json::<Value>().await {
                if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
                    return data
                        .iter()
                        .filter_map(|m| m.get("id").and_then(|id| id.as_str()))
                        .filter(|id| id.starts_with("claude-"))
                        .map(|id| json!({ "name": id }))
                        .collect();
                }
            }
        }
        vec![]
    }

    async fn test_key(&self, app: &AppHandle) -> AppResult<()> {
        let api_key = get_provider_key(app, self.id().credential_key())
            .ok_or_else(|| AppError::Config("No API key found".to_string()))?;
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
        let caps = self.capabilities(model);
        // Unknown / non-tool models degrade to a single-shot answer rather than
        // 400-ing on a `tools` field they don't understand.
        if !caps.supports_tools {
            return single_shot_turn(self, app, model, messages, temperature).await;
        }
        let api_key = get_provider_key(app, self.id().credential_key()).unwrap_or_default();
        let endpoint = format!("{BASE}/messages");
        let trace =
            RequestTrace::begin(ProviderId::Anthropic, model, "/messages tools", BASE, false);

        let (system, rest) = split_system(messages);
        let wire_messages: Vec<Value> = rest
            .iter()
            .map(|m| json!({ "role": m.role.wire(), "content": m.content }))
            .collect();
        // Map each ToolSpec to Anthropic's tool shape (`input_schema`). The caller's
        // schema is a trusted, fixed JSON-Schema object — never built from scraped
        // or model-supplied text.
        let tool_specs: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({ "name": t.name, "description": t.description, "input_schema": t.schema })
            })
            .collect();

        let body = build_tools_body(model, &system, wire_messages, tool_specs, temperature);

        let resp = send_with_retry(|| {
            crate::net::http::shared()
                .post(&endpoint)
                .timeout(timeouts::COMPLETION)
                .header("x-api-key", &api_key)
                .header("anthropic-version", VERSION)
                .json(&body)
        })
        .await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                trace.end(None, false);
                return Err(AppError::Network(format!("Anthropic unreachable: {e}")));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            trace.end(Some(status.as_u16()), false);
            return Err(friendly_api_error(
                ProviderId::Anthropic,
                status,
                &body_text,
            ));
        }
        let data: Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
        trace.end(Some(status.as_u16()), true);
        Ok(parse_anthropic_turn(&data))
    }
}

#[cfg(test)]
#[path = "anthropic_tests.rs"]
mod tests;
