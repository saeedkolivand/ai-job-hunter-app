//! The lookup-only repair that links orphaned `ai_generations` rows to their
//! Application on every boot.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). Like the legacy backfill in `migrations`, it
//! opens a SECOND connection onto the sibling `ai_generations.db`; it is the
//! lookup-only twin of that backfill and the two must be read together.

use std::path::Path;

use rusqlite::params;

use super::{normalize_job_url, ApplicationStore};
use crate::error::AppResult;

impl ApplicationStore {
    /// Forward-safe, idempotent, **lookup-only** backfill: link every orphaned
    /// `ai_generations` row (non-empty `job_url`, `application_id` still NULL)
    /// to its Application, keyed by normalized `job_url` — the SAME key
    /// [`Self::upsert_for_origin`] merges new saves on. Returns the number of
    /// rows actually linked, so a caller (and a test) can tell a real backfill
    /// from a no-op one.
    ///
    /// **Deliberately never creates an Application — the reason this is a
    /// SEPARATE function from [`Self::backfill_from_generations`], not a
    /// second pass through it.** That backfill predates ADR-0001 entirely: for
    /// those rows no Application ever existed anywhere, so creating one is the
    /// correct — the only possible — repair. The rows THIS backfill targets are
    /// different in kind: every staged-pipeline run is launched FROM an
    /// Application's own page, so the Application for an orphaned row almost
    /// always already exists, created independently of the generation save
    /// that failed to link to it (see `commands::resume_pipeline::mod::
    /// persist_document`'s own doc for how that save came to omit the FK). A
    /// row this lookup cannot match therefore means one of two things, and
    /// BOTH must leave `application_id` NULL rather than guess:
    ///
    /// * the user has since deleted that Application — creating one here would
    ///   RESURRECT it, which is exactly the defect a lookup-only backfill
    ///   avoids (an upsert-style backfill would recreate it silently, and the
    ///   user would never know their delete was undone);
    /// * the row is a genuinely unlinked generation with no Application to
    ///   find (an id-less/unsaved apply attempt, a stale/renamed url).
    ///
    /// A WRONG link is worse than a missing one: it would put someone's résumé
    /// under an unrelated Application and delete it along with THAT
    /// Application the next time the user deletes it. `Self::find_by_job_url`
    /// is the existing READ-ONLY primitive for exactly this — the same one
    /// `extension_bridge`'s `applied.check` handler uses because it must never
    /// fetch-or-write either.
    ///
    /// **Safe to re-run, and re-run on every boot on purpose** (same posture
    /// as `backfill_from_generations`): the `WHERE application_id IS NULL`
    /// selection is itself the idempotency guard — a row this backfill already
    /// linked is excluded from the next run's scan, and a row it could not
    /// match stays eligible, so a later boot picks it up the moment its
    /// Application exists (e.g. the user re-generates, re-creating the link
    /// through the ordinary save path).
    ///
    /// No new schema migration: `AiGenerationStore`'s own `add_application_id`
    /// migration already added the column and its index long before this
    /// backfill was written, and this is DATA repair, not a schema change — so
    /// there is nothing to append to either store's position-indexed
    /// `MIGRATIONS` array for it.
    pub(super) fn link_orphaned_generations(&self, data_dir: &Path) -> AppResult<usize> {
        let gen_path = data_dir.join("ai_generations.db");
        if !gen_path.exists() {
            return Ok(0); // fresh install — nothing to link
        }
        let gen_conn = crate::db::open(&gen_path)?;
        if !crate::db::column_exists(&gen_conn, "ai_generations", "application_id") {
            // AiGenerationStore has never run its own migrations on this
            // install yet — nothing to backfill against.
            return Ok(0);
        }

        let mut stmt = gen_conn.prepare(
            "SELECT id, job_url FROM ai_generations \
             WHERE job_url != '' AND (application_id IS NULL OR application_id = '')",
        )?;
        let orphans: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(Result::ok)
            .collect();
        drop(stmt);

        let mut linked = 0usize;
        for (gen_id, job_url) in orphans {
            let normalized = normalize_job_url(&job_url);
            // Read-only lookup — see this function's own doc for why an
            // upsert is never the right call here.
            let Some(app) = self.find_by_job_url(&normalized) else {
                continue; // no resolvable Application — stays NULL, never guessed
            };
            gen_conn.execute(
                "UPDATE ai_generations SET application_id = ?2 WHERE id = ?1",
                params![gen_id, app.id],
            )?;
            linked += 1;
        }
        Ok(linked)
    }
}
