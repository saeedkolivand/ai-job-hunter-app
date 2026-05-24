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

// ── PostingsCache ─────────────────────────────────────────────────────────────

/// Live job postings received during an active scrape.
/// Cleared on `scrape_clear_postings` or on next scrape start for the same board.
#[derive(Default)]
pub struct PostingsCache {
    items: Vec<Value>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_postings_cache_default() {
        let cache = PostingsCache::default();
        assert!(cache.get_all().is_empty());
    }

    #[test]
    fn test_postings_cache_add() {
        let mut cache = PostingsCache::default();
        let item = serde_json::json!({"id": "1", "title": "Test"});
        cache.add(item);
        assert_eq!(cache.get_all().len(), 1);
    }

    #[test]
    fn test_postings_cache_clear() {
        let mut cache = PostingsCache::default();
        let item = serde_json::json!({"id": "1", "title": "Test"});
        cache.add(item);
        cache.clear_all();
        assert!(cache.get_all().is_empty());
    }

    #[test]
    fn test_interaction_record_serialization() {
        let record = InteractionRecord {
            job_id: "job-1".to_string(),
            interaction_type: "viewed".to_string(),
            timestamp: 1234567890,
            title: "Software Engineer".to_string(),
            company: "Test Corp".to_string(),
            url: "https://example.com".to_string(),
            source: "linkedin".to_string(),
            location: "Remote".to_string(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let deserialized: InteractionRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.job_id, record.job_id);
        assert_eq!(deserialized.interaction_type, record.interaction_type);
    }

    #[test]
    fn test_interaction_store_new() {
        let data_dir = std::path::PathBuf::from("/tmp/test_data_new");
        let store = InteractionStore::new(&data_dir);
        assert!(store.data_file.ends_with("interactions.json"));
    }

    #[test]
    fn test_interaction_store_list_no_filter() {
        let data_dir = std::path::PathBuf::from("/tmp/test_data_list");
        let _ = std::fs::remove_dir_all(&data_dir);
        let mut store = InteractionStore::new(&data_dir);
        let records = store.list(None);
        assert!(records.is_empty());
    }

    #[test]
    fn test_interaction_store_list_with_filter() {
        let data_dir = std::path::PathBuf::from("/tmp/test_data_filter");
        let _ = std::fs::remove_dir_all(&data_dir);
        let mut store = InteractionStore::new(&data_dir);
        let record = InteractionRecord {
            job_id: "job-1".to_string(),
            interaction_type: "viewed".to_string(),
            timestamp: 1234567890,
            title: "Test".to_string(),
            company: "Test".to_string(),
            url: "https://example.com".to_string(),
            source: "test".to_string(),
            location: "Remote".to_string(),
        };
        store.upsert(record.clone());
        let viewed = store.list(Some("viewed"));
        assert_eq!(viewed.len(), 1);
        let applied = store.list(Some("applied"));
        assert!(applied.is_empty());
    }

    #[test]
    fn test_interaction_store_upsert_new() {
        let data_dir = std::path::PathBuf::from("/tmp/test_data_upsert_new");
        let _ = std::fs::remove_dir_all(&data_dir);
        let mut store = InteractionStore::new(&data_dir);
        let record = InteractionRecord {
            job_id: "job-1".to_string(),
            interaction_type: "viewed".to_string(),
            timestamp: 1234567890,
            title: "Test".to_string(),
            company: "Test".to_string(),
            url: "https://example.com".to_string(),
            source: "test".to_string(),
            location: "Remote".to_string(),
        };
        store.upsert(record.clone());
        let records = store.list(None);
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_interaction_store_upsert_update() {
        let data_dir = std::path::PathBuf::from("/tmp/test_data_upsert_update");
        let _ = std::fs::remove_dir_all(&data_dir);
        let mut store = InteractionStore::new(&data_dir);
        let mut record = InteractionRecord {
            job_id: "job-1".to_string(),
            interaction_type: "viewed".to_string(),
            timestamp: 1234567890,
            title: "Test".to_string(),
            company: "Test".to_string(),
            url: "https://example.com".to_string(),
            source: "test".to_string(),
            location: "Remote".to_string(),
        };
        store.upsert(record.clone());
        record.timestamp = 1234567891;
        store.upsert(record.clone());
        let records = store.list(None);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].timestamp, 1234567891);
    }

    #[test]
    fn test_interaction_store_clear_all() {
        let data_dir = std::path::PathBuf::from("/tmp/test_data_clear");
        let _ = std::fs::remove_dir_all(&data_dir);
        let mut store = InteractionStore::new(&data_dir);
        let record = InteractionRecord {
            job_id: "job-1".to_string(),
            interaction_type: "viewed".to_string(),
            timestamp: 1234567890,
            title: "Test".to_string(),
            company: "Test".to_string(),
            url: "https://example.com".to_string(),
            source: "test".to_string(),
            location: "Remote".to_string(),
        };
        store.upsert(record);
        store.clear_all();
        let records = store.list(None);
        assert!(records.is_empty());
    }

    #[test]
    fn test_interaction_store_export_all() {
        let data_dir = std::path::PathBuf::from("/tmp/test_data_export");
        let _ = std::fs::remove_dir_all(&data_dir);
        let mut store = InteractionStore::new(&data_dir);
        let record = InteractionRecord {
            job_id: "job-1".to_string(),
            interaction_type: "viewed".to_string(),
            timestamp: 1234567890,
            title: "Test".to_string(),
            company: "Test".to_string(),
            url: "https://example.com".to_string(),
            source: "test".to_string(),
            location: "Remote".to_string(),
        };
        store.upsert(record.clone());
        let exported = store.export_all();
        assert_eq!(exported.len(), 1);
    }

    #[test]
    fn test_interaction_store_import_bundle() {
        let data_dir = std::path::PathBuf::from("/tmp/test_data_import");
        let _ = std::fs::remove_dir_all(&data_dir);
        let mut store = InteractionStore::new(&data_dir);
        let records = vec![
            InteractionRecord {
                job_id: "job-1".to_string(),
                interaction_type: "viewed".to_string(),
                timestamp: 1234567890,
                title: "Test".to_string(),
                company: "Test".to_string(),
                url: "https://example.com".to_string(),
                source: "test".to_string(),
                location: "Remote".to_string(),
            },
        ];
        let imported = store.import_bundle(records);
        assert_eq!(imported, 1);
    }
}
