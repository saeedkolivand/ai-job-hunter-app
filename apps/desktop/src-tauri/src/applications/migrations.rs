//! Schema migrations for `applications.db`.
//!
//! Split out of [`super`] so the store body stays under the architecture LOC cap
//! (`tests/architecture.rs` R8) and so the append-only migration list has an
//! obvious home as it grows. Index in this slice = `PRAGMA user_version` (1-based,
//! see [`crate::db::run_migrations`]), so entries are **append-only**: never
//! reorder, never delete, never edit an already-shipped body.
//!
//! Every migration is additive and forward-safe — `ADD COLUMN` with a default, or
//! a guarded backfill `UPDATE`. Nothing here drops a column or a table.

use std::path::Path;

use rusqlite::{params, OptionalExtension};

use crate::ai_generations::ApplicationAnswer;
use crate::db::{now_ms, ts_from_db, ts_to_db, Migration};
use crate::error::AppResult;

use super::{normalize_job_url, ApplicationMeta, ApplicationStore, APPLICATIONS_FEATURE_EPOCH_MS};

/// The whitespace set SQLite's two-argument `TRIM(x, y)` must strip so the
/// `unify_application_contact` emptiness test agrees with Rust's `str::trim` —
/// see `super::contact` for the other half of that lockstep.
///
/// Bare `TRIM(x)` strips ONLY U+0020, whereas `str::trim` strips all Unicode
/// `White_Space`. So a `"\u{A0}"` (NBSP — endemic in text copied out of scraped
/// HTML) or a TAB read as NON-empty in SQL and as empty in Rust, and the same
/// row folded one way through the in-place migration and the other way through
/// a restored bundle.
///
/// Covers space, TAB, LF, CR and NBSP — everything a real contact field picks up
/// from HTML or a paste. Rust stays a strict SUPERSET for exotic separators
/// (U+2028, U+3000, …); that residual divergence is documented and accepted in
/// `super::contact` rather than chased with an unbounded `char()` list.
const SQL_TRIM_CHARS: &str = "' ' || char(9) || char(10) || char(13) || char(160)";

/// The ordered migration list for `applications.db`. Append only.
pub(super) const MIGRATIONS: &[Migration] = &[
    Migration {
        name: "create_applications",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS applications (
                    id              TEXT PRIMARY KEY,
                    status          TEXT NOT NULL DEFAULT 'saved',
                    applied_at      INTEGER,
                    created_at      INTEGER NOT NULL,
                    updated_at      INTEGER NOT NULL,
                    job_url         TEXT NOT NULL DEFAULT '',
                    board           TEXT NOT NULL DEFAULT '',
                    company         TEXT NOT NULL DEFAULT '',
                    title           TEXT NOT NULL DEFAULT '',
                    candidate       TEXT NOT NULL DEFAULT '',
                    answers         TEXT NOT NULL DEFAULT '[]',
                    brief           TEXT NOT NULL DEFAULT '',
                    notes           TEXT NOT NULL DEFAULT '',
                    next_action_at  INTEGER,
                    comp            TEXT NOT NULL DEFAULT '',
                    contact_name    TEXT NOT NULL DEFAULT '',
                    contact_email   TEXT NOT NULL DEFAULT ''
                );
                CREATE INDEX IF NOT EXISTS idx_applications_job_url
                    ON applications(job_url);",
            )
        },
    },
    Migration {
        name: "create_status_events",
        up: |conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS status_events (
                    application_id  TEXT NOT NULL,
                    from_status     TEXT NOT NULL DEFAULT '',
                    to_status       TEXT NOT NULL,
                    at              INTEGER NOT NULL,
                    note            TEXT NOT NULL DEFAULT ''
                );
                CREATE INDEX IF NOT EXISTS idx_status_events_app
                    ON status_events(application_id);",
            )
        },
    },
    Migration {
        name: "add_applications_job_description",
        up: |conn| {
            conn.execute_batch(
                "ALTER TABLE applications ADD COLUMN job_description TEXT NOT NULL DEFAULT '';",
            )
        },
    },
    Migration {
        name: "add_applications_job_summary",
        up: |conn| {
            conn.execute_batch(
                "ALTER TABLE applications ADD COLUMN job_summary TEXT NOT NULL DEFAULT ''",
            )
        },
    },
    Migration {
        name: "add_applications_recipient",
        up: |conn| {
            conn.execute_batch(
                "ALTER TABLE applications ADD COLUMN recipient_name TEXT NOT NULL DEFAULT '';
                 ALTER TABLE applications ADD COLUMN recipient_email TEXT NOT NULL DEFAULT '';",
            )
        },
    },
    Migration {
        name: "add_applications_salary",
        up: |conn| {
            // Nullable, NOT text-default: NULL means "unknown salary" (mirrors
            // applied_at/next_action_at), never 0 — a 0 would read as a real
            // (wrong) salary downstream.
            conn.execute_batch(
                "ALTER TABLE applications ADD COLUMN salary_min REAL;
                 ALTER TABLE applications ADD COLUMN salary_max REAL;
                 ALTER TABLE applications ADD COLUMN salary_currency TEXT;",
            )
        },
    },
    Migration {
        name: "unify_application_contact",
        up: |conn| {
            // Contact unification: `contact_name`/`contact_email` become THE
            // single primary contact per application; `recipient_name`/
            // `recipient_email` (the apply-by-email pair) are demoted to
            // deprecated aliases of it — see `super::Application`.
            //
            // **PAIR-ATOMIC.** The promotion moves name AND email together, and
            // only when the WHOLE canonical pair is empty. Promoting each column
            // independently would fuse two different people — person A's name
            // (already in `contact_name`) with person B's address (in
            // `recipient_email`) — and that fused identity feeds the
            // apply-by-email `mailto:` sink, i.e. a message addressed to B under
            // A's name. Same rule, same emptiness test (whitespace-only values
            // are reachable from pre-trim builds — see [`SQL_TRIM_CHARS`]), as
            // `super::Application::canonicalize_contact`, which applies it to an
            // imported bundle — the two MUST stay in lockstep.
            //
            // Statement 1 preserves an apply-by-email contact that statement 2
            // will NOT promote (the canonical pair is occupied by someone else)
            // by appending it to `notes`. Without it that person is lost for
            // good: the store stops reading the deprecated columns, and an
            // export mirrors the canonical pair, so an export/import round trip
            // could never recover them. Mirrors the note this migration's
            // import-time twin builds — see `super::contact`.
            //
            // NON-DESTRUCTIVE: the deprecated columns are never written, so a
            // row whose two pairs genuinely differed still has its old alias
            // value on disk too. The store simply stops reading them
            // (`super::SELECT_COLS` / `super::ApplicationStore::write_row_conn`).
            //
            // Idempotent: statement 1 skips a note it already appended (`instr`
            // on the exact line), and statement 2's `WHERE` no longer matches a
            // row it already promoted.
            conn.execute_batch(&format!(
                "UPDATE applications
                    SET notes = CASE WHEN notes = '' THEN '' ELSE notes || char(10) || char(10) END
                                || 'Apply-by-email: ' || recipient_name || ' <' || recipient_email || '>'
                  WHERE (TRIM(recipient_name, {WS}) <> '' OR TRIM(recipient_email, {WS}) <> '')
                    AND NOT (TRIM(contact_name, {WS}) = '' AND TRIM(contact_email, {WS}) = '')
                    AND (recipient_name <> contact_name OR recipient_email <> contact_email)
                    AND instr(
                          notes,
                          'Apply-by-email: ' || recipient_name || ' <' || recipient_email || '>'
                        ) = 0;

                 UPDATE applications
                    SET contact_name  = recipient_name,
                        contact_email = recipient_email
                  WHERE TRIM(contact_name, {WS}) = '' AND TRIM(contact_email, {WS}) = ''
                    AND (TRIM(recipient_name, {WS}) <> '' OR TRIM(recipient_email, {WS}) <> '');",
                WS = SQL_TRIM_CHARS
            ))
        },
    },
    Migration {
        name: "add_applications_next_action_notified_at",
        up: |conn| {
            // Reminder dedupe marker: epoch-ms of the follow-up notification
            // already raised for the CURRENT `next_action_at`. NULL = not yet
            // notified, so a fresh/rescheduled reminder fires exactly once.
            // Nullable (never 0) for the same reason as `next_action_at`.
            //
            // BACKFILL — "already announced" for everything ALREADY due. Without
            // it every pre-existing overdue reminder is NULL on first boot after
            // the upgrade, so the very first sweep treats the user's whole
            // backlog as brand-new and announces it: a banner storm the moment
            // they open the app. `MAX_PER_SWEEP` only paces that (5 per 30 min),
            // it does not suppress it. Stamping the migration's own timestamp
            // means "we consider these delivered"; a reminder in the FUTURE is
            // left NULL so it still notifies when it comes due, and moving any
            // due date clears the marker (`update_fields`) so a deliberate
            // reschedule always re-announces.
            //
            // The `<=` boundary matches `reminder_scheduler::should_notify`'s
            // inclusive due test, so a reminder due at exactly this instant is
            // suppressed by the same rule that would have fired it.
            //
            // (The backfill was folded into this body rather than appended as
            // migration 9: entry 8 has never been RELEASED — it was added on
            // this same unmerged branch — so no installed database has run it,
            // and amending in place keeps one schema step per feature. Editing a
            // shipped body would be forbidden.)
            let at = crate::db::ts_to_db(crate::db::now_ms());
            conn.execute_batch(&format!(
                "ALTER TABLE applications ADD COLUMN next_action_notified_at INTEGER;

                 UPDATE applications SET next_action_notified_at = {at}
                  WHERE next_action_at IS NOT NULL AND next_action_at <= {at};"
            ))
        },
    },
    Migration {
        name: "add_backfill_state",
        up: |conn| {
            // Durable one-shot marker table for `ApplicationStore::
            // backfill_from_generations` below — a name/done_at row means that
            // migration has already run and must never re-scan
            // `ai_generations.db` again. See the constant's own doc for why a
            // per-row epoch guard alone cannot close the resurrection window.
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS backfill_state (
                    name    TEXT PRIMARY KEY,
                    done_at INTEGER NOT NULL
                );",
            )
        },
    },
];

/// `backfill_state.name` for [`ApplicationStore::backfill_from_generations`]'s
/// durable one-shot marker: once set, [`super::ApplicationStore::open`] never
/// re-scans `ai_generations.db` again — see that function's own doc for why
/// re-running it resurrects a deletion.
const LEGACY_BACKFILL_MARKER: &str = "legacy_generations_backfill";

impl ApplicationStore {
    /// **One-time**, forward-safe backfill from the pre-split `ai_generations.db`:
    ///
    /// 1. `ALTER TABLE ai_generations ADD COLUMN application_id` (+ index) — guarded
    ///    by [`crate::db::column_exists`] so a re-run is a no-op.
    /// 2. For each generation OLDER than [`APPLICATIONS_FEATURE_EPOCH_MS`] (own
    ///    doc; younger left NULL), ensure exactly one `Application(status=
    ///    applied, applied_at=created_at)` keyed by `normalize(job_url)`,
    ///    link the gen's `application_id`, and seed one status event.
    ///
    /// **Runs at most once per data dir, ever — a migration, not a recurring
    /// reconciliation.** Every row it repairs predates the Applications
    /// feature entirely, so once it has run there is nothing left it could
    /// legitimately still find. Re-running it on a later boot does not merely
    /// waste work: a row still unlinked at that point is usually one whose
    /// Application the user deliberately deleted since — deleting an
    /// Application detaches its generations back to `NULL` without touching
    /// `created_at`, so the row is STILL pre-epoch and STILL unmatched, and an
    /// unconditional re-scan would recreate the Application the delete just
    /// removed. [`APPLICATIONS_FEATURE_EPOCH_MS`] alone narrows that window;
    /// it cannot close it (own doc there).
    ///
    /// Guarded by [`LEGACY_BACKFILL_MARKER`]: checked FIRST (an early return,
    /// no I/O against `ai_generations.db` at all once set) and set LAST, only
    /// after every row above has been processed — so a run that errors out
    /// partway is retried on the next boot instead of silently marked done.
    /// (`legacy_backfill_done`'s own `?` is part of that: a transient read
    /// failure must re-run the scan, not be treated as "not done yet".)
    pub(super) fn backfill_from_generations(&self, data_dir: &Path) -> AppResult<()> {
        if self.legacy_backfill_done()? {
            return Ok(()); // ran once already — see doc above, never re-scan
        }
        self.scan_legacy_generations(data_dir)?;
        self.mark_legacy_backfill_done()
    }

    /// **Restore-specific**, called ONLY from `commands::data::data_import`,
    /// right after it replaces `ai_generations.db` from a bundle whose
    /// `stores` object has NO `"applications"` key — i.e. verifiably a
    /// pre-`ApplicationStore`-era backup. That absence is what makes this
    /// safe to run outside the one-shot marker: `AiGenerationStore::
    /// detach_application` (the source of the "deliberately unlinked" rows
    /// this whole marker exists to protect, own doc above) could not
    /// possibly have run against any row in a bundle from before
    /// `ApplicationStore` existed, so nothing in it can be a
    /// deliberately-detached row masquerading as a never-processed legacy
    /// one — every unlinked pre-epoch row genuinely has never been scanned.
    ///
    /// Without this, a legacy bundle imported after the marker is already
    /// set (the common case — the marker is set on the very first boot,
    /// long before any later restore) leaves its generations permanently
    /// orphaned: [`Self::backfill_from_generations`] no-ops forever once the
    /// marker is set, and this bundle shape never even reaches the store
    /// through the normal boot path.
    ///
    /// Deliberately does NOT touch [`LEGACY_BACKFILL_MARKER`] — that marker
    /// gates the recurring BOOT-time scan, not this one-off, structurally
    /// scoped restore pass.
    ///
    /// **Never call this for a bundle that DOES carry an `"applications"`
    /// section** — a modern full backup already restores self-consistent
    /// `application_id` links via `ApplicationStore::import`/
    /// `AiGenerationStore::import` running on the SAME snapshot, and it CAN
    /// legitimately contain a deliberately-detached row (a real user
    /// `keepDocuments=true` delete before the backup was taken) — re-scanning
    /// that would resurrect it, the exact bug the marker exists to prevent.
    pub fn relink_legacy_generations_after_restore(&self, data_dir: &Path) -> AppResult<()> {
        self.scan_legacy_generations(data_dir)
    }

    /// Core one-shot scan body shared by [`Self::backfill_from_generations`]
    /// (marker-gated, every boot) and [`Self::relink_legacy_generations_after_restore`]
    /// (marker-bypassing, restore-only — own doc there for why that is safe).
    /// Never touches the marker itself.
    fn scan_legacy_generations(&self, data_dir: &Path) -> AppResult<()> {
        let gen_path = data_dir.join("ai_generations.db");
        if gen_path.exists() {
            let gen_conn = crate::db::open(&gen_path)?;

            // Step 1: add the FK column + index if absent (idempotent guard).
            if !crate::db::column_exists(&gen_conn, "ai_generations", "application_id") {
                gen_conn.execute_batch(
                    "ALTER TABLE ai_generations ADD COLUMN application_id TEXT;
                     CREATE INDEX IF NOT EXISTS idx_ai_generations_application_id
                         ON ai_generations(application_id);",
                )?;
            }

            struct GenRow {
                id: String,
                created_at: u64,
                candidate: String,
                title: String,
                company: String,
                job_url: String,
                board: String,
                answers: Vec<ApplicationAnswer>,
                brief: String,
                application_id: Option<String>,
            }

            let mut stmt = gen_conn.prepare(
                "SELECT id, created_at, candidate_name, job_title, company_name,
                        job_url, board, application_answers, company_brief, application_id
                 FROM ai_generations
                 ORDER BY created_at ASC",
            )?;
            let mut dropped = 0usize;
            let rows: Vec<GenRow> = stmt
                .query_map([], |row| {
                    let answers_json: String = row.get(7)?;
                    let app_id: Option<String> = row.get(9).unwrap_or(None);
                    Ok(GenRow {
                        id: row.get(0)?,
                        created_at: ts_from_db(row.get::<_, i64>(1)?),
                        candidate: row.get(2)?,
                        title: row.get(3)?,
                        company: row.get(4)?,
                        job_url: row.get(5)?,
                        board: row.get(6)?,
                        answers: serde_json::from_str(&answers_json).unwrap_or_default(),
                        brief: row.get(8)?,
                        application_id: app_id.filter(|s| !s.is_empty()),
                    })
                })?
                .filter_map(|r| match r {
                    Ok(row) => Some(row),
                    Err(_) => {
                        dropped += 1;
                        None
                    }
                })
                .collect();
            drop(stmt);
            if dropped > 0 {
                // Path-free, content-free (ADR-027): a count only, never the
                // row's id/company/url. These rows are never retried — the
                // marker this scan feeds into is one-shot — so this is the
                // only signal their loss is ever diagnosable from.
                log::warn!(
                    "[applications] legacy backfill dropped {dropped} generation row(s) \
                     that failed to parse (not retried)"
                );
            }

            for g in rows {
                if g.application_id.is_some() {
                    continue; // already linked — idempotent skip
                }
                if g.created_at >= APPLICATIONS_FEATURE_EPOCH_MS {
                    continue; // modern + still unmatched — own doc on the constant
                }
                let normalized = normalize_job_url(&g.job_url);
                let meta = ApplicationMeta {
                    company: g.company,
                    title: g.title,
                    candidate: g.candidate,
                    brief: g.brief,
                    job_description: String::new(), // ponytail: legacy generations carry no JD column
                    answers: g.answers,
                    job_summary: String::new(),
                    salary_min: None,
                    salary_max: None,
                    salary_currency: None,
                };
                // Empty url → a fresh per-gen Application (no shared key to merge on);
                // non-empty url → merge into that url's Application if one exists.
                let app_id = self.upsert_internal(
                    &normalized,
                    &g.board,
                    &meta,
                    &meta.job_description,
                    Some(g.created_at),
                )?;
                gen_conn.execute(
                    "UPDATE ai_generations SET application_id = ?2 WHERE id = ?1",
                    params![g.id, app_id],
                )?;
            }
        } // else: nothing to scan — the caller decides what "done" means here.

        Ok(())
    }

    /// Whether the one-shot marker [`LEGACY_BACKFILL_MARKER`] is set.
    /// `.optional()` so only `QueryReturnedNoRows` reads as "not done yet" —
    /// any OTHER error (e.g. a transient SQLite failure) propagates through
    /// `?` instead of being swallowed into a false `false`, which would make
    /// [`Self::backfill_from_generations`] re-run the one-shot scan on a
    /// glitch and recreate a deliberately-deleted `Application` — the exact
    /// bug the marker exists to prevent, reachable through a different door.
    fn legacy_backfill_done(&self) -> AppResult<bool> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT 1 FROM backfill_state WHERE name = ?1",
                params![LEGACY_BACKFILL_MARKER],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// Set it. `INSERT OR REPLACE` so a caller never special-cases "already set".
    fn mark_legacy_backfill_done(&self) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO backfill_state (name, done_at) VALUES (?1, ?2)",
            params![LEGACY_BACKFILL_MARKER, ts_to_db(now_ms())],
        )?;
        Ok(())
    }
}
