//! The `applications` row <-> [`Application`] mapping and the connection-scoped
//! row primitives every write path runs inside its transaction.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). Persistence still lives entirely inside
//! [`super::ApplicationStore`], on the SAME connection.

use rusqlite::{params, Connection};

use super::{clamp_to_bytes, Application, ApplicationStatus, ApplicationStore};
use crate::db::{ts_from_db, ts_to_db};
use crate::error::AppResult;

impl ApplicationStore {
    /// Connection-scoped row write, callable inside a transaction. `conn` may be a
    /// plain `&Connection` or a `&Transaction` (which derefs to `&Connection`).
    pub(super) fn write_row_conn(conn: &Connection, app: &Application) -> AppResult<()> {
        let answers_json = serde_json::to_string(&app.answers).unwrap_or_else(|_| "[]".into());
        let job_summary = clamp_to_bytes(app.job_summary.clone(), MAX_JOB_SUMMARY_BYTES);
        // The deprecated `recipient_name`/`recipient_email` columns are NOT in
        // this statement: the canonical pair is `contact_name`/`contact_email`
        // (migration `unify_application_contact`). Omitting them means a new row
        // gets their `DEFAULT ''` and an existing row keeps its pre-unification
        // value untouched — additive, never destructive.
        //
        // `next_action_notified_at` IS written, from the struct — so every
        // caller must carry the existing value forward. Rust enforces that
        // mechanically: `Application` is built by exhaustive struct literals, so
        // a new write path cannot compile without deciding what the marker is.
        conn.execute(
            "INSERT INTO applications
                (id, status, applied_at, created_at, updated_at, job_url, board,
                 company, title, candidate, answers, brief, notes, next_action_at,
                 comp, contact_name, contact_email, job_description, job_summary,
                 salary_min, salary_max, salary_currency, next_action_notified_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)
             ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                applied_at = excluded.applied_at,
                updated_at = excluded.updated_at,
                job_url = excluded.job_url,
                board = excluded.board,
                company = excluded.company,
                title = excluded.title,
                candidate = excluded.candidate,
                answers = excluded.answers,
                brief = excluded.brief,
                notes = excluded.notes,
                next_action_at = excluded.next_action_at,
                comp = excluded.comp,
                contact_name = excluded.contact_name,
                contact_email = excluded.contact_email,
                job_description = excluded.job_description,
                job_summary = excluded.job_summary,
                salary_min = excluded.salary_min,
                salary_max = excluded.salary_max,
                salary_currency = excluded.salary_currency,
                next_action_notified_at = excluded.next_action_notified_at",
            params![
                app.id,
                app.status.as_id(),
                app.applied_at.map(ts_to_db),
                ts_to_db(app.created_at),
                ts_to_db(app.updated_at),
                app.job_url,
                app.board,
                app.company,
                app.title,
                app.candidate,
                answers_json,
                app.brief,
                app.notes,
                app.next_action_at.map(ts_to_db),
                app.comp,
                app.contact_name,
                app.contact_email,
                app.job_description,
                job_summary,
                app.salary_min,
                app.salary_max,
                app.salary_currency,
                app.next_action_notified_at.map(ts_to_db),
            ],
        )?;
        Ok(())
    }

    /// Connection-scoped single-row read by id, callable inside an existing
    /// lock/transaction — unlike [`Self::get`] (which takes its OWN lock;
    /// calling `get` while already holding `self.conn.lock()` would deadlock
    /// the non-reentrant `parking_lot::Mutex`). Used by
    /// [`Self::merge_answers`] so its dedup read and its write land in the
    /// exact same transaction.
    pub(super) fn row_by_id_conn(conn: &Connection, id: &str) -> AppResult<Option<Application>> {
        use rusqlite::OptionalExtension;
        let mut stmt = conn.prepare(&format!("{SELECT_COLS} WHERE id = ?1"))?;
        Ok(stmt.query_row(params![id], row_to_application).optional()?)
    }

    /// `find_by_job_url` sibling for [`Self::upsert_internal`] (self-locking here would deadlock).
    pub(super) fn row_by_job_url_conn(
        conn: &Connection,
        normalized: &str,
    ) -> AppResult<Option<Application>> {
        use rusqlite::OptionalExtension;
        if normalized.is_empty() {
            return Ok(None);
        }
        let sql = format!("{SELECT_COLS} WHERE job_url = ?1 ORDER BY created_at DESC LIMIT 1");
        let mut stmt = conn.prepare(&sql)?;
        Ok(stmt
            .query_row(params![normalized], row_to_application)
            .optional()?)
    }
}

/// Server-side hard cap on a persisted job summary. The Zod `.max(50_000)` on the
/// IPC schema is UX-only — this is the real bound, applied in the store write path
/// so the IPC and any future import path are both protected.
// ponytail: byte-cap + char-boundary truncate; raise the const if summaries grow.
const MAX_JOB_SUMMARY_BYTES: usize = 50_000;

/// Column projection shared by `list`/`get`/`find_by_job_url` so order lives once.
///
/// The deprecated `recipient_name`/`recipient_email` COLUMNS are deliberately
/// absent: since `unify_application_contact` the canonical pair is
/// `contact_name`/`contact_email`, and the struct's alias fields are mirrored
/// from it in [`row_to_application`].
pub(super) const SELECT_COLS: &str =
    "SELECT id, status, applied_at, created_at, updated_at, job_url, board,
            company, title, candidate, answers, brief, notes, next_action_at,
            comp, contact_name, contact_email, job_description, job_summary,
            salary_min, salary_max, salary_currency, next_action_notified_at
     FROM applications";

pub(super) fn row_to_application(row: &rusqlite::Row) -> rusqlite::Result<Application> {
    let status_raw: String = row.get(1)?;
    let answers_json: String = row.get(10)?;
    // Canonical contact pair, read once and mirrored onto the deprecated
    // `recipient_*` alias fields below (see `Application::recipient_name`).
    let contact_name: String = row.get(15)?;
    let contact_email: String = row.get(16)?;
    Ok(Application {
        id: row.get(0)?,
        status: ApplicationStatus::from_id(&status_raw),
        applied_at: row.get::<_, Option<i64>>(2)?.map(ts_from_db),
        created_at: ts_from_db(row.get::<_, i64>(3)?),
        updated_at: ts_from_db(row.get::<_, i64>(4)?),
        job_url: row.get(5)?,
        board: row.get(6)?,
        company: row.get(7)?,
        title: row.get(8)?,
        candidate: row.get(9)?,
        answers: serde_json::from_str(&answers_json).unwrap_or_default(),
        brief: row.get(11)?,
        notes: row.get(12)?,
        next_action_at: row.get::<_, Option<i64>>(13)?.map(ts_from_db),
        next_action_notified_at: row.get::<_, Option<i64>>(22)?.map(ts_from_db),
        comp: row.get(14)?,
        job_description: row.get(17)?,
        job_summary: row.get(18)?,
        recipient_name: contact_name.clone(),
        recipient_email: contact_email.clone(),
        contact_name,
        contact_email,
        // NULL (unknown salary, e.g. a pre-migration row) → None, never 0.
        salary_min: row.get(19)?,
        salary_max: row.get(20)?,
        salary_currency: row.get(21)?,
    })
}
