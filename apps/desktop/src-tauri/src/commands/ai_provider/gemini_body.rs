//! The pure Gemini request-body builders — `streamGenerateContent`,
//! `generateContent`, and `embedContent`.
//!
//! Wired via `#[path = "gemini_body.rs"] mod body;` in `gemini.rs` (the same
//! sibling-file convention as `gemini_tests.rs`, and the mirror of
//! `openai_body.rs`), so these stay CHILD items of `gemini` and keep reaching
//! its private model-classification gates. Split out because `gemini.rs` is at
//! the R8 LOC cap; no call site or test import moves.

use serde_json::{json, Value};

use super::{
    gemini_effective_temperature, gemini_effort_levels, gemini_omits_sampling_params,
    gemini_supports_thinking, AiGenerateRequest, SamplingProfile, EMBED_OUTPUT_DIMENSIONALITY,
};

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
/// deprecation and stay ungated — this adapter's `GeminiClient::sampling_profile`
/// never declares either (no vendor-recommended band for them, unlike
/// OpenAI's), so in practice they only ever carry an explicit override.
pub(super) fn build_chat_stream_body(req: &AiGenerateRequest, sampling: SamplingProfile) -> Value {
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
    // `thinkingLevel` is gated on Gemini 3+ (`gemini_is_v3_or_later`) per
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
    if let Some(level) = thinking_level(&req.model, req.effort.as_deref()) {
        thinking_config.insert("thinkingLevel".to_string(), json!(level));
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

/// The `thinkingConfig.thinkingLevel` value a request may send on this model —
/// `None` when it must not be sent at all. See [`build_chat_stream_body`]'s
/// call site for the two gates this single per-model lookup covers (Gemini 3+
/// AND the model's own accepted subset).
///
/// Shared by BOTH body builders rather than re-written per call site.
/// `complete_structured` is the one non-streaming path that receives the whole
/// [`AiGenerateRequest`], and it silently dropped `effort` for its entire
/// existence while `chat_stream` honored it — a second hand-written copy of
/// this gate is precisely how that happens again.
pub(super) fn thinking_level(model: &str, effort: Option<&str>) -> Option<String> {
    let effort = effort.map(str::trim)?;
    gemini_effort_levels(model)
        .contains(&effort)
        .then(|| effort.to_ascii_uppercase())
}

/// What [`super::GeminiClient::complete_structured`] adds to a
/// `generateContent` call and the plain `complete`/`complete_with_usage` path
/// cannot: those two take no [`AiGenerateRequest`], so they have neither a
/// schema nor an effort to send. `Some(..)` IS the JSON-mode switch — the two
/// can no longer disagree.
pub(super) struct StructuredCall<'a> {
    /// The translated `responseSchema`, or `None` when the caller's JSON Schema
    /// had no faithful Gemini equivalent (see
    /// `structured::gemini_response_schema`) — JSON mode still applies, just
    /// without a shape constraint.
    pub(super) schema: Option<Value>,
    /// The request's RAW reasoning effort, gated here by [`thinking_level`].
    pub(super) effort: Option<&'a str>,
    /// `req.max_tokens` → `generationConfig.maxOutputTokens`, exactly as on
    /// [`build_chat_stream_body`].
    pub(super) max_tokens: Option<u32>,
}

/// Build the non-streaming `generateContent` body shared by `complete`/
/// `complete_with_usage`/`complete_structured`. Pure + unit-tested.
///
/// `structured` is `Some` only on the structured path and sets
/// `responseMimeType: "application/json"` — Google documents the MIME type as
/// the switch, with `responseSchema` as an optional extra constraint on top
/// (a schema without the MIME type is ignored). `maxOutputTokens` rides along
/// with it because the plain paths have no request to read it from, and is
/// gated exactly as [`build_chat_stream_body`] gates its own copy: sent only
/// when the request set one, never as `null`.
///
/// `thinkingConfig` carries `thinkingLevel` ONLY — never
/// `includeThoughts`, unlike [`build_chat_stream_body`]. The streaming path
/// splits `thought` parts out into their own UI channel; this path's reader
/// (`join_parts_text`) concatenates every `parts[].text` there is, so asking
/// for thoughts here would paste the model's reasoning into the very string the
/// caller is about to parse as JSON.
pub(super) fn build_complete_body(
    model: &str,
    system: &str,
    user: &str,
    temperature: Option<f64>,
    structured: Option<StructuredCall<'_>>,
) -> Value {
    let mut generation_config = json!({});
    if let Some(t) = gemini_effective_temperature(model, temperature, 0.7) {
        generation_config["temperature"] = json!(t);
    }
    if let Some(structured) = structured {
        generation_config["responseMimeType"] = json!("application/json");
        if let Some(schema) = structured.schema {
            generation_config["responseSchema"] = schema;
        }
        if let Some(mt) = structured.max_tokens {
            generation_config["maxOutputTokens"] = json!(mt);
        }
        if let Some(level) = thinking_level(model, structured.effort) {
            generation_config["thinkingConfig"] = json!({ "thinkingLevel": level });
        }
    }
    let mut body = json!({
        "contents": [ { "role": "user", "parts": [{ "text": user }] } ],
        "generationConfig": generation_config,
    });
    if !system.is_empty() {
        body["systemInstruction"] = json!({ "parts": [{ "text": system }] });
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
/// **Nesting matters — settled against the machine-readable API surface, not
/// against prose.** Three reviewers have now read the rendered REST reference
/// for `models.embedContent` and reached two different conclusions, so the
/// citation of record is the v1beta DISCOVERY DOCUMENT
/// (`https://generativelanguage.googleapis.com/$discovery/rest?version=v1beta`,
/// revision `20260806`), which is what the endpoint actually validates
/// against:
///
/// - `EmbedContentRequest.embedContentConfig` → `$ref: EmbedContentConfig`,
///   with no deprecation marker. **This is the field below.**
/// - `EmbedContentRequest.outputDimensionality` → `"deprecated": true`,
///   *"Deprecated: Please use EmbedContentConfig.output_dimensionality
///   instead."* — i.e. moving this field UP one level (the recurring review
///   suggestion) targets the deprecated location, not the current one.
/// - `EmbedContentConfig.outputDimensionality` → `integer`/`int32`; camelCase
///   is the proto-JSON name, so no `output_dimensionality` snake spelling.
///
/// Belt and braces, because a doc revision can still move underneath us:
/// proto3-JSON transcoding tolerates an unknown
/// or misplaced field with NO error, so a wrong nesting here would silently
/// return the model's full default dimension instead of
/// [`EMBED_OUTPUT_DIMENSIONALITY`] — `embed_impl` checks the ACTUAL returned
/// vector length against it and fails loudly on any mismatch, so the wire
/// shape is verified by the code at runtime on every real call, not by
/// whoever last read the docs.
pub(super) fn build_embed_body(m: &str, text: &str) -> Value {
    json!({
        "model": format!("models/{m}"),
        "content": { "parts": [{ "text": text }] },
        "embedContentConfig": { "outputDimensionality": EMBED_OUTPUT_DIMENSIONALITY },
    })
}
