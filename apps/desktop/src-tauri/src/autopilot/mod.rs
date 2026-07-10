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
    pub country_code: Option<String>,
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
    /// Board id the posting was scraped from (its JobPosting.source). Persisted so the
    /// apply flow records accurate per-job provenance for multi-board autopilots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    /// Full job description — used to pre-fill a tailored resume/cover letter generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Scraped salary range from the board (Adzuna only, today) — grounds the salary
    /// application answer before it falls back to a web lookup. `None` when the
    /// board doesn't expose salary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salary_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salary_max: Option<f64>,
    /// ISO-4217 currency for `salary_min`/`salary_max`. `None` when the board didn't
    /// report one (e.g. an Adzuna market not in the country→currency map).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub salary_currency: Option<String>,
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
    /// Ghost-job trust signal, computed at find-time via
    /// [`crate::scraping::trust::assess_trust`]. `Option` (not a plain
    /// [`crate::scraping::trust::TrustAssessment`]) purely so a run recorded
    /// before this field existed still deserializes — `#[serde(default)]` gives
    /// `None` for a legacy record; every run recorded from here on always sets
    /// `Some(..)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<crate::scraping::trust::TrustAssessment>,
    /// Optional AI-reasoned note (2–4 sentences: why the job fits the résumé +
    /// one tailoring tip) generated for the top matches of a run when the
    /// autopilot has AI notes enabled (`assistant`). `None` for jobs not
    /// annotated (below the top-N ceiling, notes disabled, no provider, or the
    /// daily ceiling was hit mid-run). Read-only — never applied or submitted.
    /// `#[serde(default)]` so a job recorded before this field existed loads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_notes: Option<String>,
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
    /// Opt-in (Phase 4): after the keyword rank, attach a short AI-reasoned note
    /// to the top matches of each run. Read-only enrichment — never applies or
    /// submits anything. `#[serde(default)]` so existing records load as `false`.
    #[serde(default)]
    pub assistant: bool,
    /// Provider/model/base-URL snapshot the headless AI-notes run resolves through
    /// the centralized [`crate::pipeline::Completer`] (the same layer `ai_generate`
    /// uses). The scheduler has no renderer to read the active provider from, so the
    /// one chosen at opt-in time is persisted here. `None`/empty → notes skip
    /// gracefully for that run. Defaulted so older records load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_base_url: Option<String>,
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
                    country_code: None,
                    work_type: None,
                    pages: 1,
                    date_filter: None,
                    top_n: default_top_n(),
                }
            }),
            filter: serde_json::from_value(input["filter"].clone()).unwrap_or({
                // Creation default when no explicit filter is supplied: keep
                // everything (0.0). A non-zero default silently dropped jobs a
                // manual search would have returned — the autopilot zero-jobs bug.
                AutopilotFilter {
                    min_match_score: 0.0,
                    keywords: None,
                    exclude_keywords: None,
                }
            }),
            schedule: str_field(&input, "schedule"),
            schedule_hour: u32_field_in_range(&input, "scheduleHour", 23),
            schedule_minute: u32_field_in_range(&input, "scheduleMinute", 59),
            resume_text: input["resumeText"].as_str().map(String::from),
            cover_letter: input["coverLetter"].as_str().map(String::from),
            assistant: input["assistant"].as_bool().unwrap_or(false),
            assistant_provider: input["assistantProvider"].as_str().map(String::from),
            assistant_model: input["assistantModel"].as_str().map(String::from),
            assistant_base_url: input["assistantBaseUrl"].as_str().map(String::from),
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
        if let Some(v) = patch.get("assistant").and_then(|v| v.as_bool()) {
            ap.assistant = v;
            if !v {
                // Toggling AI notes off: clear the stale provider/model/base-url
                // snapshot too. The renderer omits `assistantProvider`/`Model`/
                // `BaseUrl` from the patch when disabling, so without this the old
                // snapshot would linger invisibly and could be reused verbatim if
                // AI notes are re-enabled later without a fresh provider pick.
                ap.assistant_provider = None;
                ap.assistant_model = None;
                ap.assistant_base_url = None;
            }
        }
        // The provider snapshot travels together with the toggle: the renderer
        // writes all three when the user enables AI notes (from the active
        // provider), so a re-selected provider re-snapshots on the next update.
        if let Some(v) = patch.get("assistantProvider").and_then(|v| v.as_str()) {
            ap.assistant_provider = Some(v.to_string());
        }
        if let Some(v) = patch.get("assistantModel").and_then(|v| v.as_str()) {
            ap.assistant_model = Some(v.to_string());
        }
        if let Some(v) = patch.get("assistantBaseUrl").and_then(|v| v.as_str()) {
            ap.assistant_base_url = Some(v.to_string());
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
        // Existing behavior: best-effort write, errors swallowed. Migrations that
        // need to *observe* a successful persist call `write_to_disk` directly.
        self.write_to_disk(&map).ok();
        *self.cache.lock() = Some(map);
    }

    /// Serialize + flush the map to `autopilots.json`, returning the IO outcome so
    /// a caller can gate on a successful persist (e.g. the one-shot migration's
    /// done-marker). Does NOT update the in-memory cache — that's `save`'s job.
    /// `Ok(())` is also returned on the no-op-write path (state already on disk).
    fn write_to_disk(&self, map: &HashMap<String, Autopilot>) -> std::io::Result<()> {
        let list: Vec<&Autopilot> = {
            let mut v: Vec<&Autopilot> = map.values().collect();
            v.sort_by(|a, b| cmp_autopilot_newest_first(a, b));
            v
        };
        let Ok(json) = serde_json::to_string_pretty(&list) else {
            // Serialization can't fail for this shape, but if it ever did there's
            // nothing on disk to trust — surface it as an IO-style error so the
            // migration won't mark itself done on un-persisted data.
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "failed to serialize autopilots",
            ));
        };
        // No-op-write skip: many mutations (set_run_status, stamp_last_run, …)
        // re-serialize identical state. Skip the disk write when the bytes
        // match what's already persisted — a pure dirty check, NOT debouncing,
        // so state is still flushed synchronously the instant it changes (no
        // crash-loss window). A missing/unreadable file never matches → write.
        let unchanged = std::fs::read_to_string(&self.data_file)
            .map(|existing| existing == json)
            .unwrap_or(false);
        if unchanged {
            return Ok(()); // desired state already persisted
        }
        std::fs::write(&self.data_file, json)
    }

    /// Replace all autopilots with the given set (preserving their ids). Used by
    /// backup restore.
    pub fn replace_all(&self, items: Vec<Autopilot>) {
        let map: HashMap<String, Autopilot> =
            items.into_iter().map(|ap| (ap.id.clone(), ap)).collect();
        self.save(map);
    }

    /// One-shot, idempotent migration that loosens autopilots saved with the old
    /// auto-prefilled restrictive filters (the cause of "autopilot returns ZERO
    /// jobs while manual search returns jobs for the same query"). Runs once per
    /// install, gated by a sidecar marker file beside `autopilots.json` — NOT a
    /// schema_version field, so the persisted/IPC shape is unchanged and gen:ipc
    /// can't drift.
    ///
    /// If the marker already exists this returns immediately. Otherwise it loads
    /// every autopilot, applies [`relax_legacy_filters`] to each, saves once, and
    /// writes the marker. Synchronous file IO — call it from the setup path, never
    /// from an async worker.
    ///
    /// Lock safety: [`Self::load`] takes `self.cache.lock()` but drops the guard
    /// before returning a cloned map, and [`Self::save`] re-takes the lock. This
    /// method only ever holds the owned clone from `load()` across the `save()`
    /// call — no `load()` guard is alive when `save()` re-locks, so there is no
    /// re-entrant lock / deadlock.
    pub fn relax_legacy_filters_once(&self) {
        let marker = self
            .data_file
            .parent()
            .map(|p| p.join(RELAX_MARKER_FILE))
            .unwrap_or_else(|| PathBuf::from(RELAX_MARKER_FILE));
        if marker.exists() {
            return;
        }

        let mut map = self.load(); // owned clone; the cache guard is already dropped
        for ap in map.values_mut() {
            relax_legacy_filters(ap);
        }

        // Persist FIRST and observe the result. Only write the done-marker once the
        // relaxed data is known to have hit disk — otherwise a save failure plus a
        // successful marker write would leave autopilots restrictive forever
        // ("done" but never relaxed). On a save error we skip the marker, the cache
        // stays as-loaded, and the next launch retries (harmless — the pass is
        // idempotent). `write_to_disk` returns Ok on the no-op path too (state
        // already persisted), which is also a valid "done" condition.
        let persisted = self.write_to_disk(&map);
        // Keep the in-memory cache consistent with whatever we just (attempted to)
        // persist; on success this is the relaxed map, mirroring `save`.
        *self.cache.lock() = Some(map);

        if persisted.is_ok() {
            // Mark done even if some autopilots were already loose: the goal is to
            // run the loosen pass exactly once, not to gate on whether it changed
            // anything.
            if let Some(parent) = self.data_file.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::write(&marker, b"1").ok();
        }
    }
}

/// Sidecar marker that records the legacy-filter loosen migration ran. Lives
/// beside `autopilots.json` in the data dir; its presence makes
/// [`AutopilotStore::relax_legacy_filters_once`] a no-op on subsequent launches.
const RELAX_MARKER_FILE: &str = "autopilot_relax_v1.done";

/// Surgically loosen one autopilot's auto-prefilled restrictive filters in place.
/// Kills the zero-jobs bug while preserving deliberate user customizations:
///
/// - `filter.keywords` → `None` **only on a legacy record** (see below),
/// - `filter.min_match_score` → `0.0` **only if** it is still the old auto
///   default `50.0`,
/// - `target.date_filter` → `None` **only if** it is still the old auto default
///   `Some("24h")`.
///
/// Everything else (`exclude_keywords`, `query`, `location`, `country_code`,
/// `boards`, `pages`, `work_type`, `top_n`) is left untouched. Pure + filesystem-
/// free so it is unit-testable on a bare `&mut Autopilot`.
///
/// **Idempotency guarantee.** "Legacy" is decided up front from the two *sentinel*
/// fields (`min_match_score == 50.0` OR `date_filter == Some("24h")`) — the
/// prefilled `keywords` clear is gated on that flag, NOT applied unconditionally.
/// So a record that's already been relaxed (score `0.0`, date `None`) is NOT
/// legacy → the whole function is a no-op → re-running can never erase keywords
/// the user added after the first relaxation. This matters because the done-marker
/// write in [`AutopilotStore::relax_legacy_filters_once`] is best-effort
/// (`.ok()`-swallowed): if it fails, the migration re-runs on next launch, and
/// this no-op-on-relaxed property is what makes that rerun safe. The marker is now
/// purely an optimization, not a correctness gate.
///
/// Narrow accepted gap: a record with prefilled keywords where the user ALSO
/// changed *both* the score (≠50) *and* the date (≠"24h") reads as non-legacy, so
/// its keywords are kept. That's rare and diagnosable, and erring toward keeping
/// user data is the safe direction.
pub(crate) fn relax_legacy_filters(ap: &mut Autopilot) {
    // Decide legacy-ness from the sentinels BEFORE mutating them, so the decision
    // can't be invalidated by our own resets below.
    let was_legacy =
        ap.filter.min_match_score == 50.0 || ap.target.date_filter.as_deref() == Some("24h");

    if was_legacy {
        // Only legacy records carry the auto-prefilled keyword list manual search
        // never applies; clearing it on an already-relaxed record would erase
        // user-added keywords on a migration rerun.
        ap.filter.keywords = None;
    }

    if ap.filter.min_match_score == 50.0 {
        ap.filter.min_match_score = 0.0;
    }

    // `"24h"` is the ONLY restrictive legacy auto-default: the pre-#483 wizard's
    // `buildDefaults` set `dateFilter: '24h'`, while "any time" persisted as `None`
    // (`wizardStateToPayload` maps `'' → undefined`). A user-picked `'week'`/
    // `'month'` is therefore deliberate and left untouched.
    if ap.target.date_filter.as_deref() == Some("24h") {
        ap.target.date_filter = None;
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

/// Canonical dedup key for a found job.
///
/// Uses the app-wide [`crate::applications::normalize_job_url`] (lowercased host,
/// `www.`/fragment/tracking-param stripped) so tracking-param/hash variants of the
/// SAME URL collapse to a single row — this is same-host URL-variant dedup, not
/// cross-provider identity: an aggregator's `redirect_url` (e.g. Adzuna's `FoundJob.url`,
/// hosted on an adzuna domain) normalizes to a different key than a board's direct
/// posting URL, so the same job surfaced by both sources is NOT collapsed here (that
/// needs redirect-chain canonicalization, deferred to trust-program PR E). Falls back
/// to a normalized `title \u{1} company` when the URL is missing/unusable, so URL-less
/// postings still dedupe sanely (the control-char separator can't be reproduced by a
/// title that merely contains the company name) — known limitation: two distinct
/// URL-less postings that happen to share title+company will also merge; rare, and
/// only affects the discovery surface, so it's an accepted trade-off.
fn merge_key(j: &FoundJob) -> String {
    let normalized = crate::applications::normalize_job_url(&j.url);
    if !normalized.is_empty() {
        normalized
    } else {
        format!(
            "{}\u{1}{}",
            j.title.trim().to_lowercase(),
            j.company.trim().to_lowercase()
        )
    }
}

/// Byte length of a job's description (0 when absent) — used to keep the richer of
/// two same-key postings when collapsing a within-batch duplicate.
fn description_len(j: &FoundJob) -> usize {
    j.description.as_deref().map(str::len).unwrap_or(0)
}

/// Merge a fresh run's postings into the cumulative found-jobs list, idempotently:
///
/// - the incoming batch is first collapsed on [`merge_key`], so the SAME job
///   surfaced twice in one batch, or via two tracking-param/hash URL variants,
///   becomes ONE row — and counts once in the "N new jobs" notification (this
///   does NOT collapse an aggregator's redirect URL against a board's direct
///   URL for the same job; see [`merge_key`]),
/// - existing rows are kept (preserving `found_at`/first-seen) and have `is_new`
///   cleared; if the run re-surfaced them (matched by `merge_key`), volatile
///   fields (title/company/description/score/location/salary/board/trust) refresh,
/// - postings whose key was never seen are placed first (on top) and flagged
///   `is_new`, in the incoming (already score-sorted) order.
///
/// Re-running with the same postings yields the same set — only `is_new` moves.
/// `applied` is derived on read, so it is left at its default here.
fn merge_found_jobs(existing: &[FoundJob], incoming: Vec<FoundJob>) -> Vec<FoundJob> {
    use std::collections::{HashMap, HashSet};

    // 1) Collapse duplicates WITHIN the incoming batch by canonical key. First
    //    occurrence keeps its position; a later duplicate carrying a longer
    //    description upgrades that one field (richer text for the tailor flow).
    let mut order: Vec<String> = Vec::new();
    let mut by_key: HashMap<String, FoundJob> = HashMap::new();
    for job in incoming {
        let key = merge_key(&job);
        match by_key.get_mut(&key) {
            Some(kept) => {
                if description_len(&job) > description_len(kept) {
                    kept.description = job.description;
                }
            }
            None => {
                order.push(key.clone());
                by_key.insert(key, job);
            }
        }
    }
    let incoming: Vec<FoundJob> = order
        .into_iter()
        .map(|k| by_key.remove(&k).expect("key inserted above"))
        .collect();

    // 2) Merge the de-duplicated batch against existing rows, keyed the same way so
    //    a re-surfaced posting matches regardless of which URL variant/source
    //    persisted it.
    let incoming_by_key: HashMap<String, &FoundJob> =
        incoming.iter().map(|j| (merge_key(j), j)).collect();

    let refreshed_existing: Vec<FoundJob> = existing
        .iter()
        .map(|e| {
            let mut row = e.clone();
            row.is_new = false;
            if let Some(inc) = incoming_by_key.get(&merge_key(e)) {
                row.title = inc.title.clone();
                row.company = inc.company.clone();
                if inc.location.is_some() {
                    row.location = inc.location.clone();
                }
                // Carry the board over: existing rows persisted before `board` existed
                // (`None`) pick it up when the same job re-surfaces; the append branch
                // (`..inc`) preserves it for never-seen jobs.
                if inc.board.is_some() {
                    row.board = inc.board.clone();
                }
                if inc.description.is_some() {
                    row.description = inc.description.clone();
                }
                if inc.score.is_some() {
                    row.score = inc.score;
                }
                // Same fill-without-clobbering pattern as `board`/`description`: a
                // re-scrape that newly learns the salary updates the row, but never
                // overwrites an already-known value with an unknown one.
                if inc.salary_min.is_some() {
                    row.salary_min = inc.salary_min;
                }
                if inc.salary_max.is_some() {
                    row.salary_max = inc.salary_max;
                }
                if inc.salary_currency.is_some() {
                    row.salary_currency = inc.salary_currency.clone();
                }
                // Same legacy-migration case as `board` above: an existing row
                // persisted before `trust` existed (`None`) picks it up when the
                // same job re-surfaces on a later run.
                if inc.trust.is_some() {
                    row.trust = inc.trust.clone();
                }
            }
            row
        })
        .collect();

    let existing_keys: HashSet<String> = existing.iter().map(merge_key).collect();
    // New jobs go on top: the batch is already score-sorted desc upstream
    // (commands/autopilot.rs), so preserving incoming order here keeps that.
    let mut merged: Vec<FoundJob> = incoming
        .into_iter()
        .filter(|inc| !existing_keys.contains(&merge_key(inc)))
        .map(|inc| FoundJob {
            is_new: true,
            applied: false,
            ..inc
        })
        .collect();
    merged.extend(refreshed_existing);

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
