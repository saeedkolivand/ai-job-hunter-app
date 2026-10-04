use parking_lot::Mutex;
/// AutopilotStore — JSON-file-backed CRUD for Autopilot records.
///
/// Records are persisted to <dataDir>/autopilots.json as a flat JSON array.
/// All field names are serialised in camelCase to match the TypeScript schema
/// (`#[serde(rename_all = "camelCase")]`).
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;

use uuid::Uuid;

mod cap;
mod corrupt;
mod found_jobs_db;
mod merge;
mod model;
mod persist;
mod relax;
mod runs;

pub(crate) use merge::found_job_cluster_inputs;
pub(crate) use model::derive_run_status;
pub use model::{
    Autopilot, AutopilotFilter, AutopilotStatus, AutopilotTarget, FoundJob, RunStatus, ScoreSource,
};

use model::default_top_n;

use crate::db::now_ms;
use crate::error::AppResult;
use crate::observability::sanitize_reason;

// ── Store ─────────────────────────────────────────────────────────────────────

pub struct AutopilotStore {
    data_file: PathBuf,
    cache: Mutex<Option<HashMap<String, Autopilot>>>,
    /// Set true only when a corrupt `autopilots.json` was detected but the
    /// backup rename FAILED (file locked / cross-device / permissions). While it
    /// is set, `write_to_disk` refuses to write — overwriting the path would
    /// clobber the un-backed-up corrupt original and lose the user's recoverable
    /// data. A successful backup leaves this false so `write_to_disk` proceeds
    /// normally.
    block_save: std::sync::atomic::AtomicBool,
    /// Where found jobs are persisted; `None` if the database couldn't be opened
    /// (found jobs then stay inside `autopilots.json`). See `found_jobs_db.rs`.
    found_jobs_db: Option<Mutex<found_jobs_db::FoundJobsDb>>,
}

impl AutopilotStore {
    pub fn new(data_dir: &PathBuf) -> Self {
        std::fs::create_dir_all(data_dir).ok();
        Self {
            data_file: data_dir.join("autopilots.json"),
            cache: Mutex::new(None),
            block_save: std::sync::atomic::AtomicBool::new(false),
            found_jobs_db: found_jobs_db::open_for_store(data_dir),
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
                    work_types: None,
                    pages: 1,
                    date_filter: None,
                    top_n: default_top_n(),
                    watched_companies_only: None,
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
            last_run_summaries: Vec::new(),
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
        self.forget_found_jobs(Some(id));
    }

    /// Remove every autopilot and its found-jobs history (factory reset).
    pub fn clear_all(&self) {
        self.forget_found_jobs(None);
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

        // See `corrupt.rs` for what can go wrong and what each outcome means.
        let outcome = self.load_with_corrupt_handling();
        self.set_block_save(outcome.block_save);
        // A blocked outcome (file unreadable, or corrupt with no backup slot) is
        // never cached, so the next load reads the file again instead of serving
        // this empty stand-in for the rest of the session.
        let mut map = outcome.map;
        if !outcome.block_save {
            // Found jobs live in SQLite (`found_jobs_db.rs`); a legacy file that
            // still carries them is migrated by this one persist.
            if self.hydrate_found_jobs(&mut map) {
                if let Err(e) = self.write_to_disk(&map) {
                    let reason = sanitize_reason(&e.to_string());
                    log::error!("[autopilot] found-jobs migration write failed: {reason}");
                }
            }
            *guard = Some(map.clone());
        }
        map
    }

    fn save(&self, map: HashMap<String, Autopilot>) {
        // A persistent-write failure must be LOUD, not swallowed: the in-memory
        // cache below would otherwise diverge from disk silently, and the next
        // reader (or a restart) would lose this state with no signal at all
        // (quick win 9). Smallest honest surface — a `log::error` with context;
        // deliberately NOT a retry queue. Migrations that need to *observe* a
        // successful persist still call `write_to_disk` directly.
        if let Err(e) = self.write_to_disk(&map) {
            log::error!(
                "[autopilot] failed to persist autopilots.json: {}",
                sanitize_reason(&e.to_string())
            );
            // Blocked because the file on disk couldn't be loaded safely: don't
            // cache this change either. The UI then shows it didn't stick right
            // away (instead of it vanishing on restart), and the next load
            // re-reads the file once it's readable again.
            if self.is_block_save() {
                return;
            }
        }
        *self.cache.lock() = Some(map);
    }

    /// Replace all autopilots with the given set (preserving their ids). Used by
    /// backup restore.
    pub fn replace_all(&self, items: Vec<Autopilot>) {
        let map: HashMap<String, Autopilot> =
            items.into_iter().map(|ap| (ap.id.clone(), ap)).collect();
        self.replace_found_jobs(&map); // one transaction: old rows out, new rows in
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

impl crate::data_store::DataStore for AutopilotStore {
    fn key(&self) -> &'static str {
        "autopilots"
    }

    fn export(&self) -> serde_json::Value {
        // Belt AND braces on the Track B1 board-health verdict. `record_run`
        // already strips it before persisting, so a record written by this build
        // carries none — but a record written by an intermediate build (or
        // restored from one) could, and this is the boundary where it would
        // leave the machine. The verdict is derived from the LOCAL
        // `scraping::board_health` store, which is deliberately not a
        // `DataStore`; letting it ride out inside `lastRunSummaries` would
        // replay this machine's failure streaks, error text and run ids on
        // whatever machine imports the bundle.
        let mut records = self.list();
        strip_board_health(records.iter_mut());
        serde_json::json!(records)
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        // Same boundary as `export`, the other direction: a legacy/tampered
        // backup can carry `lastRunSummaries[].health` (the field predates
        // this strip, or a hand-edited bundle), and `replace_all` below
        // persists whatever it's handed verbatim — closing this side of the
        // door is what `export`'s own "belt AND braces" comment describes.
        let mut items: Vec<Autopilot> =
            serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
        strip_board_health(items.iter_mut());
        let count = items.len();
        self.replace_all(items);
        Ok(count)
    }
}

/// Strip the Track B1 `health` verdict from every record's
/// `last_run_summaries`. It is a DISPLAY-TIME derivation of the live
/// `board_health` store, never persisted run state (see `record_run`'s own
/// strip, right above `last_run_summaries` in `runs.rs`) — this is the same
/// scrub applied at every boundary that can put non-`record_run`-authored
/// data into memory or onto disk: `export` (leaving the machine), `import` (a
/// legacy/tampered backup coming back in), and `AutopilotStore::load` (an
/// on-disk `autopilots.json` written by an intermediate build before this
/// strip existed, or edited by hand — `load`'s cache means a mutation
/// unrelated to `last_run_summaries` would otherwise re-persist such a
/// record's stale health forever).
fn strip_board_health<'a>(records: impl IntoIterator<Item = &'a mut Autopilot>) {
    for ap in records {
        for summary in &mut ap.last_run_summaries {
            summary.health = None;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
