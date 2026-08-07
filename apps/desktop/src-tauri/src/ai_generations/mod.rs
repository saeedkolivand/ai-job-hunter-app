use parking_lot::Mutex;
use std::path::PathBuf;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::data_store::DataStore;
use crate::db::{now_ms, run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::{AppError, AppResult};

/// One answered application question, stored on the application record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplicationAnswer {
    pub id: String,
    pub question: String,
    pub answer: String,
}

/// One AI-suggested question the candidate can ASK the interviewer (distinct from
/// the answered application questions above). Stored on the application record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InterviewQuestion {
    pub id: String,
    pub question: String,
    /// Why this question lands well / what it signals to the interviewer.
    pub why: String,
    /// Target interviewer — `recruiter` | `hiringManager` | `team` | `leadership`
    /// | `general` (open-typed; an unknown value is treated as `general`).
    pub audience: String,
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
    /// AI-suggested "questions to ask the interviewer" — the second assistant,
    /// distinct from `application_answers` above. Stored as JSON.
    #[serde(rename = "interviewQuestions", default)]
    pub interview_questions: Vec<InterviewQuestion>,
    /// The apply-by-email draft generated in the Application detail tab: the
    /// subject line and the body, kept as two plain columns (they are edited and
    /// copied independently by the UI, so a JSON blob would only add parsing).
    #[serde(rename = "emailSubject", default)]
    pub email_subject: String,
    #[serde(rename = "emailBody", default)]
    pub email_body: String,
    /// Parent Application FK (NULL when unlinked). Carried through export/import so
    /// a backup round-trip preserves the application↔generation link; otherwise
    /// `remove_for_application`/`detach_application` stop matching restored rows.
    #[serde(
        rename = "applicationId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub application_id: Option<String>,
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
        // ADR 0001: demote the generation to a child Document of an Application.
        // This adds the parent FK (NULL until the backfill in
        // `applications::ApplicationStore::open` links each row). Nullable so old
        // rows and any future doc-less inserts are valid.
        Migration {
            name: "add_application_id",
            up: |conn| {
                conn.execute_batch(
                    "ALTER TABLE ai_generations ADD COLUMN application_id TEXT;
                     CREATE INDEX IF NOT EXISTS idx_ai_generations_application_id
                         ON ai_generations(application_id);",
                )
            },
        },
        // Additive: AI-suggested "questions to ask the interviewer" — the second
        // assistant alongside application answers. Old rows default to empty.
        Migration {
            name: "add_interview_questions",
            up: |conn| {
                conn.execute_batch(
                    "ALTER TABLE ai_generations ADD COLUMN interview_questions TEXT NOT NULL DEFAULT '[]';",
                )
            },
        },
        // #816 follow-up: enforce ONE aggregate row per job. Two concurrent
        // `save_application` calls for the same job could each miss
        // `find_by_job_url` (the lock is released between the read and the insert)
        // and both insert, forking the per-job aggregate. The write path now
        // recovers from the resulting constraint error by merging instead.
        //
        // PARTIAL (`WHERE job_url != ''`): an unusable raw url normalizes to '' and
        // is deliberately stored as a separate, unlinked manual generation — the
        // constraint must NOT collapse those into one.
        //
        // Forward-safe against a DB that ALREADY forked: collapse each duplicate
        // non-empty job_url to a single row BEFORE creating the unique index, or
        // the CREATE would fail. Keep the row linked to an Application if any, else
        // the newest — the runtime merge already prefers newest content, so this
        // drops only the rarer partial fork.
        Migration {
            name: "unique_job_url_aggregate",
            up: |conn| {
                conn.execute_batch(
                    "DELETE FROM ai_generations
                       WHERE job_url != ''
                         AND id NOT IN (
                           SELECT id FROM (
                             SELECT id, ROW_NUMBER() OVER (
                                      PARTITION BY job_url
                                      ORDER BY (application_id IS NOT NULL) DESC,
                                               created_at DESC, rowid DESC
                                    ) AS rn
                             FROM ai_generations
                             WHERE job_url != ''
                           ) WHERE rn = 1
                         );
                     CREATE UNIQUE INDEX IF NOT EXISTS idx_ai_generations_job_url_unique
                         ON ai_generations(job_url) WHERE job_url != '';",
                )
            },
        },
        // Additive: the apply-by-email draft (subject + body) produced by the
        // Application detail "Apply by email" tab, which until now lived only in
        // React state and was lost on a tab switch. Two plain columns rather than
        // one JSON field — the UI edits/copies subject and body independently.
        // Old rows default to '' (an empty draft, exactly what they had).
        // Purely forward-safe: `ADD COLUMN` with a NOT NULL default never touches
        // existing data, and dropping the feature just leaves two unread columns.
        Migration {
            name: "add_email_draft",
            up: |conn| {
                conn.execute_batch(
                    "ALTER TABLE ai_generations ADD COLUMN email_subject TEXT NOT NULL DEFAULT '';
                     ALTER TABLE ai_generations ADD COLUMN email_body    TEXT NOT NULL DEFAULT '';",
                )
            },
        },
        Migration {
            // One-shot repair for rows written before PR #955 fixed
            // `pdf_text_string` (see `extraction::pdf::pdf_text_string` and
            // `repair_utf16_mojibake`, which documents the exact corruption
            // shape). A generation built from a pre-fix PDF import can carry
            // the same mojibake into `resume_text` and/or `cover_letter_text`
            // (whichever text was assembled from the corrupted extraction).
            //
            // Only rows with an embedded NUL in either column are candidates
            // — SQLite's `length()` stops at the first NUL in NUL-bearing
            // TEXT, so `instr(cast(<col> as blob), x'00')` is used instead.
            // The repair runs in RUST, not SQL — `replace()` on NUL-bearing
            // TEXT is not dependable in SQLite, and `char(0)` cannot produce
            // a NUL to match against in the first place.
            //
            // A single row's UPDATE failing is logged and skipped rather than
            // propagated: this is a best-effort repair of already-corrupt
            // data, not new data, so aborting the whole migration would roll
            // back every other row's fix AND leave `user_version` stuck
            // retrying the identical failure on every future startup —
            // bricking the app over a cosmetic mojibake defect.
            name: "repair_pre_pdf_text_string_mojibake",
            up: |conn| {
                let rows: Vec<(String, String, String)> = {
                    let mut stmt = conn.prepare(
                        "SELECT id, resume_text, cover_letter_text FROM ai_generations
                         WHERE instr(cast(resume_text as blob), x'00') > 0
                            OR instr(cast(cover_letter_text as blob), x'00') > 0",
                    )?;
                    let mapped = stmt.query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?;
                    mapped.filter_map(|r| r.ok()).collect()
                };
                for (id, resume_text, cover_letter_text) in rows {
                    let resume_repaired = crate::extraction::pdf::repair_utf16_mojibake(&resume_text);
                    let cover_repaired =
                        crate::extraction::pdf::repair_utf16_mojibake(&cover_letter_text);
                    if let Err(e) = conn.execute(
                        "UPDATE ai_generations SET resume_text = ?1, cover_letter_text = ?2 WHERE id = ?3",
                        params![resume_repaired.as_ref(), cover_repaired.as_ref(), id],
                    ) {
                        tracing::warn!(
                            "[db] repair_pre_pdf_text_string_mojibake: failed to repair ai_generation {id}: {e}"
                        );
                    }
                }
                Ok(())
            },
        },
    ];

    pub fn open(data_dir: &PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("ai_generations.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
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
                    job_url, board, application_answers, company_brief, application_id,
                    interview_questions, email_subject, email_body
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
                    job_url, board, application_answers, company_brief, application_id,
                    interview_questions, email_subject, email_body
             FROM ai_generations WHERE job_url = ?1 ORDER BY created_at DESC LIMIT 1",
        )
        .ok()
        .and_then(|mut stmt| stmt.query_row(params![job_url], row_to_record).ok())
    }

    pub fn insert(&self, rec: &AiGenerationRecord) -> AppResult<()> {
        let top_req_json = serde_json::to_string(&rec.top_requirements).unwrap_or_default();
        let answers_json = serde_json::to_string(&rec.application_answers).unwrap_or_default();
        let interview_questions_json =
            serde_json::to_string(&rec.interview_questions).unwrap_or_default();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO ai_generations
             (id, created_at, candidate_name, job_title, company_name,
              resume_language, job_ad_language, target_language, mismatch,
              top_requirements, mode, resume_text, cover_letter_text, job_ad,
              job_url, board, application_answers, company_brief, application_id,
              interview_questions, email_subject, email_body)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)",
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
                rec.resume_text,
                rec.cover_letter_text,
                rec.job_ad,
                rec.job_url,
                rec.board,
                answers_json,
                rec.company_brief,
                rec.application_id,
                interview_questions_json,
                rec.email_subject,
                rec.email_body,
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Overwrite an existing row by id (used by the per-job merge-upsert).
    fn update(&self, rec: &AiGenerationRecord) -> AppResult<()> {
        let top_req_json = serde_json::to_string(&rec.top_requirements).unwrap_or_default();
        let answers_json = serde_json::to_string(&rec.application_answers).unwrap_or_default();
        let interview_questions_json =
            serde_json::to_string(&rec.interview_questions).unwrap_or_default();
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE ai_generations SET
              candidate_name = ?2, job_title = ?3, company_name = ?4,
              resume_language = ?5, job_ad_language = ?6, target_language = ?7,
              mismatch = ?8, top_requirements = ?9, mode = ?10, resume_text = ?11,
              cover_letter_text = ?12, job_ad = ?13, job_url = ?14, board = ?15,
              application_answers = ?16, company_brief = ?17, application_id = ?18,
              interview_questions = ?19, email_subject = ?20, email_body = ?21
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
                rec.application_id,
                interview_questions_json,
                rec.email_subject,
                rec.email_body,
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
        // Build "?,?,…" placeholders — never interpolate user-supplied ids.
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("DELETE FROM ai_generations WHERE id IN ({placeholders})");
        let conn = self.conn.lock();
        let deleted = conn.execute(&sql, rusqlite::params_from_iter(ids.iter()))?;
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
}

/// Map a DB row (the full 22-column projection) to a record. Shared by `list`
/// and `find_by_job_url` so the column order lives in one place.
fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<AiGenerationRecord> {
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
    // `mismatch` is derived from the resume/jobAd language pair, so it is only
    // meaningful when the incoming save actually carries that pair. An
    // answers-only / interview-only save leaves the language fields blank and its
    // `mismatch` defaults to `false` — not a real verdict.
    let incoming_has_languages =
        !incoming.resume_language.trim().is_empty() && !incoming.job_ad_language.trim().is_empty();
    // Subject and body are ONE draft: the email surface always writes both
    // together, so they merge together. Picking them independently let a
    // regeneration whose output broke the `Subject:` line contract (parsed
    // subject = "") keep the PREVIOUS subject glued onto the NEW body — a
    // mismatched email the user never wrote. Either field carrying content means
    // "this save owns the draft"; neither means the save is about something else
    // (résumé, answers, interview questions) and the stored draft is kept.
    let (email_subject, email_body) =
        if incoming.email_subject.trim().is_empty() && incoming.email_body.trim().is_empty() {
            (existing.email_subject, existing.email_body)
        } else {
            (incoming.email_subject, incoming.email_body)
        };
    AiGenerationRecord {
        id: existing.id,
        created_at: existing.created_at,
        candidate_name: pick(incoming.candidate_name, existing.candidate_name),
        job_title: pick(incoming.job_title, existing.job_title),
        company_name: pick(incoming.company_name, existing.company_name),
        resume_language: pick(incoming.resume_language, existing.resume_language),
        job_ad_language: pick(incoming.job_ad_language, existing.job_ad_language),
        target_language: pick(incoming.target_language, existing.target_language),
        // Follow the incoming verdict ONLY when this save computed the language
        // pair — mirroring how the language fields themselves merge (`pick`), so a
        // corrected regeneration (`mismatch=false`) clears a stale warning while a
        // content-less save can't clobber a real prior verdict. The old
        // `incoming || existing` made a `true` permanent.
        mismatch: if incoming_has_languages {
            incoming.mismatch
        } else {
            existing.mismatch
        },
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
        interview_questions: if incoming.interview_questions.is_empty() {
            existing.interview_questions
        } else {
            incoming.interview_questions
        },
        // Merged as one atomic draft — see the binding above.
        email_subject,
        email_body,
        application_id: incoming.application_id.or(existing.application_id),
    }
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
        // Deserialize EVERY record before mutating the store, so a malformed row
        // aborts the import without having cleared the table.
        let records: Vec<AiGenerationRecord> = items
            .iter()
            .map(|item| serde_json::from_value(item.clone()).map_err(AppError::from))
            .collect::<AppResult<_>>()?;

        // `clear_all` + the repopulation loop run in ONE transaction: either the
        // whole replace lands or the old rows are left untouched on any failure.
        // `Connection::transaction` needs `&mut Connection`, so take the lock and
        // call on `&mut *guard`.
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        tx.execute("DELETE FROM ai_generations", [])?;
        for rec in &records {
            let top_req_json = serde_json::to_string(&rec.top_requirements).unwrap_or_default();
            let answers_json = serde_json::to_string(&rec.application_answers).unwrap_or_default();
            let interview_questions_json =
                serde_json::to_string(&rec.interview_questions).unwrap_or_default();
            tx.execute(
                "INSERT INTO ai_generations
                 (id, created_at, candidate_name, job_title, company_name,
                  resume_language, job_ad_language, target_language, mismatch,
                  top_requirements, mode, resume_text, cover_letter_text, job_ad,
                  job_url, board, application_answers, company_brief, application_id,
                  interview_questions, email_subject, email_body)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)",
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
                    rec.resume_text,
                    rec.cover_letter_text,
                    rec.job_ad,
                    rec.job_url,
                    rec.board,
                    answers_json,
                    rec.company_brief,
                    rec.application_id,
                    interview_questions_json,
                    rec.email_subject,
                    rec.email_body,
                ],
            )?;
        }
        tx.commit()?;
        Ok(records.len())
    }
}

#[cfg(test)]
mod test;
