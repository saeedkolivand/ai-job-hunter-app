//! Writing the autopilot store to disk. Kept out of `autopilot/mod.rs` for the
//! R8 module-size cap.

use std::collections::HashMap;

use super::{cmp_autopilot_newest_first, Autopilot, AutopilotStore};
use crate::platform::fs::write_atomic;

impl AutopilotStore {
    /// Persist the map: found jobs to SQLite (see `found_jobs_db.rs`), then
    /// `autopilots.json`. Returns the IO outcome so a caller can gate on a
    /// successful persist (e.g. the one-shot migration's done-marker). Does NOT
    /// update the in-memory cache; that's `save`'s job. `Ok(())` is also
    /// returned on the no-op-write path (state already on disk).
    pub(super) fn write_to_disk(&self, map: &HashMap<String, Autopilot>) -> std::io::Result<()> {
        // The file on disk couldn't be loaded safely (unreadable, or corrupt with
        // no backup slot to move it to): writing now could replace the only copy
        // of the user's data with this session's empty map. The new state stays in
        // memory (lost on restart); preserving the on-disk file wins. `save` logs it.
        if self.is_block_save() {
            return Err(std::io::Error::other(
                "autopilots.json could not be loaded safely; not overwriting it",
            ));
        }

        // Found jobs first. Only when they are safely in SQLite does the JSON
        // leave them out; if the database is missing or the write failed, the
        // JSON keeps carrying them, so nothing is lost.
        let without_found_jobs: Vec<Autopilot>;
        let mut list: Vec<&Autopilot> = if self.persist_found_jobs(map) {
            without_found_jobs = map
                .values()
                .map(|ap| Autopilot {
                    found_jobs: Vec::new(),
                    ..ap.clone()
                })
                .collect();
            without_found_jobs.iter().collect()
        } else {
            map.values().collect()
        };
        list.sort_by(|a, b| cmp_autopilot_newest_first(a, b));

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
        // With found jobs out of the file this re-read is small.
        let unchanged = std::fs::read_to_string(&self.data_file)
            .map(|existing| existing == json)
            .unwrap_or(false);
        if unchanged {
            return Ok(()); // desired state already persisted
        }
        write_atomic(&self.data_file, json.as_bytes())
    }
}
