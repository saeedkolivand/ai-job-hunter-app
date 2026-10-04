//! Schema migrations for `ai_generations.db`.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). Index in this slice = `PRAGMA user_version`
//! (1-based, see [`crate::db::run_migrations`]), so entries are **append-only**:
//! never reorder, never delete, never edit an already-shipped body.

use rusqlite::params;

use crate::db::Migration;

pub(super) const MIGRATIONS: &[Migration] = &[
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
        // A snapshot of every affected row is taken FIRST, in the same
        // transaction, before anything is rewritten — an irreversible
        // in-place edit gets a safety net despite the repair being
        // shape-specific and therefore low-risk (see
        // `repair_utf16_mojibake`). No other derived state in THIS store
        // is keyed on `resume_text`/`cover_letter_text` — they are the
        // final generated content shown to the user, not an input to any
        // cache here (ATS/match scoring reads the SOURCE résumé from
        // `documents.db`, never this store) — so, unlike the sibling
        // `documents` migration, there is nothing else to invalidate.
        //
        // A per-row UPDATE failure PROPAGATES (`?`), aborting and rolling
        // back the whole migration rather than being logged and
        // skipped — see the identical reasoning on the sibling
        // `documents::DocumentStore::MIGRATIONS` entry of the same name:
        // on SQLite's "fatal" error classes (SQLITE_FULL and friends),
        // SQLite may silently roll back the WHOLE enclosing transaction,
        // and a swallowed error would let the unconditional `PRAGMA
        // user_version = N` bump that follows commit anyway — durably
        // marking this migration "done" with the row never repaired and
        // no future retry. Not fatal to startup either way: `lib.rs`'s
        // setup hook treats a failed `AiGenerationStore::open()` as
        // non-fatal.
        name: "repair_pre_pdf_text_string_mojibake",
        up: |conn| {
            // Safety net: snapshot the pre-repair value of every row this
            // migration is about to touch, in this same transaction.
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS ai_generations_pre_mojibake_repair AS
                     SELECT id, resume_text, cover_letter_text FROM ai_generations
                     WHERE instr(cast(resume_text as blob), x'00') > 0
                        OR instr(cast(cover_letter_text as blob), x'00') > 0;",
            )?;

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
                let mut out = Vec::new();
                for r in mapped {
                    match r {
                        Ok(row) => out.push(row),
                        // Never silently drop a row this migration was
                        // supposed to reach — see the identical note on
                        // the sibling `documents` migration.
                        Err(e) => tracing::warn!(
                            "[db] repair_pre_pdf_text_string_mojibake: skipping a row \
                                 that failed to map: {e}"
                        ),
                    }
                }
                out
            };
            for (id, resume_text, cover_letter_text) in rows {
                let resume_repaired = crate::extraction::pdf::repair_utf16_mojibake(&resume_text);
                let cover_repaired =
                    crate::extraction::pdf::repair_utf16_mojibake(&cover_letter_text);
                conn.execute(
                        "UPDATE ai_generations SET resume_text = ?1, cover_letter_text = ?2 WHERE id = ?3",
                        params![resume_repaired.as_ref(), cover_repaired.as_ref(), id],
                    )?;
            }
            Ok(())
        },
    },
    // Additive: the deterministic content-quality report — see the field
    // doc on `AiGenerationRecord::quality_report`. Old rows default to ''
    // (unparseable by `merge_quality_report`, i.e. "no report"), never
    // touched again unless a future save carries one. Default is `''`, not
    // `'{}'`: this migration is still unreleased on this branch, so the
    // simpler default costs nothing — no shipped build has ever seen it.
    Migration {
        name: "add_quality_report",
        up: |conn| {
            conn.execute_batch(
                "ALTER TABLE ai_generations ADD COLUMN quality_report TEXT NOT NULL DEFAULT '';",
            )
        },
    },
];
