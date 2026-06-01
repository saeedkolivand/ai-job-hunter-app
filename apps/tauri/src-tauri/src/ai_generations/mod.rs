use parking_lot::Mutex;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data_store::DataStore;
use crate::db::{run_migrations, Migration};
use crate::error::AppResult;

/// One answered application question, stored on the application record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplicationAnswer {
    pub id: String,
    pub question: String,
    pub answer: String,
}

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
    // Application link — makes the record the single "application" aggregate: the
    // job it targets and the board it came from. `job_url` is what derives a found
    // job's `applied` flag (a matching url means the user generated for it).
    #[serde(rename = "jobUrl", default)]
    pub job_url: String,
    #[serde(default)]
    pub board: String,
    // Application extras — answered questions and the company-research brief used,
    // so the record is the full auditable application aggregate. Stored as JSON.
    #[serde(rename = "applicationAnswers", default)]
    pub application_answers: Vec<ApplicationAnswer>,
    #[serde(rename = "companyBrief", default)]
    pub company_brief: String,
}

pub struct AiGenerationStore {
    conn: Mutex<Connection>,
}

impl AiGenerationStore {
    const MIGRATIONS: &'static [Migration] = &[
        Migration {
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
        },
        // Additive: link a generation to the job it targets (and its board), so
        // each row is the full "application" record. Old rows default to ''.
        Migration {
            name: "add_job_link",
            up: |conn| {
                conn.execute_batch(
                    "ALTER TABLE ai_generations ADD COLUMN job_url TEXT NOT NULL DEFAULT '';
                     ALTER TABLE ai_generations ADD COLUMN board   TEXT NOT NULL DEFAULT '';
                     CREATE INDEX IF NOT EXISTS idx_ai_generations_job_url
                         ON ai_generations(job_url);",
                )
            },
        },
        // Additive: the answered application questions and the company-research
        // brief used, completing the application aggregate. Old rows default empty.
        Migration {
            name: "add_application_answers",
            up: |conn| {
                conn.execute_batch(
                    "ALTER TABLE ai_generations ADD COLUMN application_answers TEXT NOT NULL DEFAULT '[]';
                     ALTER TABLE ai_generations ADD COLUMN company_brief       TEXT NOT NULL DEFAULT '';",
                )
            },
        },
    ];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let path = data_dir.join("ai_generations.db");
        let conn = Connection::open(&path).map_err(|e| e.to_string())?;
        run_migrations(&conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
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
                    top_requirements, mode, resume_text, cover_letter_text, job_ad,
                    job_url, board, application_answers, company_brief
             FROM ai_generations ORDER BY created_at DESC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], row_to_record)
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    /// Most-recent record linked to `job_url`, if any — the row a per-job save
    /// merges into so each job keeps one application aggregate.
    fn find_by_job_url(&self, job_url: &str) -> Option<AiGenerationRecord> {
        if job_url.is_empty() {
            return None;
        }
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT id, created_at, candidate_name, job_title, company_name,
                    resume_language, job_ad_language, target_language, mismatch,
                    top_requirements, mode, resume_text, cover_letter_text, job_ad,
                    job_url, board, application_answers, company_brief
             FROM ai_generations WHERE job_url = ?1 ORDER BY created_at DESC LIMIT 1",
        )
        .ok()
        .and_then(|mut stmt| stmt.query_row(params![job_url], row_to_record).ok())
    }

    pub fn insert(&self, rec: &AiGenerationRecord) -> AppResult<()> {
        let top_req_json = serde_json::to_string(&rec.top_requirements).unwrap_or_default();
        let answers_json = serde_json::to_string(&rec.application_answers).unwrap_or_default();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO ai_generations
             (id, created_at, candidate_name, job_title, company_name,
              resume_language, job_ad_language, target_language, mismatch,
              top_requirements, mode, resume_text, cover_letter_text, job_ad,
              job_url, board, application_answers, company_brief)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
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
                rec.job_url,
                rec.board,
                answers_json,
                rec.company_brief,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Overwrite an existing row by id (used by the per-job merge-upsert).
    fn update(&self, rec: &AiGenerationRecord) -> AppResult<()> {
        let top_req_json = serde_json::to_string(&rec.top_requirements).unwrap_or_default();
        let answers_json = serde_json::to_string(&rec.application_answers).unwrap_or_default();
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE ai_generations SET
              candidate_name = ?2, job_title = ?3, company_name = ?4,
              resume_language = ?5, job_ad_language = ?6, target_language = ?7,
              mismatch = ?8, top_requirements = ?9, mode = ?10, resume_text = ?11,
              cover_letter_text = ?12, job_ad = ?13, job_url = ?14, board = ?15,
              application_answers = ?16, company_brief = ?17
             WHERE id = ?1",
            params![
                rec.id,
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
                rec.job_url,
                rec.board,
                answers_json,
                rec.company_brief,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Save an application generation as a **per-job aggregate**: when it carries
    /// a `job_url`, merge into that job's existing row ([`merge_application`]) so
    /// résumé, cover letter, answers, and brief from separate user actions land on
    /// one record; otherwise insert a fresh row (manual generations with no link).
    /// Returns the id of the affected row.
    pub fn save_application(&self, incoming: AiGenerationRecord) -> AppResult<String> {
        if let Some(existing) = self.find_by_job_url(&incoming.job_url) {
            let merged = merge_application(existing, incoming);
            let id = merged.id.clone();
            self.update(&merged)?;
            return Ok(id);
        }
        let id = incoming.id.clone();
        self.insert(&incoming)?;
        Ok(id)
    }

    /// Distinct non-empty `job_url`s that have at least one saved generation —
    /// the set used to derive a found job's `applied` flag.
    pub fn applied_job_urls(&self) -> std::collections::HashSet<String> {
        let conn = self.conn.lock();
        conn.prepare("SELECT DISTINCT job_url FROM ai_generations WHERE job_url != ''")
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], |row| row.get::<_, String>(0))
                    .ok()
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default()
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM ai_generations WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Map a DB row (the full 18-column projection) to a record. Shared by `list`
/// and `find_by_job_url` so the column order lives in one place.
fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<AiGenerationRecord> {
    let top_req_json: String = row.get(9)?;
    let answers_json: String = row.get(16)?;
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
        job_url: row.get(14)?,
        board: row.get(15)?,
        application_answers: serde_json::from_str(&answers_json).unwrap_or_default(),
        company_brief: row.get(17)?,
    })
}

/// Merge an incoming per-job save into the existing application row: keep the
/// existing id + first-seen time, and take each incoming field only when it
/// carries content, so independent saves (résumé, cover, answers, brief) layer
/// onto one aggregate instead of clobbering each other. Pure — unit-tested.
fn merge_application(
    existing: AiGenerationRecord,
    incoming: AiGenerationRecord,
) -> AiGenerationRecord {
    let pick = |inc: String, ex: String| if inc.trim().is_empty() { ex } else { inc };
    AiGenerationRecord {
        id: existing.id,
        created_at: existing.created_at,
        candidate_name: pick(incoming.candidate_name, existing.candidate_name),
        job_title: pick(incoming.job_title, existing.job_title),
        company_name: pick(incoming.company_name, existing.company_name),
        resume_language: pick(incoming.resume_language, existing.resume_language),
        job_ad_language: pick(incoming.job_ad_language, existing.job_ad_language),
        target_language: pick(incoming.target_language, existing.target_language),
        mismatch: incoming.mismatch || existing.mismatch,
        top_requirements: if incoming.top_requirements.is_empty() {
            existing.top_requirements
        } else {
            incoming.top_requirements
        },
        mode: pick(incoming.mode, existing.mode),
        resume_text: pick(incoming.resume_text, existing.resume_text),
        cover_letter_text: pick(incoming.cover_letter_text, existing.cover_letter_text),
        job_ad: pick(incoming.job_ad, existing.job_ad),
        job_url: pick(incoming.job_url, existing.job_url),
        board: pick(incoming.board, existing.board),
        application_answers: if incoming.application_answers.is_empty() {
            existing.application_answers
        } else {
            incoming.application_answers
        },
        company_brief: pick(incoming.company_brief, existing.company_brief),
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

impl DataStore for AiGenerationStore {
    fn key(&self) -> &'static str {
        "aiGenerations"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::json!(self.list())
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        let items = data.as_array().ok_or("aiGenerations: expected an array")?;
        self.clear_all();
        let mut count = 0;
        for item in items {
            let record: AiGenerationRecord =
                serde_json::from_value(item.clone()).map_err(|e| e.to_string())?;
            self.insert(&record)?;
            count += 1;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod test;
