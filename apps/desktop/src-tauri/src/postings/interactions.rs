//! Per-posting user interactions (viewed, applied, bookmarked): the record, the
//! flat-JSON-file [`InteractionStore`] that persists them, and the join that
//! projects them onto cached postings for the renderer.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::observability::sanitize_reason;
use crate::platform::fs::write_atomic;

/// Join the recorded interactions onto each cached posting so the jobs list can
/// render viewed/applied/saved state.
///
/// `scrape_list_postings` returns the raw [`PostingsCache`] items, which never
/// carry interactions (those live in the [`InteractionStore`]). Without this join
/// `posting.interactions` is always empty in the renderer and no badges show.
///
/// Each returned object gets an `interactions` array of the records whose
/// `job_id` equals the posting's string `"id"`, projected onto the renderer's
/// `JobInteraction` contract (`packages/shared/src/types/index.ts`:
/// `{ jobId, title, company, url, source, location?, interactionType, timestamp }`).
/// We map each record to those fields EXPLICITLY rather than serializing the whole
/// [`InteractionRecord`], so adding a storage-only field later can't silently leak
/// into this IPC response or drift from the shared contract. An item with no `"id"`
/// — or one whose id has no recorded interactions — gets an empty array, keeping
/// the posting shape stable. The records are grouped once into a map, so the join
/// is O(postings + interactions) (n ≤ ~500).
pub fn attach_interactions(items: &[Value], interactions: &[InteractionRecord]) -> Vec<Value> {
    let mut by_job_id: HashMap<&str, Vec<&InteractionRecord>> = HashMap::new();
    for record in interactions {
        by_job_id
            .entry(record.job_id.as_str())
            .or_default()
            .push(record);
    }

    items
        .iter()
        .map(|item| {
            let mut item = item.clone();
            let matched: Vec<Value> = item
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| by_job_id.get(id))
                .map_or_else(Vec::new, |records| {
                    records.iter().map(|r| interaction_value(r)).collect()
                });
            if let Some(obj) = item.as_object_mut() {
                obj.insert("interactions".to_string(), Value::Array(matched));
            }
            item
        })
        .collect()
}

/// Project an [`InteractionRecord`] onto the renderer's `JobInteraction` contract,
/// carrying ONLY the contract fields. Decouples the IPC response from the storage
/// struct so a future storage-only field can't leak into `scrape_list_postings`.
///
/// `interactionType` is a strict union in the shared contract
/// (`viewed | opened | applied | bookmarked | dismissed`), but the persisted
/// value is a free `String` on disk — a corrupt or unexpected entry must not
/// break the cross-layer contract at runtime, so an out-of-union value is
/// coerced to `"viewed"`.
fn interaction_value(record: &InteractionRecord) -> Value {
    // ponytail: clamp the on-disk type to the shared union; unknown → "viewed"
    // (the most benign default — it only dims the row, never marks applied/saved).
    let interaction_type = match record.interaction_type.as_str() {
        "viewed" | "opened" | "applied" | "bookmarked" | "dismissed" => {
            record.interaction_type.as_str()
        }
        _ => "viewed",
    };
    json!({
        "jobId": record.job_id.as_str(),
        "title": record.title.as_str(),
        "company": record.company.as_str(),
        "url": record.url.as_str(),
        "source": record.source.as_str(),
        "location": record.location.as_str(),
        "interactionType": interaction_type,
        "timestamp": record.timestamp,
    })
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

    /// Remove the interaction keyed by `(job_id, interaction_type)` — the exact
    /// key `upsert` writes under. The real "undo" for a persisted interaction
    /// (e.g. reversing a `dismissed` write): unlike a client-side-only revert,
    /// this deletes the record so a later `list`/`compute_best_matches` pass no
    /// longer sees it. Returns `true` when a record was removed, `false` when
    /// there was nothing to remove — callers need to tell "undone" apart from
    /// "there was nothing there". Only writes to disk when something actually
    /// changed, matching `upsert`'s save-on-write behavior without an idle
    /// rewrite when the key was already absent.
    pub fn remove(&mut self, job_id: &str, interaction_type: &str) -> bool {
        let map = self.map_mut();
        let removed = map
            .remove(&(job_id.to_string(), interaction_type.to_string()))
            .is_some();
        if removed {
            self.save();
        }
        removed
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
        // Log the backup FILE NAME only, never the full path — the data dir is
        // under the user's home directory and a `.display()` here would put an
        // absolute, username-bearing path into logs (which ship in diagnostics
        // bundles the user may send us).
        log::error!(
            "[postings] interactions.json failed to parse ({err}); \
             backed_up={renamed} backup_name={}",
            file_name_label(&backup)
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
            // No path here — "corrupt interactions.json" already identifies which
            // file, and a full path would leak the user's home directory into logs.
            log::error!(
                "[postings] save skipped: corrupt interactions.json could not be \
                 backed up; refusing to overwrite the un-backed-up original"
            );
            return;
        }
        let records = self.records();
        let json = match serde_json::to_string_pretty(&records) {
            Ok(json) => json,
            Err(e) => {
                log::error!(
                    "[postings] save skipped: could not serialize {} interaction(s): {e}",
                    records.len()
                );
                return;
            }
        };
        // Use the shared atomic write helper so we gain `sync_all` and a
        // single, tested code path. The helper writes to a sibling `.tmp` file,
        // syncs it, then renames — preserving the "no .tmp left behind" test
        // contract.
        if let Err(e) = write_atomic(&self.data_file, json.as_bytes()) {
            log::error!(
                "[postings] failed to write {}: {} — interaction NOT persisted",
                file_name_label(&self.data_file),
                sanitize_reason(&e.to_string())
            );
        }
    }
}

/// The final path component as a diagnostic label — never the full path. The
/// data dir this file lives under is inside the user's home directory, and
/// these `log::error!` calls end up in `crashes.log` / diagnostics bundles
/// users send us, so only the file name (e.g. `interactions.json.tmp`) is
/// logged, never an absolute, username-bearing path.
fn file_name_label(path: &std::path::Path) -> &str {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("<interactions-file>")
}

#[cfg(test)]
mod tests;
