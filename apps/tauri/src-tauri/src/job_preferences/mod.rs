/// Job preferences store (SQLite-backed).
/// Stores user's job search preferences: location, tech stack, seniority, salary, remote.
use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

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
    pub fn open(data_dir: &PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let path = data_dir.join("job_preferences.db");
        let conn = Connection::open(&path).map_err(|e| e.to_string())?;
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
        )
        .map_err(|e| e.to_string())?;
        
        // Ensure the single row exists
        conn.execute(
            "INSERT OR IGNORE INTO job_preferences (id) VALUES (1)",
            [],
        )
        .map_err(|e| e.to_string())?;
        
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn get(&self) -> JobPreferences {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT location, remote, seniority, salary_min, salary_max, tech_stack
             FROM job_preferences WHERE id = 1",
            [],
            |row| {
                let tech_stack_json: Option<String> = row.get(5)?;
                let tech_stack = tech_stack_json
                    .and_then(|s| serde_json::from_str(&s).ok());
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
        .unwrap_or_else(|_| JobPreferences {
            location: None,
            remote: None,
            seniority: None,
            salary_min: None,
            salary_max: None,
            tech_stack: None,
        })
    }

    pub fn set(&self, prefs: &JobPreferences) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let tech_stack_json = prefs.tech_stack.as_ref()
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

#[cfg(test)]
mod test;
