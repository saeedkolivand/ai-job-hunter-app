//! `document.export` request parsing + pure source resolution — split from `document_export.rs`
//! (R8 relief): everything here is pure (no `AppHandle`), directly unit-testable, and produces
//! either a validated [`ParsedRequest`] or the text+meta `handle_document_export` feeds into
//! `ExportRequest`.

use serde_json::{json, Value};

use crate::export::types::{DocumentType, ExportFormat, GenerationMeta, LetterLayout, TemplateId};

use super::document_export::ERR_INVALID_REQUEST;

// ── Request parsing (pure) ──────────────────────────────────────────────────

/// One of the two allowed `source.kind` values, plus the id/url it carries.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Source {
    Generation(String),
    Document(String),
}

/// A validated `document.export` request, ready to resolve against app state. Every field the
/// caller CHOSE (as opposed to resolved from a store) keeps its own wire string alongside the
/// parsed value, so the success reply can echo back exactly what was requested without
/// re-serializing the parsed value.
#[derive(Debug)]
pub(super) struct ParsedRequest {
    pub(super) source: Source,
    pub(super) document_type: DocumentType,
    pub(super) kind_wire: String,
    pub(super) format: ExportFormat,
    pub(super) format_wire: String,
    pub(super) template_id: TemplateId,
    pub(super) template_id_wire: String,
    pub(super) letter_layout: LetterLayout,
    pub(super) ats_mode: bool,
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
pub(super) fn parse_request(payload: &Value) -> Result<ParsedRequest, &'static str> {
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

pub(super) fn non_empty(s: &str) -> Option<String> {
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
pub(super) fn cover_letter_market(target_language: Option<&str>) -> &'static str {
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
pub(super) fn resolve_generation(
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

#[cfg(test)]
mod tests;
