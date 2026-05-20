/// PostingsCache — buffers live job items streamed from the sidecar via SSE.
/// InteractionStore — records user interactions (viewed, applied, bookmarked).
///
/// Both are in-memory with optional JSON file persistence.
/// The Electron equivalents are DataRuntime.liveJobs and the NeDB jobInteractions
/// collection. This Rust version intentionally avoids a full DB dependency —
/// data is written to <dataDir>/interactions.json as a flat JSON array.
use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── PostingsCache ─────────────────────────────────────────────────────────────

/// Live job postings received from the sidecar during an active scrape.
/// Cleared on `scrape_clear_postings` or on next scrape start for the same board.
#[derive(Default)]
pub struct PostingsCache {
    items: Vec<Value>,
}

impl PostingsCache {
    pub fn add(&mut self, item: Value) {
        self.items.push(item);
    }

    pub fn get_all(&self) -> &[Value] {
        &self.items
    }

    pub fn clear_all(&mut self) {
        self.items.clear();
    }
}

// ── InteractionStore ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionRecord {
    pub job_id: String,
    pub interaction_type: String,
    pub timestamp: u64,
    pub title: String,
    pub company: String,
    pub url: String,
    pub source: String,
    pub location: String,
}

pub struct InteractionStore {
    data_file: PathBuf,
    cache: Option<Vec<InteractionRecord>>,
}

impl InteractionStore {
    pub fn new(data_dir: &PathBuf) -> Self {
        std::fs::create_dir_all(data_dir).ok();
        Self {
            data_file: data_dir.join("interactions.json"),
            cache: None,
        }
    }

    pub fn list(&mut self, filter_type: Option<&str>) -> Vec<InteractionRecord> {
        let all = self.load();
        match filter_type {
            Some(t) => all.into_iter().filter(|r| r.interaction_type == t).collect(),
            None => all,
        }
    }

    pub fn upsert(&mut self, record: InteractionRecord) {
        let mut all = self.load();
        if let Some(existing) = all
            .iter_mut()
            .find(|r| r.job_id == record.job_id && r.interaction_type == record.interaction_type)
        {
            *existing = record;
        } else {
            all.push(record);
        }
        self.save(all);
    }

    pub fn clear_all(&mut self) {
        self.save(vec![]);
    }

    /// Export all interactions for the data export feature.
    pub fn export_all(&mut self) -> Vec<InteractionRecord> {
        self.load()
    }

    /// Import from an exported bundle, upserting each record.
    pub fn import_bundle(&mut self, records: Vec<InteractionRecord>) -> usize {
        let mut all = self.load();
        let mut imported = 0;
        let mut index: HashMap<(String, String), usize> = all
            .iter()
            .enumerate()
            .map(|(i, r)| ((r.job_id.clone(), r.interaction_type.clone()), i))
            .collect();

        for record in records {
            let key = (record.job_id.clone(), record.interaction_type.clone());
            if let Some(&idx) = index.get(&key) {
                all[idx] = record;
            } else {
                index.insert(key, all.len());
                all.push(record);
                imported += 1;
            }
        }
        self.save(all);
        imported
    }

    fn load(&mut self) -> Vec<InteractionRecord> {
        if let Some(ref c) = self.cache {
            return c.clone();
        }
        let loaded: Vec<InteractionRecord> = std::fs::read_to_string(&self.data_file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        self.cache = Some(loaded.clone());
        loaded
    }

    fn save(&mut self, records: Vec<InteractionRecord>) {
        if let Ok(json) = serde_json::to_string_pretty(&records) {
            std::fs::write(&self.data_file, json).ok();
        }
        self.cache = Some(records);
    }
}
