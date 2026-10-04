//! Persistence for the contact profile: a single-row SQLite settings table.

use std::path::PathBuf;

use parking_lot::Mutex;
use rusqlite::Connection;

use super::ContactProfile;
use crate::data_store::DataStore;
use crate::db::{run_migrations, Migration};
use crate::error::AppResult;

pub struct ContactProfileStore {
    conn: Mutex<Connection>,
}

impl ContactProfileStore {
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_contact_profile",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS contact_profile (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    data TEXT
                );",
            )?;
            conn.execute("INSERT OR IGNORE INTO contact_profile (id) VALUES (1)", [])?;
            Ok(())
        },
    }];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("contact_profile.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn get(&self) -> ContactProfile {
        self.try_get().unwrap_or_default()
    }

    /// The fallible half of [`Self::get`] (agent-cli review, P-r1-AC-R4-F3,
    /// issue #1180): a query failure (locked/busy row) and a corrupt stored
    /// row both surface here as `Err` instead of silently degrading to
    /// `ContactProfile::default()`. `get()` keeps the old degrade-to-default
    /// behaviour for its many read-only callers; a write path that would
    /// otherwise treat "couldn't read" as "nothing stored" (the
    /// `contact_profile_set` photo-restore in `extension_bridge/agent_call.rs`)
    /// must use this instead and refuse rather than proceed on a guess.
    pub fn try_get(&self) -> AppResult<ContactProfile> {
        let conn = self.conn.lock();
        let json: Option<String> = conn
            .query_row("SELECT data FROM contact_profile WHERE id = 1", [], |row| {
                row.get(0)
            })
            .map_err(|e| e.to_string())?;
        match json {
            Some(s) => Ok(serde_json::from_str(&s).map_err(|e| e.to_string())?),
            None => Ok(ContactProfile::default()),
        }
    }

    pub fn set(&self, profile: &ContactProfile) -> AppResult<()> {
        let json = serde_json::to_string(profile).map_err(|e| e.to_string())?;
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE contact_profile SET data = ?1 WHERE id = 1",
            rusqlite::params![json],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Reset the contact profile to empty (factory reset).
    pub fn clear(&self) -> AppResult<()> {
        self.set(&ContactProfile::default())
    }
}

impl DataStore for ContactProfileStore {
    fn key(&self) -> &'static str {
        "contactProfile"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::to_value(self.get()).unwrap_or_else(|_| serde_json::json!({}))
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        if data.is_null() {
            return Ok(0);
        }
        let profile: ContactProfile =
            serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
        self.set(&profile)?;
        Ok(1)
    }
}

#[cfg(test)]
mod tests;
