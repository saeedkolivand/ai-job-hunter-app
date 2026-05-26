use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

const TTL_SECS: i64 = 7 * 24 * 3600;

pub struct CompanyBriefCache {
    conn: Mutex<Connection>,
}

impl CompanyBriefCache {
    pub fn open(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join("company_briefs.db");
        let conn = Connection::open(&path).map_err(|e| format!("cache open: {e}"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS company_briefs (
                company    TEXT NOT NULL COLLATE NOCASE,
                brief      TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (company)
            );",
        )
        .map_err(|e| format!("cache init: {e}"))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Returns a cached brief if one exists and is younger than 7 days.
    pub fn get(&self, company: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        let cutoff = now_secs() - TTL_SECS;
        conn.query_row(
            "SELECT brief FROM company_briefs WHERE company = ?1 AND created_at > ?2",
            params![company, cutoff],
            |row| row.get(0),
        )
        .ok()
    }

    pub fn set(&self, company: &str, brief: &str) {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO company_briefs (company, brief, created_at) VALUES (?1, ?2, ?3)",
            params![company, brief, now_secs()],
        );
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
