//! The `ai_generations` store: one row per posting (the "per-job aggregate") holding a
//! résumé, a cover letter, answered questions, an email draft and the content-quality
//! report, as child documents of an [`crate::applications`] Application.
//!
//! This file holds the store handle, `open` and `clear_all`; every other slice of the
//! same store lives in a sibling module, split by responsibility (R8): `record` (the
//! types + the per-job merge), `quality_report` (the column's cap + merge), `migrations`
//! (schema), `rows` (row mapping + the shared whole-record `INSERT`), `reads`, `writes`
//! and `backup` (export / import).

use parking_lot::Mutex;
use std::path::PathBuf;

use rusqlite::Connection;

use crate::db::run_migrations;
use crate::error::AppResult;

mod backup;
mod migrations;
mod quality_report;
mod reads;
mod record;
mod rows;
#[cfg(test)]
mod tests;
mod writes;

pub(crate) use quality_report::sanitize_quality_report;
pub use record::{make_generation_id, AiGenerationRecord, ApplicationAnswer, InterviewQuestion};

pub struct AiGenerationStore {
    conn: Mutex<Connection>,
}

impl AiGenerationStore {
    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("ai_generations.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, migrations::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        // The `repair_pre_pdf_text_string_mojibake` migration snapshots every
        // affected row's pre-repair, still-corrupt résumé/cover-letter text
        // into `ai_generations_pre_mojibake_repair` as a safety net for its
        // in-place rewrite (see the identical rationale on the sibling
        // `documents::DocumentStore::clear_all`). A full "erase my data"
        // reset must drop that snapshot too — `DROP`, not `DELETE`, since it
        // is a one-shot migration artifact `user_version` will not recreate.
        conn.execute_batch(
            "DELETE FROM ai_generations; DROP TABLE IF EXISTS ai_generations_pre_mojibake_repair;",
        )
        .ok();
    }
}
