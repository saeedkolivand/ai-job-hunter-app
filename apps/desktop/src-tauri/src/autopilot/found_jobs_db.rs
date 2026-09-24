//! Found jobs are persisted in SQLite, not inside `autopilots.json` (#1277).
//!
//! They were over 99% of that file (34 MB of about 6,700 job descriptions on
//! one machine), and the whole file was rewritten on every autopilot change.
//! Now `autopilots.json` holds only the autopilots themselves, and each found
//! job is one row keyed by (autopilot id, position). `Autopilot::found_jobs`
//! keeps its in-memory and IPC shape: nothing that reads found jobs changes.
//!
//! **Writes are diffed.** The hash of every row on disk is kept in memory, so a
//! save only writes rows whose content changed. A status change writes none.
//!
//! **Sync never deletes rows of an autopilot it wasn't given.** A load can come
//! back empty (a corrupt `autopilots.json` moved aside, #1274), and syncing that
//! must not wipe the found jobs, which may then be the only surviving copy. Rows
//! go only through the explicit paths: deleting an autopilot, restore, reset.
//!
//! **Migration** (legacy files with `foundJobs` inside): per autopilot, found
//! jobs still in the JSON win over rows; the rows are committed first and the
//! JSON is rewritten without them afterwards, so a crash in between reruns it.
//! The legacy file is copied once to `autopilots.json.pre-sqlite`.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection};

use super::{Autopilot, AutopilotStore, FoundJob};
use crate::db::{run_migrations, Migration};
use crate::error::{AppError, AppResult};
use crate::observability::sanitize_reason;

const DB_FILE: &str = "autopilot_found_jobs.db";

const MIGRATIONS: &[Migration] = &[Migration {
    name: "create_found_jobs",
    up: |c| {
        c.execute_batch(
            "CREATE TABLE found_jobs (
                autopilot_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                json TEXT NOT NULL,
                PRIMARY KEY (autopilot_id, position)
            ) WITHOUT ROWID;",
        )
    },
}];

pub(super) struct FoundJobsDb {
    conn: Connection,
    /// Hash of every row as it is on disk, keyed like the table.
    written: HashMap<(String, i64), u64>,
    /// Test-only: rows this instance has written, to prove what a save costs.
    #[cfg(test)]
    pub(super) rows_written: usize,
}

fn row_hash(json: &str) -> u64 {
    let mut h = DefaultHasher::new();
    json.hash(&mut h);
    h.finish()
}

impl FoundJobsDb {
    pub(super) fn open(data_dir: &Path) -> AppResult<Self> {
        let mut conn = crate::db::open(&data_dir.join(DB_FILE))?;
        run_migrations(&mut conn, MIGRATIONS)?;
        Ok(Self {
            conn,
            written: HashMap::new(),
            #[cfg(test)]
            rows_written: 0,
        })
    }

    /// Test-only: how many rows are stored for one autopilot.
    #[cfg(test)]
    pub(super) fn row_count(&self, id: &str) -> usize {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM found_jobs WHERE autopilot_id = ?1",
                params![id],
                |r| r.get::<_, i64>(0),
            )
            .unwrap() as usize
    }

    /// Every stored row, grouped per autopilot in position order.
    fn load_all(&mut self) -> AppResult<HashMap<String, Vec<FoundJob>>> {
        let rows: Vec<(String, i64, String)> = {
            let mut stmt = self.conn.prepare(
                "SELECT autopilot_id, position, json FROM found_jobs
                 ORDER BY autopilot_id, position",
            )?;
            let mapped = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            mapped.collect::<rusqlite::Result<_>>()?
        };
        let mut out: HashMap<String, Vec<FoundJob>> = HashMap::new();
        for (id, position, json) in rows {
            match serde_json::from_str::<FoundJob>(&json) {
                Ok(job) => {
                    self.written.insert((id.clone(), position), row_hash(&json));
                    out.entry(id).or_default().push(job);
                }
                Err(e) => log::warn!(
                    "[autopilot] dropping unparseable found-job row: {}",
                    sanitize_reason(&e.to_string())
                ),
            }
        }
        Ok(out)
    }

    /// Persist the found jobs of every autopilot in `map`, writing only rows
    /// whose content changed and trimming rows past each list's end. Autopilots
    /// not in `map` are left alone (see the module doc).
    fn sync(&mut self, map: &HashMap<String, Autopilot>) -> AppResult<()> {
        let mut changed: Vec<((String, i64), u64)> = Vec::new();
        let tx = self.conn.transaction()?;
        {
            let mut upsert = tx.prepare_cached(
                "INSERT INTO found_jobs (autopilot_id, position, json) VALUES (?1, ?2, ?3)
                 ON CONFLICT (autopilot_id, position) DO UPDATE SET json = excluded.json",
            )?;
            let mut trim = tx.prepare_cached(
                "DELETE FROM found_jobs WHERE autopilot_id = ?1 AND position >= ?2",
            )?;
            for (id, ap) in map {
                for (i, job) in ap.found_jobs.iter().enumerate() {
                    let json =
                        serde_json::to_string(job).map_err(|e| AppError::Storage(e.to_string()))?;
                    let key = (id.clone(), i as i64);
                    let hash = row_hash(&json);
                    if self.written.get(&key) != Some(&hash) {
                        upsert.execute(params![id, key.1, json])?;
                        changed.push((key, hash));
                    }
                }
                trim.execute(params![id, ap.found_jobs.len() as i64])?;
            }
        }
        tx.commit()?;
        #[cfg(test)]
        {
            self.rows_written += changed.len();
        }
        // Only after the commit: the in-memory picture must never run ahead of disk.
        self.written.retain(|(id, position), _| {
            map.get(id)
                .is_none_or(|ap| (*position as usize) < ap.found_jobs.len())
        });
        self.written.extend(changed);
        Ok(())
    }

    fn delete_autopilot(&mut self, id: &str) -> AppResult<()> {
        self.conn.execute(
            "DELETE FROM found_jobs WHERE autopilot_id = ?1",
            params![id],
        )?;
        self.written.retain(|(wid, _), _| wid != id);
        Ok(())
    }

    fn clear(&mut self) -> AppResult<()> {
        self.conn.execute("DELETE FROM found_jobs", [])?;
        self.written.clear();
        Ok(())
    }
}

impl AutopilotStore {
    /// Fill in each autopilot's found jobs from SQLite. Returns true when the
    /// JSON still carried found jobs (a legacy file), meaning the caller must
    /// persist once so they move into the table.
    pub(super) fn hydrate_found_jobs(&self, map: &mut HashMap<String, Autopilot>) -> bool {
        let Some(db) = &self.found_jobs_db else {
            return false; // no database: found jobs simply stay in the JSON
        };
        let mut rows = match db.lock().load_all() {
            Ok(rows) => rows,
            Err(e) => {
                log::error!(
                    "[autopilot] could not read found jobs: {}",
                    sanitize_reason(&e.to_string())
                );
                HashMap::new()
            }
        };
        let mut legacy = false;
        for (id, ap) in map.iter_mut() {
            if ap.found_jobs.is_empty() {
                ap.found_jobs = rows.remove(id).unwrap_or_default();
            } else {
                legacy = true;
            }
        }
        if legacy {
            let pre = self.data_file.with_extension("json.pre-sqlite");
            if pre.symlink_metadata().is_err() {
                if let Err(e) = std::fs::copy(&self.data_file, &pre) {
                    log::warn!(
                        "[autopilot] could not keep a pre-migration copy: {}",
                        sanitize_reason(&e.to_string())
                    );
                }
            }
        }
        legacy
    }

    /// Persist found jobs to SQLite. `true` means they are safely stored there
    /// and the JSON may omit them; `false` (no database, or the write failed)
    /// means the JSON must keep carrying them, so nothing is lost and the next
    /// load migrates them again.
    pub(super) fn persist_found_jobs(&self, map: &HashMap<String, Autopilot>) -> bool {
        let Some(db) = &self.found_jobs_db else {
            return false;
        };
        match db.lock().sync(map) {
            Ok(()) => true,
            Err(e) => {
                log::error!(
                    "[autopilot] could not store found jobs, keeping them in autopilots.json: {}",
                    sanitize_reason(&e.to_string())
                );
                false
            }
        }
    }

    /// Delete stored found jobs: one autopilot's, or every autopilot's (`None`).
    pub(super) fn forget_found_jobs(&self, id: Option<&str>) {
        let Some(db) = &self.found_jobs_db else {
            return;
        };
        let mut db = db.lock();
        let result = match id {
            Some(id) => db.delete_autopilot(id),
            None => db.clear(),
        };
        if let Err(e) = result {
            log::error!(
                "[autopilot] could not delete found jobs: {}",
                sanitize_reason(&e.to_string())
            );
        }
    }
}

/// Open the found-jobs database for a store, or `None` (logged) if it can't be:
/// the store then keeps found jobs inside `autopilots.json` as before.
pub(super) fn open_for_store(data_dir: &Path) -> Option<Mutex<FoundJobsDb>> {
    match FoundJobsDb::open(data_dir) {
        Ok(db) => Some(Mutex::new(db)),
        Err(e) => {
            log::error!(
                "[autopilot] could not open {DB_FILE}, keeping found jobs in autopilots.json: {}",
                sanitize_reason(&e.to_string())
            );
            None
        }
    }
}
