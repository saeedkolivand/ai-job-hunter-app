//! Cross-board dedup verdict store (ADR-029 §a).
//!
//! The ONLY durable state of the clustering feature: the user's "not a
//! duplicate" verdicts, stored as unordered `canonical_job_key` PAIRS in
//! `<dataDir>/dedup.db`. Cluster membership itself is never persisted — it is
//! recomputed at every ingest by the pure `scraping::cluster` module — so this
//! store holds only the split decisions that must survive a re-scrape.
//!
//! Invariant: every stored pair has `key_a < key_b` (see [`DedupStore::pair`]),
//! so an unordered pair has exactly one row and a membership lookup is a single
//! `HashSet` hit regardless of the order the two keys are presented in.
//!
//! Wired like every other L1 store: opened via `db::open` + a transactional
//! migration (ADR-022), backed up/restored via [`crate::data_store::DataStore`]
//! (`dedupTombstones` section), and wiped on factory reset via `Resettable`
//! (registered in `commands::privacy`).

use std::collections::HashSet;
use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::data_store::DataStore;
use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::{AppError, AppResult};

/// One persisted "not a duplicate" verdict. `key_a < key_b` by construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TombstoneRow {
    pub key_a: String,
    pub key_b: String,
    pub created_at: u64,
}

pub struct DedupStore {
    conn: Mutex<Connection>,
}

impl DedupStore {
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_dedup_tombstones",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS dedup_tombstones (
                    key_a      TEXT NOT NULL,
                    key_b      TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    PRIMARY KEY (key_a, key_b)
                );",
            )
        },
    }];

    pub fn open(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("dedup.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Canonicalize an unordered key pair to the stored `(key_a, key_b)` shape
    /// with `key_a < key_b`. A pair of a key with itself is meaningless and is
    /// returned as-is (callers filter self-pairs before insert).
    pub fn pair(a: &str, b: &str) -> (String, String) {
        if a <= b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        }
    }

    /// Insert every pair (idempotently) in ONE transaction. Each pair is
    /// canonicalized via [`Self::pair`] and self-pairs (`a == b`) are skipped;
    /// `INSERT OR IGNORE` makes a re-insert a no-op (the existing `created_at`
    /// is preserved). Errors map to [`AppError::Storage`].
    pub fn insert_pairs(&self, pairs: &[(String, String)]) -> AppResult<()> {
        let created_at = ts_to_db(now_ms());
        let mut guard = self.conn.lock();
        let tx = guard
            .transaction()
            .map_err(|e| AppError::Storage(e.to_string()))?;
        for (a, b) in pairs {
            if a == b {
                continue; // a key is never a duplicate of itself
            }
            let (key_a, key_b) = Self::pair(a, b);
            tx.execute(
                "INSERT OR IGNORE INTO dedup_tombstones (key_a, key_b, created_at)
                 VALUES (?1, ?2, ?3)",
                params![key_a, key_b, created_at],
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
        }
        tx.commit().map_err(|e| AppError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Every stored verdict as an unordered pair set (`key_a < key_b`), for the
    /// clustering pass's tombstone veto. A read failure yields an empty set —
    /// clustering degrades to "no splits", never blocks ingest.
    pub fn all_pairs(&self) -> HashSet<(String, String)> {
        let conn = self.conn.lock();
        conn.prepare("SELECT key_a, key_b FROM dedup_tombstones")
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .ok()
                .map(|rows| rows.filter_map(Result::ok).collect())
            })
            .unwrap_or_default()
    }

    /// Wipe every verdict (factory reset).
    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM dedup_tombstones", []).ok();
    }

    /// Snapshot all rows (deterministic order) for export.
    fn rows(&self) -> Vec<TombstoneRow> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT key_a, key_b, created_at FROM dedup_tombstones ORDER BY key_a, key_b",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                Ok(TombstoneRow {
                    key_a: row.get(0)?,
                    key_b: row.get(1)?,
                    created_at: ts_from_db(row.get::<_, i64>(2)?),
                })
            })
            .ok()
            .map(|rows| rows.filter_map(Result::ok).collect())
        })
        .unwrap_or_default()
    }
}

impl DataStore for DedupStore {
    fn key(&self) -> &'static str {
        "dedupTombstones"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::json!(self.rows())
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        let items = data
            .as_array()
            .ok_or_else(|| AppError::Validation("dedupTombstones: expected an array".to_string()))?;
        // Deserialize EVERY row before mutating, so a malformed row aborts the
        // import without having cleared the table (mirrors the other stores).
        let rows: Vec<TombstoneRow> = items
            .iter()
            .map(|item| serde_json::from_value(item.clone()).map_err(AppError::from))
            .collect::<AppResult<_>>()?;

        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        tx.execute("DELETE FROM dedup_tombstones", [])?;
        for row in &rows {
            // Re-canonicalize on import so a hand-edited/legacy bundle can never
            // violate the `key_a < key_b` invariant; skip self-pairs.
            if row.key_a == row.key_b {
                continue;
            }
            let (key_a, key_b) = Self::pair(&row.key_a, &row.key_b);
            tx.execute(
                "INSERT OR IGNORE INTO dedup_tombstones (key_a, key_b, created_at)
                 VALUES (?1, ?2, ?3)",
                params![key_a, key_b, ts_to_db(row.created_at)],
            )?;
        }
        tx.commit()?;
        Ok(rows.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn open() -> (TempDir, DedupStore) {
        let dir = TempDir::new().unwrap();
        let store = DedupStore::open(dir.path()).unwrap();
        (dir, store)
    }

    #[test]
    fn pair_orders_keys_ascending() {
        assert_eq!(DedupStore::pair("b", "a"), ("a".into(), "b".into()));
        assert_eq!(DedupStore::pair("a", "b"), ("a".into(), "b".into()));
    }

    #[test]
    fn insert_is_order_independent_and_idempotent() {
        let (_dir, store) = open();
        store.insert_pairs(&[("z".into(), "a".into())]).unwrap();
        // Re-insert the SAME pair in the opposite order — must not duplicate.
        store.insert_pairs(&[("a".into(), "z".into())]).unwrap();
        let pairs = store.all_pairs();
        assert_eq!(pairs.len(), 1);
        assert!(pairs.contains(&("a".into(), "z".into())));
    }

    #[test]
    fn self_pairs_are_ignored() {
        let (_dir, store) = open();
        store.insert_pairs(&[("a".into(), "a".into())]).unwrap();
        assert!(store.all_pairs().is_empty(), "a key can't be a dup of itself");
    }

    #[test]
    fn export_import_round_trips() {
        let (_dir, store) = open();
        store
            .insert_pairs(&[("k1".into(), "k2".into()), ("k3".into(), "k2".into())])
            .unwrap();
        let bundle = store.export();

        let (_dir2, store2) = open();
        let restored = store2.import(&bundle).unwrap();
        assert_eq!(restored, 2);
        assert_eq!(store2.all_pairs(), store.all_pairs());
        // Invariant preserved through the round-trip.
        assert!(store2.all_pairs().contains(&("k2".into(), "k3".into())));
    }

    #[test]
    fn import_with_a_malformed_row_errors_and_preserves_existing_rows() {
        let (_dir, store) = open();
        store.insert_pairs(&[("keep".into(), "me".into())]).unwrap();

        // One well-formed row + one malformed (missing `keyB`). The store
        // deserializes EVERY row before mutating, so the malformed row must fail
        // the whole import BEFORE any DELETE/insert runs.
        let bundle = serde_json::json!([
            { "keyA": "a", "keyB": "b", "createdAt": 1 },
            { "keyA": "x", "createdAt": 2 }
        ]);
        let result = store.import(&bundle);
        assert!(result.is_err(), "a malformed row must fail the whole import");

        // The pre-existing verdict survives untouched, and the well-formed row
        // from the failed bundle was NOT partially inserted.
        let pairs = store.all_pairs();
        assert!(
            pairs.contains(&("keep".into(), "me".into())),
            "existing rows must survive a failed import (deserialize-all-before-mutate)"
        );
        assert!(
            !pairs.contains(&("a".into(), "b".into())),
            "no partial insert from the failed import"
        );
        assert_eq!(pairs.len(), 1, "the table is exactly the pre-import state");
    }

    #[test]
    fn import_replaces_existing_rows() {
        let (_dir, store) = open();
        store.insert_pairs(&[("old".into(), "row".into())]).unwrap();
        // A bundle with a different single pair replaces, not merges.
        let bundle = serde_json::json!([
            { "keyA": "a", "keyB": "b", "createdAt": 123 }
        ]);
        store.import(&bundle).unwrap();
        let pairs = store.all_pairs();
        assert_eq!(pairs.len(), 1);
        assert!(pairs.contains(&("a".into(), "b".into())));
    }

    #[test]
    fn clear_all_empties_the_store() {
        let (_dir, store) = open();
        store.insert_pairs(&[("a".into(), "b".into())]).unwrap();
        assert!(!store.all_pairs().is_empty());
        store.clear_all();
        assert!(store.all_pairs().is_empty());
    }

    #[test]
    fn reopening_the_same_db_is_migration_idempotent() {
        let dir = TempDir::new().unwrap();
        {
            let store = DedupStore::open(dir.path()).unwrap();
            store.insert_pairs(&[("a".into(), "b".into())]).unwrap();
        }
        // Second open re-runs run_migrations (no-op) and keeps the data.
        let store = DedupStore::open(dir.path()).unwrap();
        assert!(store.all_pairs().contains(&("a".into(), "b".into())));
    }
}
