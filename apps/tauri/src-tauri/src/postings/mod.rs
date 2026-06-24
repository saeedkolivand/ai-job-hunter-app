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
    /// Insert a streamed posting, upserting by its `"id"` string.
    ///
    /// "Show more" re-scrapes with the same search signature (`replace=false`), so
    /// the same postings stream in again and would otherwise be appended a second
    /// time — the backend cache returned by `scrape_list_postings` then contained
    /// duplicates of the first batch. To prevent that, an incoming item whose `"id"`
    /// already exists **replaces that entry in place** (preserving its position /
    /// insertion order); the latest copy wins. An item with no `"id"` or a null id
    /// always pushes — distinct id-less rows must not be collapsed onto each other.
    ///
    /// Linear scan over a `Vec` is correct here: the frontend caps the list at ~500
    /// items, so the O(n) scan is cheap and a HashMap/index map would be premature.
    /// Mirrors the existing linear-scan-by-`id` in [`Self::update_description`].
    pub fn add(&mut self, item: Value) {
        if let Some(incoming_id) = item.get("id").and_then(Value::as_str) {
            if let Some(existing) = self.items.iter_mut().find(|existing| {
                existing.get("id").and_then(Value::as_str) == Some(incoming_id)
            }) {
                *existing = item;
                return;
            }
        }
        self.items.push(item);
    }

    pub fn get_all(&self) -> &[Value] {
        &self.items
    }

    /// Patch the `description` of the cached posting whose `id` matches, in place.
    ///
    /// Aggregator list scrapes store only a truncated snippet; once the detail
    /// pane resolves the full description we write it back here so the match
    /// scorer (which reads title+description+requirements from this cache) sees
    /// the full text. We mutate the EXISTING entry rather than pushing a new one:
    /// [`Self::add`] upserts by id (it would replace, not duplicate, the row) and
    /// the scorer's `job_texts_for` is first-wins by id, so routing the patch
    /// through `add` would needlessly rebuild the whole value. Each item is stored
    /// as a JSON object, so we patch the `description` field on the matching object
    /// directly.
    ///
    /// Returns `true` when an entry was updated, `false` when no item carries that
    /// id (no row is created in either case).
    pub fn update_description(&mut self, id: &str, description: &str) -> bool {
        // Two-pass approach to avoid holding a simultaneous mutable borrow on
        // `self.items` while also mutating `self.embeddings`.
        //
        // Pass 1: check whether the description is actually changing and patch
        //         the item. Track whether an invalidation is needed.
        let mut needs_embedding_invalidation = false;
        let mut found = false;
        for item in &mut self.items {
            if item.get("id").and_then(Value::as_str) == Some(id) {
                if let Some(obj) = item.as_object_mut() {
                    // Only invalidate the cached embedding when the text actually
                    // changes. If the full description is identical to what's
                    // already stored (e.g. a duplicate resolve-on-open call) we
                    // keep the embedding; otherwise the stale snippet embedding
                    // would be reused on the next score after a description update,
                    // defeating the resolve-on-open re-score.
                    let existing = obj.get("description").and_then(Value::as_str).unwrap_or("");
                    if existing != description {
                        needs_embedding_invalidation = true;
                    }
                    obj.insert(
                        "description".to_string(),
                        Value::String(description.to_string()),
                    );
                    found = true;
                    break;
                }
            }
        }
        // Pass 2: drop the stale embedding now that `self.items` borrow is released.
        if needs_embedding_invalidation {
            self.embeddings.remove(id);
        }
        found
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
    /// Set true only when a corrupt `interactions.json` was detected but the
    /// backup rename FAILED (file locked / cross-device / permissions). While it
    /// is set, `save` refuses to write — overwriting the path would clobber the
    /// un-backed-up corrupt original and lose the user's recoverable data. A
    /// successful backup leaves this false so `save` writes fresh data normally.
    block_save: bool,
}

impl InteractionStore {
    pub fn new(data_dir: &PathBuf) -> Self {
        std::fs::create_dir_all(data_dir).ok();
        Self {
            data_file: data_dir.join("interactions.json"),
            cache: None,
            block_save: false,
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
    ///
    /// A *missing* file hydrates an empty map (first run). A file that *exists*
    /// but fails to parse is NOT silently treated as empty — that would let the
    /// next `save` overwrite it with an empty array, destroying every recorded
    /// interaction. Instead the corrupt file is backed up (see
    /// [`Self::back_up_corrupt_file`]) before we start with an empty map, so the
    /// data is recoverable and the next `save` can't clobber the original.
    fn map_mut(&mut self) -> &mut HashMap<InteractionKey, InteractionRecord> {
        if self.cache.is_none() {
            let loaded: Vec<InteractionRecord> = match std::fs::read_to_string(&self.data_file) {
                // No file yet — first run, an empty map is correct.
                Err(_) => Vec::new(),
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(records) => records,
                    // The file exists but is malformed. Preserve it before the
                    // store can overwrite it with an empty map on the next save.
                    Err(err) => {
                        self.back_up_corrupt_file(&err);
                        Vec::new()
                    }
                },
            };
            self.cache = Some(
                loaded
                    .into_iter()
                    .map(|r| ((r.job_id.clone(), r.interaction_type.clone()), r))
                    .collect(),
            );
        }
        self.cache.as_mut().expect("cache just initialized")
    }

    /// Move a malformed `interactions.json` aside to `interactions.json.corrupt`
    /// so a parse failure never silently discards the user's data. A fixed
    /// suffix (no timestamp/random source here) is enough: it survives the
    /// next `save`, which writes back to the original path. The error is logged
    /// via the shared tracing layer for diagnostics.
    ///
    /// If the rename FAILS (file locked / cross-device / permissions) the corrupt
    /// original still sits at `data_file`, so we set `block_save` to stop the next
    /// `save` from overwriting it with an empty map. A successful rename frees the
    /// path and leaves `block_save` false so `save` proceeds normally.
    fn back_up_corrupt_file(&mut self, err: &serde_json::Error) {
        let backup = self.data_file.with_extension("json.corrupt");
        let renamed = std::fs::rename(&self.data_file, &backup).is_ok();
        if !renamed {
            self.block_save = true;
        }
        log::error!(
            "[postings] interactions.json failed to parse ({err}); \
             backed_up={renamed} backup={}",
            backup.display()
        );
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
        // A corrupt original is still sitting at `data_file` because its backup
        // rename failed. Writing now would clobber the only copy of the user's
        // recoverable data, so skip the write. The new interaction stays in
        // memory (lost on restart) — preserving the on-disk original wins.
        if self.block_save {
            log::error!(
                "[postings] save skipped: corrupt interactions.json could not be \
                 backed up; refusing to overwrite the un-backed-up original at {}",
                self.data_file.display()
            );
            return;
        }
        let records = self.records();
        if let Ok(json) = serde_json::to_string_pretty(&records) {
            std::fs::write(&self.data_file, json).ok();
        }
    }
}

#[cfg(test)]
mod test;
