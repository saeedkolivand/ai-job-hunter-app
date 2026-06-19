/// PostingsCache — buffers live job items streamed from the in-process scraper engine.
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

use crate::commands::ai_provider::EmbeddingVector;

// ── PostingsCache ─────────────────────────────────────────────────────────────

/// Live job postings received during an active scrape.
/// Cleared on `scrape_clear_postings` or on next scrape start for the same board.
#[derive(Default)]
pub struct PostingsCache {
    items: Vec<Value>,
    /// Embedding cache keyed by posting id, populated lazily by hybrid search so
    /// repeat searches over the same live postings don't re-embed. Each entry
    /// carries its embedding space so stale-space entries can be detected.
    embeddings: HashMap<String, EmbeddingVector>,
}

impl PostingsCache {
    #[allow(dead_code)]
    pub fn add(&mut self, item: Value) {
        self.items.push(item);
    }

    pub fn get_all(&self) -> &[Value] {
        &self.items
    }

    pub fn clear_all(&mut self) {
        self.items.clear();
        self.embeddings.clear();
    }

    pub fn get_embedding(&self, id: &str) -> Option<EmbeddingVector> {
        self.embeddings.get(id).cloned()
    }

    pub fn set_embedding(&mut self, id: String, vector: EmbeddingVector) {
        self.embeddings.insert(id, vector);
    }

    /// Drop cached embeddings (keeping items) — used when the embedding space
    /// changes so stale-space vectors aren't reused.
    pub fn clear_embeddings(&mut self) {
        self.embeddings.clear();
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

/// Key into the in-memory interaction map: a record is unique per
/// (`job_id`, `interaction_type`) — the same identity the old linear scan used.
type InteractionKey = (String, String);

pub struct InteractionStore {
    data_file: PathBuf,
    /// In-memory map keyed by (job_id, interaction_type) for O(1) upsert. Lazily
    /// hydrated from disk on first access; the flat JSON array on disk is the
    /// source of truth and is rebuilt from this map on every `save`. Replaces the
    /// old `Option<Vec<_>>` cache that forced an O(n) linear scan + a clone of the
    /// whole vector on every interaction event.
    cache: Option<HashMap<InteractionKey, InteractionRecord>>,
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
        let all = self.records();
        match filter_type {
            Some(t) => all
                .into_iter()
                .filter(|r| r.interaction_type == t)
                .collect(),
            None => all,
        }
    }

    pub fn upsert(&mut self, record: InteractionRecord) {
        let map = self.map_mut();
        // O(1): a re-interaction with the same (job_id, type) overwrites in place;
        // a new pair inserts. No full-vector clone, no linear scan.
        map.insert(
            (record.job_id.clone(), record.interaction_type.clone()),
            record,
        );
        self.save();
    }

    pub fn clear_all(&mut self) {
        self.cache = Some(HashMap::new());
        self.save();
    }

    /// Export all interactions for the data export feature.
    pub fn export_all(&mut self) -> Vec<InteractionRecord> {
        self.records()
    }

    /// Import from an exported bundle, upserting each record. Returns the count
    /// of records that were newly inserted (not overwrites), matching the old
    /// behavior exactly.
    pub fn import_bundle(&mut self, records: Vec<InteractionRecord>) -> usize {
        let map = self.map_mut();
        let mut imported = 0;
        for record in records {
            let key = (record.job_id.clone(), record.interaction_type.clone());
            if map.insert(key, record).is_none() {
                imported += 1;
            }
        }
        self.save();
        imported
    }

    /// Borrow the in-memory map, hydrating it from disk on first access.
    fn map_mut(&mut self) -> &mut HashMap<InteractionKey, InteractionRecord> {
        if self.cache.is_none() {
            let loaded: Vec<InteractionRecord> = std::fs::read_to_string(&self.data_file)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            self.cache = Some(
                loaded
                    .into_iter()
                    .map(|r| ((r.job_id.clone(), r.interaction_type.clone()), r))
                    .collect(),
            );
        }
        self.cache.as_mut().expect("cache just initialized")
    }

    /// Snapshot the records in a deterministic order (newest first, then by
    /// id/type) so both `list`/`export_all` and the on-disk file are stable
    /// across runs despite the unordered map.
    fn records(&mut self) -> Vec<InteractionRecord> {
        let mut all: Vec<InteractionRecord> = self.map_mut().values().cloned().collect();
        all.sort_by(|a, b| {
            b.timestamp
                .cmp(&a.timestamp)
                .then_with(|| a.job_id.cmp(&b.job_id))
                .then_with(|| a.interaction_type.cmp(&b.interaction_type))
        });
        all
    }

    /// Persist the current map as the flat JSON array the on-disk format expects
    /// (unchanged shape). Serializes the deterministic snapshot so the file is
    /// stable between writes.
    fn save(&mut self) {
        let records = self.records();
        if let Ok(json) = serde_json::to_string_pretty(&records) {
            std::fs::write(&self.data_file, json).ok();
        }
    }
}

#[cfg(test)]
mod test;
