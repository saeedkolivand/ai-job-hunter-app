use parking_lot::Mutex;
/// AutopilotStore — JSON-file-backed CRUD for Autopilot records.
///
/// Records are persisted to <dataDir>/autopilots.json as a flat JSON array.
/// All field names are serialised in camelCase to match the TypeScript schema
/// (`#[serde(rename_all = "camelCase")]`).
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppResult;

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotTarget {
    pub board: String,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_type: Option<String>,
    pub pages: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_filter: Option<String>,
    /// How many top-scoring postings autopilot should apply to after scraping.
    /// Defaults to 3 when absent.
    #[serde(default = "default_top_n")]
    pub top_n: u32,
}

fn default_top_n() -> u32 {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotFilter {
    pub min_match_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_keywords: Option<Vec<String>>,
}

/// A job posting surfaced by an autopilot run. Lightweight summary persisted so
/// the user can review what each autopilot found.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundJob {
    pub title: String,
    pub company: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// Full job description — used to pre-fill a tailored resume/cover letter generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Match score (0–100) when the posting passed ranking; absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    pub found_at: u64,
    /// First surfaced in the most recent run (set by the dedup merge in
    /// [`AutopilotStore::record_run`]). Drives the "New" badge.
    #[serde(default)]
    pub is_new: bool,
    /// Whether the user has generated an application for this job. **Derived** at
    /// read time from a saved generation whose `job_url` matches `url` (see
    /// `commands::autopilot`), never hand-set, so it can't drift. Stored value is
    /// always `false`; the read path fills it in.
    #[serde(default)]
    pub applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AutopilotStatus {
    Active,
    Paused,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Autopilot {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub status: AutopilotStatus,
    pub target: AutopilotTarget,
    pub filter: AutopilotFilter,
    pub schedule: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_text: Option<String>,
    /// Optional base cover letter reused as the starting point when tailoring a
    /// found job in the apply assistant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_letter: Option<String>,
    pub total_found: u32,
    pub total_applied: u32,
    /// Jobs surfaced by the most recent run. Defaulted so older records load.
    #[serde(default)]
    pub found_jobs: Vec<FoundJob>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

// ── Store ─────────────────────────────────────────────────────────────────────

pub struct AutopilotStore {
    data_file: PathBuf,
    cache: Mutex<Option<HashMap<String, Autopilot>>>,
}

impl AutopilotStore {
    pub fn new(data_dir: &PathBuf) -> Self {
        std::fs::create_dir_all(data_dir).ok();
        Self {
            data_file: data_dir.join("autopilots.json"),
            cache: Mutex::new(None),
        }
    }

    // ── CRUD ──────────────────────────────────────────────────────────────────

    pub fn list(&self) -> Vec<Autopilot> {
        let map = self.load();
        let mut items: Vec<Autopilot> = map.into_values().collect();
        items.sort_by_key(|a| std::cmp::Reverse(a.created_at));
        items
    }

    pub fn get(&self, id: &str) -> Option<Autopilot> {
        self.load().remove(id)
    }

    pub fn create(&self, input: serde_json::Value) -> Autopilot {
        let now = now_ms();
        let ap = Autopilot {
            id: Uuid::new_v4().to_string(),
            name: str_field(&input, "name"),
            status: AutopilotStatus::Active,
            target: serde_json::from_value(input["target"].clone()).unwrap_or_else(|_| {
                AutopilotTarget {
                    board: String::new(),
                    query: String::new(),
                    location: None,
                    work_type: None,
                    pages: 1,
                    date_filter: None,
                    top_n: default_top_n(),
                }
            }),
            filter: serde_json::from_value(input["filter"].clone()).unwrap_or({
                AutopilotFilter {
                    min_match_score: 50.0,
                    keywords: None,
                    exclude_keywords: None,
                }
            }),
            schedule: str_field(&input, "schedule"),
            resume_text: input["resumeText"].as_str().map(String::from),
            cover_letter: input["coverLetter"].as_str().map(String::from),
            total_found: 0,
            total_applied: 0,
            found_jobs: Vec::new(),
            last_run_at: None,
            created_at: now,
            updated_at: now,
        };
        let mut map = self.load();
        map.insert(ap.id.clone(), ap.clone());
        self.save(map);
        ap
    }

    pub fn update(&self, id: &str, patch: serde_json::Value) -> Option<Autopilot> {
        let mut map = self.load();
        let ap = map.get_mut(id)?;
        ap.updated_at = now_ms();
        if let Some(v) = patch.get("name").and_then(|v| v.as_str()) {
            ap.name = v.to_string();
        }
        if let Some(v) = patch.get("status").and_then(|v| v.as_str()) {
            ap.status = match v {
                "paused" => AutopilotStatus::Paused,
                "archived" => AutopilotStatus::Archived,
                _ => AutopilotStatus::Active,
            };
        }
        if let Ok(t) = serde_json::from_value::<AutopilotTarget>(patch["target"].clone()) {
            ap.target = t;
        }
        if let Ok(f) = serde_json::from_value::<AutopilotFilter>(patch["filter"].clone()) {
            ap.filter = f;
        }
        if let Some(v) = patch.get("schedule").and_then(|v| v.as_str()) {
            ap.schedule = v.to_string();
        }
        if let Some(v) = patch.get("resumeText").and_then(|v| v.as_str()) {
            ap.resume_text = Some(v.to_string());
        }
        if let Some(v) = patch.get("coverLetter").and_then(|v| v.as_str()) {
            ap.cover_letter = Some(v.to_string());
        }
        let result = ap.clone();
        self.save(map);
        Some(result)
    }

    pub fn remove(&self, id: &str) {
        let mut map = self.load();
        map.remove(id);
        self.save(map);
    }

    /// Remove every autopilot and its found-jobs history (factory reset).
    pub fn clear_all(&self) {
        self.save(HashMap::new());
    }

    pub fn set_status(&self, id: &str, status: AutopilotStatus) {
        let mut map = self.load();
        if let Some(ap) = map.get_mut(id) {
            ap.status = status;
            ap.updated_at = now_ms();
        }
        self.save(map);
    }

    /// Persist the outcome of a run: counts, last-run time, and the found-jobs
    /// list **merged** with prior runs by URL — so re-running keeps history
    /// (first-seen + any state) instead of replacing it, and genuinely new
    /// postings are flagged `is_new`.
    pub fn record_run(
        &self,
        id: &str,
        total_found: u32,
        total_applied: u32,
        found_jobs: Vec<FoundJob>,
    ) {
        let mut map = self.load();
        if let Some(ap) = map.get_mut(id) {
            let now = now_ms();
            ap.total_found = total_found;
            ap.total_applied = total_applied;
            ap.found_jobs = merge_found_jobs(&ap.found_jobs, found_jobs);
            ap.last_run_at = Some(now);
            ap.updated_at = now;
        }
        self.save(map);
    }

    pub fn stamp_last_run(&self, id: &str) {
        let mut map = self.load();
        if let Some(ap) = map.get_mut(id) {
            ap.last_run_at = Some(now_ms());
            ap.updated_at = now_ms();
        }
        self.save(map);
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    fn load(&self) -> HashMap<String, Autopilot> {
        // Silent migration off auto-apply: records written before the apply engine
        // was removed carry `action` (save/review/auto_apply) and `autoSubmit`
        // fields. Serde ignores unknown fields on deserialize, so every saved
        // autopilot loads cleanly as a find-&-save agent and the dead keys are
        // dropped on the next save — no explicit rewrite needed.
        let mut guard = self.cache.lock();
        if let Some(ref c) = *guard {
            return c.clone();
        }
        let map: HashMap<String, Autopilot> = std::fs::read_to_string(&self.data_file)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<Autopilot>>(&s).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|ap| (ap.id.clone(), ap))
            .collect();
        *guard = Some(map.clone());
        map
    }

    fn save(&self, map: HashMap<String, Autopilot>) {
        let list: Vec<&Autopilot> = {
            let mut v: Vec<&Autopilot> = map.values().collect();
            v.sort_by_key(|a| std::cmp::Reverse(a.created_at));
            v
        };
        if let Ok(json) = serde_json::to_string_pretty(&list) {
            std::fs::write(&self.data_file, json).ok();
        }
        *self.cache.lock() = Some(map);
    }

    /// Replace all autopilots with the given set (preserving their ids). Used by
    /// backup restore.
    pub fn replace_all(&self, items: Vec<Autopilot>) {
        let map: HashMap<String, Autopilot> =
            items.into_iter().map(|ap| (ap.id.clone(), ap)).collect();
        self.save(map);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn str_field(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Merge a fresh run's postings into the cumulative found-jobs list, idempotently:
///
/// - existing rows are kept (preserving `found_at`/first-seen) and have `is_new`
///   cleared; if the run re-surfaced them, volatile fields (title/company/
///   description/score/location) are refreshed,
/// - postings whose URL was never seen are appended and flagged `is_new`.
///
/// Re-running with the same postings yields the same set — only `is_new` moves.
/// `applied` is derived on read, so it is left at its default here.
fn merge_found_jobs(existing: &[FoundJob], incoming: Vec<FoundJob>) -> Vec<FoundJob> {
    use std::collections::HashMap;

    let incoming_by_url: HashMap<&str, &FoundJob> =
        incoming.iter().map(|j| (j.url.as_str(), j)).collect();

    let mut merged: Vec<FoundJob> = existing
        .iter()
        .map(|e| {
            let mut row = e.clone();
            row.is_new = false;
            if let Some(inc) = incoming_by_url.get(e.url.as_str()) {
                row.title = inc.title.clone();
                row.company = inc.company.clone();
                if inc.location.is_some() {
                    row.location = inc.location.clone();
                }
                if inc.description.is_some() {
                    row.description = inc.description.clone();
                }
                if inc.score.is_some() {
                    row.score = inc.score;
                }
            }
            row
        })
        .collect();

    let existing_urls: std::collections::HashSet<&str> =
        existing.iter().map(|j| j.url.as_str()).collect();
    for inc in incoming {
        if !existing_urls.contains(inc.url.as_str()) {
            merged.push(FoundJob {
                is_new: true,
                applied: false,
                ..inc
            });
        }
    }

    merged
}

impl crate::data_store::DataStore for AutopilotStore {
    fn key(&self) -> &'static str {
        "autopilots"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::json!(self.list())
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        let items: Vec<Autopilot> =
            serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
        let count = items.len();
        self.replace_all(items);
        Ok(count)
    }
}

#[cfg(test)]
mod test;
