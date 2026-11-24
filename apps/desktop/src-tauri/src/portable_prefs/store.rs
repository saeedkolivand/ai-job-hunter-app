//! Persistence for the portable preferences: a single-row SQLite table, the
//! same shape as the contact profile store.

use std::path::PathBuf;

use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{
    is_portable, merge, parse_wire, to_wire, value_ok, Entry, Keys, SCHEMA_VERSION, SECTION,
};
use crate::data_store::{DataStore, SyncRecord, SyncSection};
use crate::db::{run_migrations, Migration};
use crate::error::{AppError, AppResult};

/// What the row holds. `migrated` is local bookkeeping (the one-time import
/// from the renderer's storage ran) and never leaves the device.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Row {
    #[serde(default)]
    migrated: bool,
    #[serde(default)]
    keys: Keys,
}

pub struct PortablePrefsStore {
    conn: Mutex<Connection>,
}

impl PortablePrefsStore {
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_portable_prefs",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS portable_prefs (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    data TEXT
                );",
            )?;
            conn.execute("INSERT OR IGNORE INTO portable_prefs (id) VALUES (1)", [])?;
            Ok(())
        },
    }];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let mut conn = crate::db::open(&data_dir.join("portable_prefs.db"))?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn load(conn: &Connection) -> AppResult<Row> {
        let json: Option<String> = conn
            .query_row("SELECT data FROM portable_prefs WHERE id = 1", [], |r| {
                r.get(0)
            })
            .map_err(|e| AppError::Storage(e.to_string()))?;
        match json {
            Some(s) => Ok(serde_json::from_str(&s)?),
            None => Ok(Row::default()),
        }
    }

    /// Read-modify-write under one lock; the row is saved only when `f` says
    /// it changed something.
    fn update(&self, f: impl FnOnce(&mut Row) -> AppResult<bool>) -> AppResult<()> {
        let conn = self.conn.lock();
        let mut row = Self::load(&conn)?;
        if f(&mut row)? {
            conn.execute(
                "UPDATE portable_prefs SET data = ?1 WHERE id = 1",
                rusqlite::params![serde_json::to_string(&row)?],
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
        }
        Ok(())
    }

    /// The current value of every stored key, and whether the one-time
    /// migration from the renderer already ran.
    pub fn values(&self) -> AppResult<(Map<String, Value>, bool)> {
        let row = Self::load(&self.conn.lock())?;
        let values = row.keys.into_iter().map(|(k, e)| (k, e.value)).collect();
        Ok((values, row.migrated))
    }

    /// Record the user's changes. Rejects the whole call on an unknown key or
    /// an unacceptable value; a value equal to the stored one is not a change.
    pub fn set(&self, updates: &Map<String, Value>, now: u64) -> AppResult<()> {
        check(updates)?;
        self.update(|row| {
            let mut changed = false;
            for (key, value) in updates {
                let prev = row.keys.get(key);
                if prev.is_some_and(|e| e.value == *value) {
                    continue;
                }
                let updated_at = prev.map_or(now, |e| now.max(e.updated_at + 1));
                row.keys.insert(
                    key.clone(),
                    Entry {
                        value: value.clone(),
                        updated_at,
                    },
                );
                changed = true;
            }
            Ok(changed)
        })
    }

    /// One-time copy of the renderer's existing values. Runs once; only fills
    /// keys the store does not hold (so a received value is never overwritten),
    /// stamped `0` so any later edit on any device wins over them.
    pub fn migrate(&self, values: &Map<String, Value>) -> AppResult<()> {
        check(values)?;
        self.update(|row| {
            if row.migrated {
                return Ok(false);
            }
            for (key, value) in values {
                row.keys.entry(key.clone()).or_insert_with(|| Entry {
                    value: value.clone(),
                    updated_at: 0,
                });
            }
            row.migrated = true;
            Ok(true)
        })
    }

    /// Factory reset: forget every value; the migration stays done so the
    /// renderer's old storage is not copied back in.
    pub fn clear(&self) -> AppResult<()> {
        self.update(|row| {
            row.keys.clear();
            row.migrated = true;
            Ok(true)
        })
    }

    /// The fallible export: a read failure must not look like "nothing stored".
    pub fn try_export(&self) -> AppResult<Value> {
        Ok(to_wire(&Self::load(&self.conn.lock())?.keys))
    }

    /// Apply a received payload: allowlist-filtered, merged per key.
    fn apply_remote(&self, payload: &Value) -> AppResult<()> {
        let remote = parse_wire(payload);
        self.update(|row| Ok(merge(&mut row.keys, remote)))
    }
}

/// Reject a write that names a key outside the allowlist or carries a value
/// that is not an acceptable setting.
fn check(updates: &Map<String, Value>) -> AppResult<()> {
    for (key, value) in updates {
        if !is_portable(key) {
            return Err(AppError::Validation(format!(
                "'{key}' is not a portable preference"
            )));
        }
        if !value_ok(value) {
            return Err(AppError::Validation(format!(
                "the value of '{key}' is empty or too large"
            )));
        }
    }
    Ok(())
}

impl DataStore for PortablePrefsStore {
    fn key(&self) -> &'static str {
        SECTION
    }

    fn export(&self) -> Value {
        self.try_export().unwrap_or_else(|_| to_wire(&Keys::new()))
    }

    /// Restore: REPLACE the keys with the allowlisted ones of `data`.
    fn import(&self, data: &Value) -> AppResult<usize> {
        if data.is_null() {
            return Ok(0);
        }
        let keys = parse_wire(data);
        let n = keys.len();
        self.update(|row| {
            row.keys = keys;
            row.migrated = true;
            Ok(true)
        })?;
        Ok(n)
    }
}

impl SyncSection for PortablePrefsStore {
    fn section(&self) -> &'static str {
        SECTION
    }

    fn schema_version(&self) -> u32 {
        SCHEMA_VERSION
    }

    fn sync_records(&self) -> AppResult<Vec<SyncRecord>> {
        Ok(vec![SyncRecord {
            id: SECTION.to_string(),
            payload: self.try_export()?,
        }])
    }

    fn upsert_record(&self, _id: &str, payload: &Value) -> AppResult<()> {
        self.apply_remote(payload)
    }

    fn delete_record(&self, _id: &str) -> AppResult<()> {
        Ok(())
    }
}
