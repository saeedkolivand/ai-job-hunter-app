//! The `ai_generations` row <-> [`AiGenerationRecord`] mapping and the statement
//! shared by every path that inserts a whole record.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). Persistence still lives entirely inside
//! [`super::AiGenerationStore`], on the SAME connection.

use rusqlite::{params, Connection};

use super::AiGenerationRecord;
use crate::db::{ts_from_db, ts_to_db};

/// The 23-column projection every record read selects, in the order
/// [`row_to_record`] decodes it — so the column list lives in one place.
pub(super) const SELECT_COLS: &str =
    "SELECT id, created_at, candidate_name, job_title, company_name,
            resume_language, job_ad_language, target_language, mismatch,
            top_requirements, mode, resume_text, cover_letter_text, job_ad,
            job_url, board, application_answers, company_brief, application_id,
            interview_questions, email_subject, email_body, quality_report
     FROM ai_generations";

/// Repair pre-#955 mojibake in `resume_text`/`cover_letter_text` on every
/// write path (`insert`, `update`, and `import`'s bulk-restore loop), not
/// just the one-time migration — so restoring an old backup bundle (which
/// still carries the corruption; `serde_json` round-trips an embedded NUL
/// intact) can't re-inject it into an already-migrated store. See
/// `extraction::pdf::repair_utf16_mojibake` and the
/// `repair_pre_pdf_text_string_mojibake` migration (see `migrations`). The gate inside
/// returns `Cow::Borrowed` on clean input, so this costs one `contains('\0')`
/// scan per write on the common (already-clean) case.
pub(super) fn repaired_generation_texts<'a>(
    resume_text: &'a str,
    cover_letter_text: &'a str,
) -> (std::borrow::Cow<'a, str>, std::borrow::Cow<'a, str>) {
    (
        crate::extraction::pdf::repair_utf16_mojibake(resume_text),
        crate::extraction::pdf::repair_utf16_mojibake(cover_letter_text),
    )
}

/// Bind and run the 23-column `INSERT` for one whole record — the single copy of the
/// statement, shared by [`super::AiGenerationStore::insert`] and the bulk-restore loop in
/// `import` (which binds its own transaction and cannot go through `insert`, whose
/// connection lock it would deadlock on). The caller supplies `quality_report`
/// because the two paths differ there: `insert` stores the record's own value, while
/// `import` first runs the untrusted bundle's value through `sanitize_quality_report`.
///
/// Both paths repair pre-#955 mojibake in the texts (`repaired_generation_texts`) and
/// map the error themselves, so each keeps its own `AppError` variant.
pub(super) fn insert_row(
    conn: &Connection,
    rec: &AiGenerationRecord,
    quality_report: &str,
) -> rusqlite::Result<usize> {
    let top_req_json = serde_json::to_string(&rec.top_requirements).unwrap_or_default();
    let answers_json = serde_json::to_string(&rec.application_answers).unwrap_or_default();
    let interview_questions_json =
        serde_json::to_string(&rec.interview_questions).unwrap_or_default();
    let (resume_text, cover_letter_text) =
        repaired_generation_texts(&rec.resume_text, &rec.cover_letter_text);
    conn.execute(
            "INSERT INTO ai_generations
             (id, created_at, candidate_name, job_title, company_name,
              resume_language, job_ad_language, target_language, mismatch,
              top_requirements, mode, resume_text, cover_letter_text, job_ad,
              job_url, board, application_answers, company_brief, application_id,
              interview_questions, email_subject, email_body, quality_report)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)",
            params![
                rec.id,
                ts_to_db(rec.created_at),
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
                quality_report,
            ],
        )
}

/// Map a DB row (the full 23-column projection) to a record. Shared by `list`
/// and `find_by_job_url` so the column order lives in one place.
pub(super) fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<AiGenerationRecord> {
    let top_req_json: String = row.get(9)?;
    let answers_json: String = row.get(16)?;
    let interview_questions_json: String = row.get(19)?;
    Ok(AiGenerationRecord {
        id: row.get(0)?,
        created_at: ts_from_db(row.get::<_, i64>(1)?),
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
        application_id: row.get(18)?,
        interview_questions: serde_json::from_str(&interview_questions_json).unwrap_or_default(),
        email_subject: row.get(20)?,
        email_body: row.get(21)?,
        quality_report: row.get(22)?,
    })
}
