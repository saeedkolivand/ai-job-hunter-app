/// AutopilotStore — JSON-file-backed CRUD for Autopilot records.
///
/// Mirrors packages/data/src/autopilot/store.ts (NeDB) with the same public
/// API surface so the renderer's existing autopilot hooks work unchanged.
///
/// Records are persisted to <dataDir>/autopilots.json as a flat JSON array.
/// All field names are serialised in camelCase to match the TypeScript schema
/// (`#[serde(rename_all = "camelCase")]`).
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub action: String,
    pub schedule: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_letter: Option<String>,
    pub auto_submit: bool,
    pub total_found: u32,
    pub total_applied: u32,
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
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
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
            filter: serde_json::from_value(input["filter"].clone()).unwrap_or_else(|_| {
                AutopilotFilter {
                    min_match_score: 50.0,
                    keywords: None,
                    exclude_keywords: None,
                }
            }),
            action: str_field(&input, "action"),
            schedule: str_field(&input, "schedule"),
            resume_text: input["resumeText"].as_str().map(String::from),
            cover_letter: input["coverLetter"].as_str().map(String::from),
            auto_submit: input["autoSubmit"].as_bool().unwrap_or(false),
            total_found: 0,
            total_applied: 0,
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
        if let Some(v) = patch.get("action").and_then(|v| v.as_str()) {
            ap.action = v.to_string();
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
        if let Some(v) = patch.get("autoSubmit").and_then(|v| v.as_bool()) {
            ap.auto_submit = v;
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
        let mut guard = self.cache.lock().unwrap();
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
            v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            v
        };
        if let Ok(json) = serde_json::to_string_pretty(&list) {
            std::fs::write(&self.data_file, json).ok();
        }
        *self.cache.lock().unwrap() = Some(map);
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

#[cfg(test)]
mod test;
