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
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_open_store() {
        let temp_dir = TempDir::new().unwrap();
        let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();
        let prefs = store.get();
        assert!(prefs.location.is_none());
        assert!(prefs.remote.is_none());
    }

    #[test]
    fn test_get_default() {
        let temp_dir = TempDir::new().unwrap();
        let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();
        let prefs = store.get();
        
        assert_eq!(prefs.location, None);
        assert_eq!(prefs.remote, None);
        assert_eq!(prefs.seniority, None);
        assert_eq!(prefs.salary_min, None);
        assert_eq!(prefs.salary_max, None);
        assert_eq!(prefs.tech_stack, None);
    }

    #[test]
    fn test_set_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();
        
        let prefs = JobPreferences {
            location: Some("Berlin".to_string()),
            remote: Some("hybrid".to_string()),
            seniority: Some("senior".to_string()),
            salary_min: Some(80000),
            salary_max: Some(120000),
            tech_stack: Some(vec![
                TechStackItem {
                    name: "Rust".to_string(),
                    category: "language".to_string(),
                },
                TechStackItem {
                    name: "React".to_string(),
                    category: "frontend".to_string(),
                },
            ]),
        };
        
        store.set(&prefs).unwrap();
        let retrieved = store.get();
        
        assert_eq!(retrieved.location, Some("Berlin".to_string()));
        assert_eq!(retrieved.remote, Some("hybrid".to_string()));
        assert_eq!(retrieved.seniority, Some("senior".to_string()));
        assert_eq!(retrieved.salary_min, Some(80000));
        assert_eq!(retrieved.salary_max, Some(120000));
        assert_eq!(retrieved.tech_stack.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_tech_stack_serialization() {
        let temp_dir = TempDir::new().unwrap();
        let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();
        
        let prefs = JobPreferences {
            location: None,
            remote: None,
            seniority: None,
            salary_min: None,
            salary_max: None,
            tech_stack: Some(vec![
                TechStackItem {
                    name: "TypeScript".to_string(),
                    category: "language".to_string(),
                },
            ]),
        };
        
        store.set(&prefs).unwrap();
        let retrieved = store.get();
        
        assert_eq!(retrieved.tech_stack.unwrap()[0].name, "TypeScript");
    }

    #[test]
    fn test_partial_update() {
        let temp_dir = TempDir::new().unwrap();
        let store = JobPreferencesStore::open(&temp_dir.path().to_path_buf()).unwrap();
        
        // Set initial preferences
        let prefs1 = JobPreferences {
            location: Some("Berlin".to_string()),
            remote: Some("remote".to_string()),
            seniority: Some("senior".to_string()),
            salary_min: Some(80000),
            salary_max: Some(120000),
            tech_stack: None,
        };
        store.set(&prefs1).unwrap();
        
        // Update only some fields
        let prefs2 = JobPreferences {
            location: Some("Munich".to_string()),
            remote: Some("hybrid".to_string()),
            seniority: None,
            salary_min: None,
            salary_max: None,
            tech_stack: None,
        };
        store.set(&prefs2).unwrap();
        
        let retrieved = store.get();
        assert_eq!(retrieved.location, Some("Munich".to_string()));
        assert_eq!(retrieved.remote, Some("hybrid".to_string()));
        // Unchanged fields should be None since we set them to None
        assert_eq!(retrieved.seniority, None);
    }
}
