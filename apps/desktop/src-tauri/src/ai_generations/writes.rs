//! The write side of the `ai_generations` store: the whole-record insert/update,
//! the per-job aggregate save, and the targeted column edits and deletes.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). Persistence still lives entirely inside
//! [`super::AiGenerationStore`], on the SAME connection.

use rusqlite::params;

use super::quality_report::sanitize_quality_report;
use super::record::merge_application;
use super::rows::{insert_row, repaired_generation_texts};
use super::{AiGenerationRecord, AiGenerationStore};
use crate::error::AppResult;

impl AiGenerationStore {
    pub fn insert(&self, rec: &AiGenerationRecord) -> AppResult<()> {
        let conn = self.conn.lock();
        insert_row(&conn, rec, &rec.quality_report).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Overwrite an existing row by id (used by the per-job merge-upsert).
    fn update(&self, rec: &AiGenerationRecord) -> AppResult<()> {
        let top_req_json = serde_json::to_string(&rec.top_requirements).unwrap_or_default();
        let answers_json = serde_json::to_string(&rec.application_answers).unwrap_or_default();
        let interview_questions_json =
            serde_json::to_string(&rec.interview_questions).unwrap_or_default();
        let (resume_text, cover_letter_text) =
            repaired_generation_texts(&rec.resume_text, &rec.cover_letter_text);
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE ai_generations SET
              candidate_name = ?2, job_title = ?3, company_name = ?4,
              resume_language = ?5, job_ad_language = ?6, target_language = ?7,
              mismatch = ?8, top_requirements = ?9, mode = ?10, resume_text = ?11,
              cover_letter_text = ?12, job_ad = ?13, job_url = ?14, board = ?15,
              application_answers = ?16, company_brief = ?17, application_id = ?18,
              interview_questions = ?19, email_subject = ?20, email_body = ?21,
              quality_report = ?22
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
                resume_text.as_ref(),
                cover_letter_text.as_ref(),
                rec.job_ad,
                rec.job_url,
                rec.board,
                answers_json,
                rec.company_brief,
                rec.application_id,
                interview_questions_json,
                rec.email_subject,
                rec.email_body,
                rec.quality_report,
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
    pub fn save_application(&self, mut incoming: AiGenerationRecord) -> AppResult<String> {
        // Key the aggregate on the NORMALIZED url — the identity `ApplicationStore`
        // already dedupes on. Matching the raw url split the same job into two
        // "one-per-job" rows as soon as it was reached under different tracking
        // params, which is the norm on query-id boards like Indeed.
        let normalized = crate::applications::normalize_job_url(&incoming.job_url);
        let raw = std::mem::replace(&mut incoming.job_url, normalized);
        let mut found = self.find_by_job_url(&incoming.job_url);
        // A row written before this normalization still carries its raw url; match
        // it too, and the write below migrates it onto the normalized key.
        if found.is_none() && !incoming.job_url.is_empty() && raw != incoming.job_url {
            found = self.find_by_job_url(&raw);
        }
        if let Some(existing) = found {
            let merged = merge_application(existing, incoming);
            let id = merged.id.clone();
            self.update(&merged)?;
            return Ok(id);
        }
        let id = incoming.id.clone();
        if let Err(insert_err) = self.insert(&incoming) {
            // A concurrent writer inserted this job_url between our
            // `find_by_job_url` above and here; the UNIQUE(job_url) index rejects
            // our duplicate. Recover by merging into the row that now exists, so
            // exactly one aggregate per job survives instead of a fork. (Empty
            // job_url is exempt from the partial index, so an error there is never
            // this race — surface it. A non-race insert failure also finds no row
            // and surfaces below.)
            if incoming.job_url.is_empty() {
                return Err(insert_err);
            }
            let Some(existing) = self.find_by_job_url(&incoming.job_url) else {
                return Err(insert_err);
            };
            let merged = merge_application(existing, incoming);
            let merged_id = merged.id.clone();
            self.update(&merged)?;
            return Ok(merged_id);
        }
        Ok(id)
    }

    /// Overwrite ONE row's `quality_report`, selected by `id`.
    ///
    /// The report-only sibling of [`update_texts`](Self::update_texts), and
    /// deliberately a direct overwrite rather than a merge: the caller has just
    /// read this exact blob, edited one decision inside it, and is writing it
    /// back. Routing it through `save_application`'s
    /// per-top-level-key merge would re-union it with itself, and a caller that
    /// wanted to CLEAR a slot could never do so.
    ///
    /// Clamped like every other write path into this column, so a hand-built
    /// blob cannot exceed [`QUALITY_REPORT_MAX_BYTES`] here either.
    pub fn update_quality_report(&self, id: &str, quality_report: String) -> AppResult<()> {
        let report = sanitize_quality_report(quality_report, "update_quality_report");
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE ai_generations SET quality_report = ?2 WHERE id = ?1",
            params![id, report],
        )?;
        if changed == 0 {
            return Err(format!("generation not found: {id}").into());
        }
        Ok(())
    }

    pub fn remove(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM ai_generations WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Delete all generations whose id is in `ids` in a single transaction.
    /// Returns the number of rows actually deleted.
    /// Empty input is a no-op that returns `Ok(0)` without touching the DB.
    pub fn remove_many(&self, ids: &[String]) -> AppResult<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        // Build "?,?,…" placeholders — never interpolate user-supplied ids —
        // and CHUNK them: an unbounded selection blows SQLite's host-parameter
        // limit and the whole delete fails to prepare.
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut deleted = 0usize;
        for chunk in ids.chunks(crate::db::MAX_SQL_PARAMS) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!("DELETE FROM ai_generations WHERE id IN ({placeholders})");
            deleted += tx.execute(&sql, rusqlite::params_from_iter(chunk.iter()))?;
        }
        tx.commit()?;
        Ok(deleted)
    }

    /// Delete every generation linked to `application_id` (the child Documents of
    /// one Application). Used by `applications_delete` when the user chose "delete
    /// everything". Idempotent: an Application with no documents deletes 0 rows.
    pub fn remove_for_application(&self, application_id: &str) -> AppResult<usize> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM ai_generations WHERE application_id = ?1",
            params![application_id],
        )?;
        Ok(deleted)
    }

    /// Detach every generation from `application_id` (set the FK back to NULL) so
    /// the documents survive as orphaned generations after the parent Application
    /// is deleted. Used by `applications_delete` when the user chose "remove
    /// tracking only (keep documents)".
    pub fn detach_application(&self, application_id: &str) -> AppResult<usize> {
        let conn = self.conn.lock();
        let updated = conn.execute(
            "UPDATE ai_generations SET application_id = NULL WHERE application_id = ?1",
            params![application_id],
        )?;
        Ok(updated)
    }

    /// Edit the résumé and/or cover-letter text of an existing row, selected by
    /// `id`. Unlike the per-job merge-upsert ([`save_application`]) this is a
    /// direct overwrite of exactly the provided fields, so a user editing a saved
    /// generation can blank out or fully replace text the merge would have kept.
    /// Each `None` field is left untouched; passing both `None` is a no-op.
    /// (The schema has no `updated_at` column, so there is no timestamp to bump.)
    pub fn update_texts(
        &self,
        id: &str,
        resume_text: Option<String>,
        cover_letter_text: Option<String>,
    ) -> AppResult<()> {
        let conn = self.conn.lock();
        let changed = match (resume_text, cover_letter_text) {
            (Some(resume), Some(cover)) => conn.execute(
                "UPDATE ai_generations SET resume_text = ?2, cover_letter_text = ?3 WHERE id = ?1",
                params![id, resume, cover],
            )?,
            (Some(resume), None) => conn.execute(
                "UPDATE ai_generations SET resume_text = ?2 WHERE id = ?1",
                params![id, resume],
            )?,
            (None, Some(cover)) => conn.execute(
                "UPDATE ai_generations SET cover_letter_text = ?2 WHERE id = ?1",
                params![id, cover],
            )?,
            // Both fields absent: no UPDATE is issued, so there is no
            // rows-changed count to check — an explicit no-op success.
            (None, None) => return Ok(()),
        };
        if changed == 0 {
            return Err(format!("generation not found: {id}").into());
        }
        Ok(())
    }

    /// Replace a row's résumé text AND its quality report in ONE transaction.
    ///
    /// Not a convenience wrapper over [`update_texts`](Self::update_texts) +
    /// [`update_quality_report`](Self::update_quality_report): the pipeline's
    /// merge rule is that **any save writing `resume_text` carries a fresh
    /// `quality_report`**, and two statements have a window between them where
    /// the row holds the NEW document beside the OLD document's report — a
    /// report the panel would render as this text's verdict. A crash, a lock
    /// error, or a clamp rejection on the second statement makes that window
    /// permanent. One transaction is the only way the rule is a guarantee
    /// rather than an ordering convention.
    ///
    /// `quality_report` is clamped exactly as `update_quality_report` clamps it
    /// — same write path, same guard.
    pub fn update_text_and_report(
        &self,
        id: &str,
        resume_text: String,
        quality_report: String,
    ) -> AppResult<()> {
        let report = sanitize_quality_report(quality_report, "update_text_and_report");
        // `Connection::transaction` needs `&mut Connection`, so take the lock
        // mutably and call on the guard (same shape as `import`).
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        let changed = tx.execute(
            "UPDATE ai_generations SET resume_text = ?2, quality_report = ?3 WHERE id = ?1",
            params![id, resume_text, report],
        )?;
        if changed == 0 {
            return Err(format!("generation not found: {id}").into());
        }
        tx.commit()?;
        Ok(())
    }
}
