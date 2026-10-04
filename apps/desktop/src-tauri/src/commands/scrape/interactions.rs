//! The user's per-job interactions (viewed / saved / dismissed …): persist, undo and
//! list. Split out of `commands/scrape.rs` for R8 (issue #1280); `scrape.rs`
//! re-exports the commands, so each keeps its `commands::scrape::<name>` path.

use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::db::now_ms;
use crate::postings::{InteractionRecord, InteractionStore};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobObject {
    pub id: Option<String>,
    pub title: Option<String>,
    pub company: Option<String>,
    pub url: Option<String>,
    pub source: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapePersistJobRequest {
    pub job: JobObject,
    pub interaction_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeListFilter {
    pub interaction_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeRemoveInteractionRequest {
    pub job_id: String,
    pub interaction_type: String,
}

/// Reverses `agent_call::fence_scraped_fields`'s wrapper for a field about
/// to be written into `InteractionStore` (security review round 4): a
/// caller that reads a job through a fenced surface (`scrape_list_postings`,
/// `autopilot_list`, …) and echoes the value straight back here would
/// otherwise persist the literal `<job_posting>…</job_posting>` markup into
/// the user's real interaction history. A no-op for the normal case — a
/// caller passing a clean value that was never fenced.
fn unfence_job_field(v: Option<String>) -> String {
    crate::prompt_fence::strip_fence_wrapper("job_posting", &v.unwrap_or_default())
}

#[tauri::command]
pub fn scrape_persist_job(app: AppHandle, req: ScrapePersistJobRequest) -> Value {
    let record = InteractionRecord {
        job_id: req.job.id.unwrap_or_default(),
        interaction_type: req.interaction_type,
        timestamp: now_ms(),
        title: unfence_job_field(req.job.title),
        company: unfence_job_field(req.job.company),
        url: req.job.url.unwrap_or_default(),
        source: req.job.source.unwrap_or_default(),
        location: unfence_job_field(req.job.location),
    };
    app.state::<Mutex<InteractionStore>>().lock().upsert(record);
    json!({ "success": true })
}

/// The real "undo" for [`scrape_persist_job`] — deletes the persisted
/// interaction instead of only hiding it client-side. Keys on the same
/// `(jobId, interactionType)` pair `upsert` writes; see
/// [`InteractionStore::remove`] for the "nothing to remove" distinction.
#[tauri::command]
pub fn scrape_remove_interaction(app: AppHandle, req: ScrapeRemoveInteractionRequest) -> bool {
    app.state::<Mutex<InteractionStore>>()
        .lock()
        .remove(&req.job_id, &req.interaction_type)
}

#[tauri::command]
pub fn scrape_list_interactions(app: AppHandle, filter: Option<ScrapeListFilter>) -> Value {
    let filter_type = filter.and_then(|f| f.interaction_type);
    let binding = app.state::<Mutex<InteractionStore>>();
    let mut store = binding.lock();
    json!(store.list(filter_type.as_deref()))
}

#[cfg(test)]
mod tests;
