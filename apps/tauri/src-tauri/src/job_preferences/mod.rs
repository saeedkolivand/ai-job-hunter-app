use parking_lot::Mutex;
/// Job preferences store (SQLite-backed).
/// Stores user's job search preferences: location and tech stack.
use std::path::PathBuf;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::data_store::DataStore;
use crate::db::{run_migrations, Migration};
use crate::error::AppResult;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech_stack: Option<Vec<TechStackItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TechStackItem {
    pub name: String,
    pub category: String,
}

// ── JobPreferencesStore ─────────────────────────────────────────────────────────

pub struct JobPreferencesStore {
    conn: Mutex<Connection>,
}

impl JobPreferencesStore {
    const MIGRATIONS: &'static [Migration] = &[
        Migration {
            name: "create_job_preferences",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS job_preferences (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    location TEXT,
                    remote TEXT,
                    seniority TEXT,
                    salary_min INTEGER,
                    salary_max INTEGER,
                    tech_stack TEXT
                );",
                )?;
                // Ensure the single settings row exists.
                conn.execute("INSERT OR IGNORE INTO job_preferences (id) VALUES (1)", [])?;
                Ok(())
            },
        },
        // Drop the dormant columns (remote, seniority, salary_min, salary_max) —
        // they were persisted but never written by any UI and only read by a dead
        // autopilot work-type seed. Use the SQLite-safe table-recreate (works on
        // every bundled SQLite regardless of `ALTER TABLE ... DROP COLUMN` support)
        // so location + tech_stack survive the column removal.
        Migration {
            name: "drop_unused_job_preferences_columns",
            up: |conn| {
                conn.execute_batch(
                    "CREATE TABLE job_preferences_new (
                        id INTEGER PRIMARY KEY CHECK (id = 1),
                        location TEXT,
                        tech_stack TEXT
                    );
                    INSERT INTO job_preferences_new (id, location, tech_stack)
                        SELECT id, location, tech_stack FROM job_preferences;
                    DROP TABLE job_preferences;
                    ALTER TABLE job_preferences_new RENAME TO job_preferences;
                    INSERT OR IGNORE INTO job_preferences (id) VALUES (1);",
                )?;
                Ok(())
            },
        },
    ];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("job_preferences.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn get(&self) -> JobPreferences {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT location, tech_stack
             FROM job_preferences WHERE id = 1",
            [],
            |row| {
                let tech_stack_json: Option<String> = row.get(1)?;
                let tech_stack = tech_stack_json.and_then(|s| serde_json::from_str(&s).ok());
                Ok(JobPreferences {
                    location: row.get(0)?,
                    tech_stack,
                })
            },
        )
        .unwrap_or(JobPreferences {
            location: None,
            tech_stack: None,
        })
    }

    /// Reset all job preferences to empty (factory reset).
    pub fn clear(&self) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE job_preferences SET location = NULL, tech_stack = NULL WHERE id = 1",
            [],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set(&self, prefs: &JobPreferences) -> AppResult<()> {
        let conn = self.conn.lock();
        let tech_stack_json = prefs
            .tech_stack
            .as_ref()
            .and_then(|ts| serde_json::to_string(ts).ok());

        conn.execute(
            "UPDATE job_preferences
             SET location = ?1, tech_stack = ?2
             WHERE id = 1",
            params![prefs.location, tech_stack_json],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl DataStore for JobPreferencesStore {
    fn key(&self) -> &'static str {
        "jobPreferences"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::to_value(self.get()).unwrap_or_else(|_| serde_json::json!({}))
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        // Single settings row; treat null/missing as "nothing to restore".
        if data.is_null() {
            return Ok(0);
        }
        let prefs: JobPreferences =
            serde_json::from_value(data.clone()).map_err(|e| e.to_string())?;
        self.set(&prefs)?;
        Ok(1)
    }
}

#[cfg(test)]
mod test;
