use parking_lot::Mutex;
/// AutopilotStore — JSON-file-backed CRUD for Autopilot records.
///
/// Records are persisted to <dataDir>/autopilots.json as a flat JSON array.
/// All field names are serialised in camelCase to match the TypeScript schema
/// (`#[serde(rename_all = "camelCase")]`).
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Back-compat deserializer: `board` (string) OR `boards` (array) → Vec<String> ──

/// Accept either a JSON string (`"board": "linkedin"`) or a JSON array
/// (`"boards": ["linkedin","remotive"]`) and normalise to `Vec<String>`.
///
/// Backward-compatibility deserializer: on-disk `autopilots.json` records written
/// before the multi-board change store a single string under the legacy `"board"`
/// key; new records store an array under `"boards"`. The
/// `#[serde(alias = "board")]` on the field lets the old key name be accepted by
/// serde before this function is called — no data migration or rewrite required.
fn string_or_vec<'de, D>(de: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct StringOrVec;

    impl<'de> Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string or a sequence of strings")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(vec![v.to_string()])
        }

        fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
            Ok(vec![v])
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut out = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                out.push(s);
            }
            Ok(out)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(Vec::new())
        }

        fn visit_some<D2: serde::Deserializer<'de>>(self, d: D2) -> Result<Self::Value, D2::Error> {
            serde::Deserialize::deserialize(d).map(|v: serde_json::Value| match v {
                serde_json::Value::String(s) => vec![s],
                serde_json::Value::Array(arr) => arr
                    .into_iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect(),
                _ => Vec::new(),
            })
        }
    }

    de.deserialize_any(StringOrVec)
}

use crate::db::now_ms;
use crate::error::AppResult;

// ── Data model ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotTarget {
    /// The boards to scrape. Accepts either a `"boards": [...]` array (new
    /// format) or a `"board": "..."` string (legacy on-disk format). The alias
    /// + custom deserializer normalise both to `Vec<String>` transparently.
    #[serde(alias = "board", deserialize_with = "string_or_vec")]
    pub boards: Vec<String>,
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

/// Outcome of the most recent run. Distinct from [`AutopilotStatus`] (the
/// agent's enabled/paused lifecycle): this tracks a single run so the UI can
/// show a live/failed/interrupted indicator.
///
/// `Interrupted` is not set by a run — it's reconciled at startup from a run
/// left `InProgress` when the app closed or crashed mid-run (see
/// [`AutopilotStore::mark_interrupted_runs`]).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    InProgress,
    Completed,
    Failed,
    Interrupted,
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
    /// Local clock hour (0–23) a recurring schedule fires at. Used by
    /// daily/twice_daily; ignored by hourly. `None` falls back to 09:00 in the
    /// scheduler. Defaulted so older persisted records load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_hour: Option<u32>,
    /// Local clock minute (0–59) a recurring schedule fires at. Used by
    /// daily/twice_daily and as the "minute past the hour" for hourly. `None`
    /// falls back to minute 0 in the scheduler. Defaulted so older records load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_minute: Option<u32>,
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
    /// Outcome of the most recent run. `None` until the first run. Drives the
    /// live/failed/interrupted badge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_status: Option<RunStatus>,
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
        items.sort_by(cmp_autopilot_newest_first);
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
                    boards: Vec::new(),
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
            schedule_hour: u32_field_in_range(&input, "scheduleHour", 23),
            schedule_minute: u32_field_in_range(&input, "scheduleMinute", 59),
            resume_text: input["resumeText"].as_str().map(String::from),
            cover_letter: input["coverLetter"].as_str().map(String::from),
            total_found: 0,
            total_applied: 0,
            found_jobs: Vec::new(),
            run_status: None,
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
        if patch.get("scheduleHour").is_some() {
            ap.schedule_hour = u32_field_in_range(&patch, "scheduleHour", 23);
        }
        if patch.get("scheduleMinute").is_some() {
            ap.schedule_minute = u32_field_in_range(&patch, "scheduleMinute", 59);
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

    /// Set the most-recent-run outcome. `InProgress` is set at run start,
    /// `Failed` on a run error; `record_run` sets `Completed` on success.
    pub fn set_run_status(&self, id: &str, status: RunStatus) {
        let mut map = self.load();
        if let Some(ap) = map.get_mut(id) {
            ap.run_status = Some(status);
            ap.updated_at = now_ms();
        }
        self.save(map);
    }

    /// Reconcile runs left mid-flight: any autopilot still marked `InProgress`
    /// when the app starts was interrupted by a crash or close, so flip it to
    /// `Interrupted` for an honest badge instead of a stuck "running" state.
    /// Returns how many were reconciled. Called once at startup.
    pub fn mark_interrupted_runs(&self) -> usize {
        let mut map = self.load();
        let mut count = 0;
        for ap in map.values_mut() {
            if ap.run_status == Some(RunStatus::InProgress) {
                ap.run_status = Some(RunStatus::Interrupted);
                ap.updated_at = now_ms();
                count += 1;
            }
        }
        if count > 0 {
            self.save(map);
        }
        count
    }

    /// Persist the outcome of a run: counts, last-run time, and the found-jobs
    /// list **merged** with prior runs by URL — so re-running keeps history
    /// (first-seen + any state) instead of replacing it, and genuinely new
    /// postings are flagged `is_new`.
    /// Returns the number of **newly surfaced** jobs in this run (postings whose
    /// URL was never seen before) — drives the "N new jobs" notification + tray.
    pub fn record_run(
        &self,
        id: &str,
        total_found: u32,
        total_applied: u32,
        found_jobs: Vec<FoundJob>,
    ) -> u32 {
        let mut map = self.load();
        let mut new_count = 0u32;
        if let Some(ap) = map.get_mut(id) {
            let now = now_ms();
            ap.total_found = total_found;
            ap.total_applied = total_applied;
            ap.found_jobs = merge_found_jobs(&ap.found_jobs, found_jobs);
            // `merge_found_jobs` flags only never-before-seen URLs as `is_new`.
            new_count = ap.found_jobs.iter().filter(|j| j.is_new).count() as u32;
            ap.run_status = Some(RunStatus::Completed);
            ap.last_run_at = Some(now);
            ap.updated_at = now;
        }
        self.save(map);
        new_count
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
            v.sort_by(|a, b| cmp_autopilot_newest_first(a, b));
            v
        };
        if let Ok(json) = serde_json::to_string_pretty(&list) {
            // No-op-write skip: many mutations (set_run_status, stamp_last_run, …)
            // re-serialize identical state. Skip the disk write when the bytes
            // match what's already persisted — a pure dirty check, NOT debouncing,
            // so state is still flushed synchronously the instant it changes (no
            // crash-loss window). A missing/unreadable file never matches → write.
            let unchanged = std::fs::read_to_string(&self.data_file)
                .map(|existing| existing == json)
                .unwrap_or(false);
            if !unchanged {
                std::fs::write(&self.data_file, json).ok();
            }
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

/// Sort autopilots newest-first by `created_at`, breaking ties by `id` so the
/// order is stable across runs despite the unordered map. Single source of truth
/// for both the read order (`list`) and the on-disk order (`save`).
fn cmp_autopilot_newest_first(a: &Autopilot, b: &Autopilot) -> Ordering {
    b.created_at
        .cmp(&a.created_at)
        .then_with(|| a.id.cmp(&b.id))
}

fn str_field(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Read an optional non-negative integer field as `u32`, accepting it only when
/// it is in `0..=max`. Absent, non-numeric, or out-of-range → `None`, so a
/// missing or poisoned schedule time falls back to the scheduler defaults
/// rather than persisting a value that makes the occurrence permanently `None`
/// (silently dead autopilot). Guards the storage boundary against a client that
/// bypassed the Zod range check (e.g. `scheduleHour: 25`).
fn u32_field_in_range(v: &serde_json::Value, key: &str, max: u32) -> Option<u32> {
    v.get(key)
        .and_then(|v| v.as_u64())
        .map(|n| n as u32)
        .filter(|&n| n <= max)
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
