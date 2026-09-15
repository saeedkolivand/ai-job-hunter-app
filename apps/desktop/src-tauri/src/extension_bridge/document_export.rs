//! `document.export` → `document.result` — PR2 (documents into ATS): exports a saved
//! generation's résumé/cover-letter text, or a base résumé, to PDF/DOCX/TXT bytes for the paired
//! extension to attach to a page or paste into a textarea. New module (R8 relief, alongside
//! `frame.rs` — see `mod.rs`'s own doc), same pattern as `match_live.rs`/`answers_save.rs`: one
//! verb, its own file.
//!
//! ## Gate + throttle
//! Extension caller only — refused for the CLI/any other origin with a fixed sentinel
//! ([`origin_refused_reply`]), checked in `caller_gate::advance_authenticated` alongside
//! `settings.get`/`settings.set`. Beyond that, rides the SAME Assisted-autofill opt-in
//! `profile.get`/`agent.query` do (PR1's `extension_read_gate` sentinel, same wording —
//! [`extension_gate_reply`]), also checked there — exporting a résumé/cover-letter out of the app
//! is the same consent class as handing it to a form field. A dedicated per-pairing throttle
//! ([`DocumentExportThrottle`]) — exports are a real Typst compile (100-400ms), not a cheap DB
//! read, so this does not share `agent_read::AgentQueryThrottle`'s cheap bucket.
//!
//! ## No new export logic
//! Chains straight into the existing [`crate::export::commands::documents_export_document`] — the
//! same Tauri command the renderer's own export flow calls — after resolving which text to feed
//! it (a generation's `resume_text`/`cover_letter_text`, or a base résumé's stored text) and
//! filling `meta`/`contact`/`letterLayoutId` the same way the renderer would. `TemplateId`/
//! `LetterLayout`'s own `Deserialize` impls already fall back to `Classic` on an unrecognized id
//! (see `export::types`), so an unknown `templateId`/`letterLayoutId` degrades exactly like every
//! other export caller's does — nothing re-validated here.
//!
//! ## Frame cap
//! Reuses the bridge's generic [`super::MAX_FRAME_BYTES`] refusal (`result_too_large`,
//! refuse-not-truncate) — NOT [`super::EXTENSION_RESULT_MAX_BYTES`] (the MCP-precedent 256 KiB
//! cap every other extension `agent.query`/`agent.call` reply is capped to): a rendered PDF/DOCX
//! does not fit in 256 KiB. `tests` measures the real worst case against the real cap directly.

use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::export::types::{
    DocumentType, ExportFormat, ExportRequest, GenerationMeta, LetterLayout, TemplateId,
};

use super::msg;

/// A malformed/incomplete request — a missing/wrong-shaped `source`, `kind`, `format`, or
/// `templateId`. Fixed sentinel, no dynamic content (matches every other refusal on this bridge).
const ERR_INVALID_REQUEST: &str = "invalid_document_request";
/// The resolved source has no usable text — an unknown/empty generation, a missing base résumé,
/// or a generation whose `resume_text`/`cover_letter_text` (per the requested `kind`) is empty.
const ERR_NOT_FOUND: &str = "not_found";
/// A `document` source (a base résumé) was requested with `kind: "cover-letter"` — base résumés
/// have no cover letter to export.
const ERR_UNSUPPORTED_SOURCE: &str = "unsupported_source";
/// The export chain itself failed (`documents_export_document` returned `Err`) — `detail` carries
/// the reason after `sanitize_reason` (credential tokens redacted, length-capped; the export chain
/// never puts document text in its errors). Paths are not specifically stripped — same helper and
/// same trust posture as every other bridge refusal detail.
const ERR_EXPORT_FAILED: &str = "export_failed";

/// [`super::agent_call::ERR_EXTENSION_READ_GATE`]'s detail, for THIS verb's own reply — same
/// sentinel, same wording as `agent_read`/`agent_call`'s own copies (each file keeps its own
/// literal; see `agent_call.rs`'s `EXTENSION_READ_GATE_MESSAGE` doc for why — the SENTINEL is
/// what all three actually reuse, never the prose).
const EXTENSION_READ_GATE_DETAIL: &str =
    "Turn on Assisted autofill in AI Job Hunter → Settings → Accounts → Browser extension to let \
     the paired browser extension read your data.";

/// `document.export` from a non-extension caller (the CLI, or any other origin) — unlike
/// `agent.query`/`agent.call`, the CLI never reaches this verb (it already has
/// `documents:documents_export_document` via `agent.call`).
const CLI_ONLY_MESSAGE: &str = "document.export is only available to the paired browser extension";

fn error_reply(req_id: &str, error: &str, detail: &str, retry_after_ms: Option<u64>) -> String {
    let mut payload = json!({ "ok": false, "error": error, "detail": detail });
    if let Some(ms) = retry_after_ms {
        payload["retryAfterMs"] = json!(ms);
    }
    json!({ "type": msg::DOCUMENT_RESULT, "reqId": req_id, "payload": payload }).to_string()
}

/// `document.export` from the CLI or any other non-extension origin.
pub(super) fn origin_refused_reply(req_id: &str) -> String {
    error_reply(req_id, "origin_refused", CLI_ONLY_MESSAGE, None)
}

/// `document.export` from the extension while Assisted autofill is off.
pub(super) fn extension_gate_reply(req_id: &str) -> String {
    error_reply(
        req_id,
        super::agent_call::ERR_EXTENSION_READ_GATE,
        EXTENSION_READ_GATE_DETAIL,
        None,
    )
}

/// `document.export` refused by [`DocumentExportThrottle`] — `retry_after_ms` comes from
/// [`super::BridgeState::document_export_retry_after_ms`], read by the ONE caller right after a
/// failed `try_acquire_document_export` (same discipline as every other throttle reply here).
pub(super) fn throttled_reply(req_id: &str, retry_after_ms: u64) -> String {
    error_reply(
        req_id,
        super::agent_call::ERR_RATE_LIMITED,
        super::agent_read::THROTTLED_MESSAGE,
        Some(retry_after_ms),
    )
}

// ── Throttle (own instance on `BridgeState`, per pairing — see module doc) ─────────

/// Burst 3, refilling one token every 10s — deliberately much tighter than
/// `agent_read::AgentQueryThrottle`'s cheap-read bucket (burst 10 @ 1/s): an export is a real
/// Typst compile, not an in-memory read.
const DOCUMENT_EXPORT_BURST: f64 = 3.0;
const DOCUMENT_EXPORT_REFILL_SECS: f64 = 10.0;

/// Minimal token bucket — the exact math `match_live::MatchLiveThrottle`/
/// `agent_read`'s private `TokenBucket` use; its own instance rather than sharing either (per-verb
/// cost profiles differ — see the module doc).
pub(super) struct DocumentExportThrottle {
    tokens: f64,
    last: std::time::Instant,
}

impl DocumentExportThrottle {
    pub(super) fn new() -> Self {
        Self {
            tokens: DOCUMENT_EXPORT_BURST,
            last: std::time::Instant::now(),
        }
    }

    fn try_acquire_at(&mut self, now: std::time::Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens =
            (self.tokens + elapsed / DOCUMENT_EXPORT_REFILL_SECS).min(DOCUMENT_EXPORT_BURST);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    pub(super) fn try_acquire(&mut self) -> bool {
        self.try_acquire_at(std::time::Instant::now())
    }

    /// Milliseconds until this bucket would hold one full token — only meaningful called right
    /// after a failed [`Self::try_acquire`] in the same tick (mirrors
    /// `agent_read`'s private `TokenBucket::retry_after_ms`).
    pub(super) fn retry_after_ms(&self) -> u64 {
        if self.tokens >= 1.0 {
            return 0;
        }
        let needed_secs = (1.0 - self.tokens) * DOCUMENT_EXPORT_REFILL_SECS;
        (needed_secs * 1000.0).ceil() as u64
    }
}

// ── Request parsing (pure) ──────────────────────────────────────────────────

/// One of the two allowed `source.kind` values, plus the id/url it carries.
#[derive(Debug, PartialEq, Eq)]
enum Source {
    Generation(String),
    Document(String),
}

/// A validated `document.export` request, ready to resolve against app state. Every field the
/// caller CHOSE (as opposed to resolved from a store) keeps its own wire string alongside the
/// parsed value, so the success reply can echo back exactly what was requested without
/// re-serializing the parsed value.
#[derive(Debug)]
struct ParsedRequest {
    source: Source,
    document_type: DocumentType,
    kind_wire: String,
    format: ExportFormat,
    format_wire: String,
    template_id: TemplateId,
    template_id_wire: String,
    letter_layout: LetterLayout,
    ats_mode: bool,
}

/// Round-trip a caller-supplied string through `T`'s own `Deserialize` — the same trick
/// `agent_read::project` uses on a whole struct, here one field at a time, since this verb's wire
/// shape (`source`/`kind`/`format`/`templateId`) is not `ExportRequest`'s own.
fn parse_wire<T: serde::de::DeserializeOwned>(s: &str) -> Option<T> {
    serde_json::from_value(json!(s)).ok()
}

fn parse_source(payload: &Value) -> Option<Source> {
    let source = payload.get("source")?;
    match source.get("kind").and_then(Value::as_str)? {
        "generation" => {
            let url = source.get("url").and_then(Value::as_str)?.trim();
            (!url.is_empty()).then(|| Source::Generation(url.to_string()))
        }
        "document" => {
            let id = source.get("id").and_then(Value::as_str)?.trim();
            (!id.is_empty()).then(|| Source::Document(id.to_string()))
        }
        _ => None,
    }
}

/// Pure parse + validate of the wire request — no `AppHandle`, directly unit-testable.
fn parse_request(payload: &Value) -> Result<ParsedRequest, &'static str> {
    let source = parse_source(payload).ok_or(ERR_INVALID_REQUEST)?;
    let kind_wire = payload
        .get("kind")
        .and_then(Value::as_str)
        .ok_or(ERR_INVALID_REQUEST)?
        .to_string();
    let document_type: DocumentType = parse_wire(&kind_wire).ok_or(ERR_INVALID_REQUEST)?;
    let format_wire = payload
        .get("format")
        .and_then(Value::as_str)
        .ok_or(ERR_INVALID_REQUEST)?
        .to_string();
    let format: ExportFormat = parse_wire(&format_wire).ok_or(ERR_INVALID_REQUEST)?;
    let template_id_wire = payload
        .get("templateId")
        .and_then(Value::as_str)
        .ok_or(ERR_INVALID_REQUEST)?
        .to_string();
    // `TemplateId`'s own `Deserialize` never rejects a string (unknown → Classic, matching
    // `documents_export_document`'s own tolerance) — only a non-string/absent `templateId` is a
    // malformed request.
    let template_id: TemplateId = parse_wire(&template_id_wire).ok_or(ERR_INVALID_REQUEST)?;
    let letter_layout = payload
        .get("letterLayoutId")
        .and_then(Value::as_str)
        .and_then(parse_wire)
        .unwrap_or_default();
    let ats_mode = payload
        .get("atsMode")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(ParsedRequest {
        source,
        document_type,
        kind_wire,
        format,
        format_wire,
        template_id,
        template_id_wire,
        letter_layout,
        ats_mode,
    })
}

fn non_empty(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Cover-letter market id for a target language — the Rust-side twin of
/// `LANGUAGE_TO_MARKET` in `packages/prompts/src/locale/index.ts`'s
/// `resolveMarket`. This bridge (unlike `AIGeneratePage`/`useTailorPipeline`,
/// which also fold in a live job's `jobCountry`/`briefCountry`) has no job
/// context at all — a saved generation or a base résumé carries only its
/// language — so this mirrors exactly the case `GenerationCard`'s own
/// `resolveMarket({ targetLanguage })` call handles (see that component's doc
/// comment): language is the whole signal, and `resolveMarket`'s own
/// language → intl fallback for an unmapped/blank language. Keeps `request`'s
/// `locale` from defaulting to the English ("intl") salutation/sign-off
/// convention (`complete_letter_text`, keyed on `request.locale`) for every
/// non-English cover letter.
fn cover_letter_market(target_language: Option<&str>) -> &'static str {
    let lang: String = target_language
        .unwrap_or("")
        .trim()
        .chars()
        .take(2)
        .collect::<String>()
        .to_lowercase();
    match lang.as_str() {
        "de" => "de",
        "fr" => "fr",
        "es" => "es",
        "it" => "it",
        "pt" => "pt",
        "tr" => "tr",
        "ru" => "ru",
        "zh" => "cn",
        "ja" => "jp",
        "ko" => "kr",
        _ => "intl",
    }
}

/// The text + `meta` for a `Source::Generation` — `resume_text`/`cover_letter_text` per
/// `document_type`; language from the generation's own `target_language`. `None` when the
/// requested kind's text is empty (an incomplete generation).
fn resolve_generation(
    record: &crate::ai_generations::AiGenerationRecord,
    document_type: DocumentType,
) -> Option<(String, GenerationMeta)> {
    let text = match document_type {
        DocumentType::Resume => record.resume_text.clone(),
        DocumentType::CoverLetter => record.cover_letter_text.clone(),
    };
    if text.trim().is_empty() {
        return None;
    }
    Some((
        text,
        GenerationMeta {
            candidate_name: non_empty(&record.candidate_name),
            job_title: non_empty(&record.job_title),
            company_name: non_empty(&record.company_name),
            target_language: non_empty(&record.target_language),
        },
    ))
}

/// Answer an authenticated, gate-checked, throttle-admitted `document.export`. Resolves the
/// requested source, chains into the existing `documents_export_document` command (no new export
/// logic — see the module doc), and returns a ready-to-send `document.result` reply, frame-cap
/// enforced.
pub(super) async fn handle_document_export(
    app: &AppHandle,
    req_id: &str,
    payload: &Value,
) -> String {
    let parsed = match parse_request(payload) {
        Ok(p) => p,
        Err(sentinel) => {
            return error_reply(req_id, sentinel, "malformed document.export request", None)
        }
    };

    let (text, meta) = match &parsed.source {
        Source::Generation(url) => {
            let Some(store) = app.try_state::<crate::ai_generations::AiGenerationStore>() else {
                return error_reply(
                    req_id,
                    ERR_NOT_FOUND,
                    "no saved generation for this job",
                    None,
                );
            };
            let Some(record) = store.find_for_job(url) else {
                return error_reply(
                    req_id,
                    ERR_NOT_FOUND,
                    "no saved generation for this job",
                    None,
                );
            };
            match resolve_generation(&record, parsed.document_type) {
                Some(pair) => pair,
                None => {
                    return error_reply(
                        req_id,
                        ERR_NOT_FOUND,
                        "this generation has no text for the requested kind",
                        None,
                    )
                }
            }
        }
        Source::Document(id) => {
            if parsed.document_type != DocumentType::Resume {
                return error_reply(
                    req_id,
                    ERR_UNSUPPORTED_SOURCE,
                    "a base résumé (\"document\" source) can only export kind \"resume\"",
                    None,
                );
            }
            let Some(store) = app.try_state::<crate::documents::DocumentStore>() else {
                return error_reply(req_id, ERR_NOT_FOUND, "no matching résumé on file", None);
            };
            let Some(doc) = store.get(id) else {
                return error_reply(req_id, ERR_NOT_FOUND, "no matching résumé on file", None);
            };
            if doc.text.trim().is_empty() {
                return error_reply(
                    req_id,
                    ERR_NOT_FOUND,
                    "this résumé has no extracted text",
                    None,
                );
            }
            let language = doc
                .locale
                .as_deref()
                .and_then(non_empty)
                .unwrap_or_else(|| crate::platform::config::read_locale_file(app));
            (
                doc.text,
                GenerationMeta {
                    candidate_name: None,
                    job_title: None,
                    company_name: None,
                    target_language: Some(language),
                },
            )
        }
    };

    let contact = app
        .try_state::<crate::contact_profile::ContactProfileStore>()
        .map(|s| s.get());

    // Résumé locale stays unset (matches the renderer precedent — no
    // template-locale picker on this bridge). Cover-letter locale must be
    // resolved from the meta's target language so `validate_and_normalize`'s
    // `complete_letter_text` picks the letter's own market instead of
    // defaulting to the English ("intl") salutation/sign-off convention.
    let locale = (parsed.document_type == DocumentType::CoverLetter)
        .then(|| cover_letter_market(meta.target_language.as_deref()).to_string());

    let request = ExportRequest {
        text,
        format: parsed.format,
        document_type: parsed.document_type,
        template_id: parsed.template_id,
        meta: Some(meta),
        ats_mode: parsed.ats_mode,
        locale,
        contact,
        accent: None,
        letter_layout: parsed.letter_layout,
    };

    let result = match crate::export::commands::documents_export_document(request).await {
        Ok(result) => result,
        Err(e) => {
            let reason = crate::observability::sanitize_reason(&e.to_string());
            return error_reply(req_id, ERR_EXPORT_FAILED, &reason, None);
        }
    };

    success_or_capped_reply(
        req_id,
        &result,
        &parsed.kind_wire,
        &parsed.format_wire,
        &parsed.template_id_wire,
    )
}

/// Build the success `document.result` reply from a real [`crate::export::types::ExportResult`],
/// re-measured against [`super::MAX_FRAME_BYTES`] and substituted with a `result_too_large`
/// refusal if it somehow doesn't fit (issue #1135's pattern, applied here) — refuse-not-truncate,
/// never a silently truncated document on the wire. Pure (no `AppHandle`/I/O) — directly
/// unit-testable against a synthetic oversized `ExportResult` without a real Typst compile.
fn success_or_capped_reply(
    req_id: &str,
    result: &crate::export::types::ExportResult,
    kind_wire: &str,
    format_wire: &str,
    template_id_wire: &str,
) -> String {
    let byte_length = result.data.len();
    let data_b64 = super::agent_call::reshape::encode_base64(&result.data);
    let reply = json!({
        "type": msg::DOCUMENT_RESULT,
        "reqId": req_id,
        "payload": {
            "ok": true,
            "data": data_b64,
            "dataEncoding": "base64",
            "mimeType": result.mime_type,
            "filename": result.filename,
            "byteLength": byte_length,
            "kind": kind_wire,
            "format": format_wire,
            "templateId": template_id_wire,
        },
    })
    .to_string();

    if reply.len() <= super::MAX_FRAME_BYTES {
        return reply;
    }
    error_reply(
        req_id,
        super::agent_call::ERR_RESULT_TOO_LARGE,
        &format!(
            "the exported document ({} B) exceeds the bridge's own frame cap and was discarded \
             rather than truncated — try a smaller template or format",
            reply.len()
        ),
        None,
    )
}

#[cfg(test)]
mod tests;
