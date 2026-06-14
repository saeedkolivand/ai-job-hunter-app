use parking_lot::Mutex;
/// Job preferences store (SQLite-backed).
/// Stores user's job search preferences: location, tech stack, seniority, salary, remote.
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
    pub remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seniority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salary_min: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salary_max: Option<i64>,
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
    const MIGRATIONS: &'static [Migration] = &[Migration {
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
    }];

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
            "SELECT location, remote, seniority, salary_min, salary_max, tech_stack
             FROM job_preferences WHERE id = 1",
            [],
            |row| {
                let tech_stack_json: Option<String> = row.get(5)?;
                let tech_stack = tech_stack_json.and_then(|s| serde_json::from_str(&s).ok());
                Ok(JobPreferences {
                    location: row.get(0)?,
                    remote: row.get(1)?,
                    seniority: row.get(2)?,
                    salary_min: row.get(3)?,
                    salary_max: row.get(4)?,
                    tech_stack,
                })
            },
        )
        .unwrap_or(JobPreferences {
            location: None,
            remote: None,
            seniority: None,
            salary_min: None,
            salary_max: None,
            tech_stack: None,
        })
    }

    /// Reset all job preferences to empty (factory reset).
    pub fn clear(&self) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE job_preferences SET location = NULL, remote = NULL, seniority = NULL,
                 salary_min = NULL, salary_max = NULL, tech_stack = NULL WHERE id = 1",
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
             SET location = ?1, remote = ?2, seniority = ?3, 
                 salary_min = ?4, salary_max = ?5, tech_stack = ?6
             WHERE id = 1",
            params![
                prefs.location,
                prefs.remote,
                prefs.seniority,
                prefs.salary_min,
                prefs.salary_max,
                tech_stack_json,
            ],
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
