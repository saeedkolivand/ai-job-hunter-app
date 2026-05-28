use std::path::PathBuf;
use parking_lot::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{run_migrations, Migration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiGenerationRecord {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    // GenerationMeta fields
    #[serde(rename = "candidateName")]
    pub candidate_name: String,
    #[serde(rename = "jobTitle")]
    pub job_title: String,
    #[serde(rename = "companyName")]
    pub company_name: String,
    #[serde(rename = "resumeLanguage")]
    pub resume_language: String,
    #[serde(rename = "jobAdLanguage")]
    pub job_ad_language: String,
    #[serde(rename = "targetLanguage")]
    pub target_language: String,
    pub mismatch: bool,
    #[serde(rename = "topRequirements")]
    pub top_requirements: Vec<String>, // stored as JSON
    // Generation settings
    pub mode: String,
    // Content
    #[serde(rename = "resumeText")]
    pub resume_text: String,
    #[serde(rename = "coverLetterText")]
    pub cover_letter_text: String,
    #[serde(rename = "jobAd")]
    pub job_ad: String,
}

pub struct AiGenerationStore {
    conn: Mutex<Connection>,
}

impl AiGenerationStore {
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_ai_generations",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS ai_generations (
                    id                TEXT PRIMARY KEY,
                    created_at        INTEGER NOT NULL,
                    candidate_name    TEXT NOT NULL DEFAULT '',
                    job_title         TEXT NOT NULL DEFAULT '',
                    company_name      TEXT NOT NULL DEFAULT '',
                    resume_language   TEXT NOT NULL DEFAULT 'en',
                    job_ad_language   TEXT NOT NULL DEFAULT 'en',
                    target_language   TEXT NOT NULL DEFAULT 'en',
                    mismatch          INTEGER NOT NULL DEFAULT 0,
                    top_requirements  TEXT NOT NULL DEFAULT '[]',
                    mode              TEXT NOT NULL DEFAULT 'ats',
                    resume_text       TEXT NOT NULL DEFAULT '',
                    cover_letter_text TEXT NOT NULL DEFAULT '',
                    job_ad            TEXT NOT NULL DEFAULT ''
                );",
            )
        },
    }];

    pub fn open(data_dir: &PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let path = data_dir.join("ai_generations.db");
        let conn = Connection::open(&path).map_err(|e| e.to_string())?;
        run_migrations(&conn, Self::MIGRATIONS)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM ai_generations", []).ok();
    }

    pub fn list(&self) -> Vec<AiGenerationRecord> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT id, created_at, candidate_name, job_title, company_name,
                    resume_language, job_ad_language, target_language, mismatch,
                    top_requirements, mode, resume_text, cover_letter_text, job_ad
             FROM ai_generations ORDER BY created_at DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| {
                let top_req_json: String = row.get(9)?;
                Ok(AiGenerationRecord {
                    id: row.get(0)?,
                    created_at: row.get::<_, i64>(1)? as u64,
                    candidate_name: row.get(2)?,
                    job_title: row.get(3)?,
                    company_name: row.get(4)?,
                    resume_language: row.get(5)?,
                    job_ad_language: row.get(6)?,
                    target_language: row.get(7)?,
                    mismatch: row.get::<_, i64>(8)? != 0,
                    top_requirements: serde_json::from_str(&top_req_json).unwrap_or_default(),
                    mode: row.get(10)?,
                    resume_text: row.get(11)?,
                    cover_letter_text: row.get(12)?,
                    job_ad: row.get(13)?,
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    pub fn insert(&self, rec: &AiGenerationRecord) -> Result<(), String> {
        let top_req_json = serde_json::to_string(&rec.top_requirements).unwrap_or_default();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO ai_generations
             (id, created_at, candidate_name, job_title, company_name,
              resume_language, job_ad_language, target_language, mismatch,
              top_requirements, mode, resume_text, cover_letter_text, job_ad)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                rec.id,
                rec.created_at as i64,
                rec.candidate_name,
                rec.job_title,
                rec.company_name,
                rec.resume_language,
                rec.job_ad_language,
                rec.target_language,
                rec.mismatch as i64,
                top_req_json,
                rec.mode,
                rec.resume_text,
                rec.cover_letter_text,
                rec.job_ad,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM ai_generations WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn make_generation_id() -> String {
    format!("gen-{}-{}", now_ms(), &Uuid::new_v4().to_string()[..8])
}
