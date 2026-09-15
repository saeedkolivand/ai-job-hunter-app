//! `documents` resource (PR2 — documents into ATS) — the extension's document-picker candidate
//! list: whether this job has a saved generation (résumé/cover-letter TEXT PRESENCE only, never
//! the text itself — that only ever crosses the wire through `document.export`) and the base
//! résumés on file, newest first. New file (R8 relief), same pattern as `found_jobs.rs`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

/// Cap on the base résumés returned — a picker, not a paginated traversal
/// (`found-jobs`/`best-matches` own that kind of unbounded list). Small, audited constant.
const MAX_DOCUMENTS: usize = 20;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentsGeneration {
    has_resume: bool,
    has_cover_letter: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_language: Option<String>,
    updated_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    job_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    company: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentsBaseDoc {
    id: String,
    name: String,
    updated_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
}

fn non_empty(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Pure projection of one generation record → the picker's own shape — directly unit-testable
/// against a hand-built `AiGenerationRecord`, no `AppHandle`. `pub(super)` for its own tests.
pub(super) fn project_generation(record: &crate::ai_generations::AiGenerationRecord) -> Value {
    json!(DocumentsGeneration {
        has_resume: !record.resume_text.trim().is_empty(),
        has_cover_letter: !record.cover_letter_text.trim().is_empty(),
        target_language: non_empty(&record.target_language),
        updated_at: record.created_at,
        job_title: non_empty(&record.job_title),
        company: non_empty(&record.company_name),
    })
}

/// Pure projection of one base résumé row → the picker's own shape.
pub(super) fn project_document(doc: &crate::documents::DocumentRecord) -> Value {
    json!(DocumentsBaseDoc {
        id: doc.id.clone(),
        name: doc.name.clone(),
        updated_at: doc.created_at,
        language: doc.locale.as_deref().and_then(non_empty),
    })
}

pub(super) fn documents_resource(app: &AppHandle, payload: &Value) -> AppResult<Value> {
    let url = payload
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if url.is_empty() {
        return Err(AppError::Validation("url is required".to_string()));
    }
    let generation = app
        .try_state::<crate::ai_generations::AiGenerationStore>()
        .and_then(|store| store.find_for_job(url))
        .map(|record| project_generation(&record))
        .unwrap_or(Value::Null);
    let documents: Vec<Value> = app
        .try_state::<crate::documents::DocumentStore>()
        .map(|store| {
            store
                .list() // already `ORDER BY created_at DESC` — newest first
                .iter()
                .take(MAX_DOCUMENTS)
                .map(project_document)
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({ "generation": generation, "documents": documents }))
}

#[cfg(test)]
mod tests;
