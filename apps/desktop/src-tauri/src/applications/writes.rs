//! The write side of the `applications` store: the per-url upsert every creation
//! trigger funnels through, the status transition, the field patch, and delete.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). Persistence still lives entirely inside
//! [`super::ApplicationStore`], on the SAME connection.

use rusqlite::params;

use super::model::clamp_job_description;
use super::{
    make_application_id, normalize_job_url, Application, ApplicationMeta, ApplicationOrigin,
    ApplicationStatus, ApplicationStore, EVENT_SOURCE_USER,
};
use crate::db::{now_ms, ts_to_db};
use crate::error::{AppError, AppResult};

impl ApplicationStore {
    /// The single dedup/merge entry point for all four creation triggers
    /// (Save→`saved`, Apply/Generate→`applied`, Manual→`applied`, Backfill).
    /// Normalizes `job_url`, merges meta onto any existing Application for that
    /// url, and returns the (new or existing) Application id.
    ///
    /// `applied`: `Some(true)` forces `applied`; `Some(false)`/`None` defers to the
    /// origin (Save stays `saved`; Generate/Manual/Backfill apply).
    pub fn upsert_for_origin(
        &self,
        job_url: &str,
        board: &str,
        meta: &ApplicationMeta,
        origin: ApplicationOrigin,
        applied: Option<bool>,
    ) -> AppResult<String> {
        let normalized = normalize_job_url(job_url);
        let origin_applies = matches!(
            origin,
            ApplicationOrigin::Generate | ApplicationOrigin::Manual | ApplicationOrigin::Backfill
        );
        let applied_at = if applied == Some(true) || origin_applies {
            Some(now_ms())
        } else {
            None
        };
        // The Rust store is the real trust boundary: clamp the JD here so BOTH the
        // import funnel and direct IPC callers are capped (the renderer Zod cap is
        // UX-only and the import path never passes through it). Truncate, never drop.
        let clamped_jd = clamp_job_description(meta.job_description.clone());
        self.upsert_internal(&normalized, board, meta, &clamped_jd, applied_at)
    }

    /// Core upsert: `normalized` is already normalized; `applied_at` Some marks the
    /// Application `applied`, None keeps it `saved`. Merges into an existing row
    /// when the (non-empty) url already has an Application; only ever advances OUT
    /// of `saved`, never demotes an applied+ row. `answers` merge by QUESTION via
    /// [`Self::merge_answers_by_question`] — not a wholesale replace.
    pub(super) fn upsert_internal(
        &self,
        normalized: &str,
        board: &str,
        meta: &ApplicationMeta,
        // Already clamped by the caller (`upsert_for_origin`) to
        // `MAX_JOB_DESCRIPTION_BYTES`; both write branches below use this, never
        // `meta.job_description`, so the cap can't be bypassed.
        clamped_jd: &str,
        applied_at: Option<u64>,
    ) -> AppResult<String> {
        let pick = |inc: &str, ex: &str| -> String {
            if inc.trim().is_empty() {
                ex.to_string()
            } else {
                inc.to_string()
            }
        };

        // Lookup + write share ONE lock/transaction (`row_by_job_url_conn`, not
        // self-locking `find_by_job_url`) — the old separately-released lookup
        // lock left a gap where a concurrent `merge_answers` commit got clobbered.
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;

        if let Some(existing) = Self::row_by_job_url_conn(&tx, normalized)? {
            let now = now_ms();
            let (status, new_applied_at) = if existing.status.is_pre_apply() && applied_at.is_some()
            {
                (
                    ApplicationStatus::Applied,
                    applied_at.or(existing.applied_at),
                )
            } else {
                (existing.status, existing.applied_at)
            };
            let answers =
                Self::merge_answers_by_question(existing.answers.clone(), meta.answers.clone());
            let app = Application {
                id: existing.id.clone(),
                status,
                applied_at: new_applied_at,
                created_at: existing.created_at,
                updated_at: now,
                job_url: normalized.to_string(),
                board: pick(board, &existing.board),
                company: pick(&meta.company, &existing.company),
                title: pick(&meta.title, &existing.title),
                candidate: pick(&meta.candidate, &existing.candidate),
                answers,
                brief: pick(&meta.brief, &existing.brief),
                job_description: pick(clamped_jd, &existing.job_description),
                notes: existing.notes.clone(),
                next_action_at: existing.next_action_at,
                // Carried through untouched: a re-scrape/re-track must never
                // re-arm a reminder the user has already been notified about.
                next_action_notified_at: existing.next_action_notified_at,
                comp: existing.comp.clone(),
                contact_name: existing.contact_name.clone(),
                contact_email: existing.contact_email.clone(),
                job_summary: pick(&meta.job_summary, &existing.job_summary),
                // Deprecated aliases: always the canonical value, never a
                // second source of truth (see `Application::recipient_name`).
                recipient_name: existing.contact_name.clone(),
                recipient_email: existing.contact_email.clone(),
                // COALESCE(new, old): a re-scrape/re-track fills salary the first
                // time it becomes known, but never clobbers an already-known value
                // with an unknown (`None`) one.
                salary_min: meta.salary_min.or(existing.salary_min),
                salary_max: meta.salary_max.or(existing.salary_max),
                salary_currency: meta
                    .salary_currency
                    .clone()
                    .or_else(|| existing.salary_currency.clone()),
            };
            // Row write + status event share the transaction opened above.
            Self::write_row_conn(&tx, &app)?;
            if status != existing.status {
                Self::append_event_conn(
                    &tx,
                    &app.id,
                    existing.status.as_id(),
                    status.as_id(),
                    "",
                    EVENT_SOURCE_USER,
                    true,
                    now,
                )?;
            }
            tx.commit()?;
            return Ok(app.id);
        }

        let now = now_ms();
        let status = if applied_at.is_some() {
            ApplicationStatus::Applied
        } else {
            ApplicationStatus::Saved
        };
        let app = Application {
            id: make_application_id(),
            status,
            applied_at,
            created_at: now,
            updated_at: now,
            job_url: normalized.to_string(),
            board: board.to_string(),
            company: meta.company.clone(),
            title: meta.title.clone(),
            candidate: meta.candidate.clone(),
            // Route the new-row branch through the same capped+deduped merge as an
            // existing-row update (against an empty existing list) so a caller
            // handing `upsert_for_origin` an oversized/duplicate-question
            // `meta.answers` on FIRST creation can't bypass `MAX_TOTAL_ANSWERS` the
            // way storing `meta.answers` verbatim did.
            answers: Self::merge_answers_by_question(Vec::new(), meta.answers.clone()),
            brief: meta.brief.clone(),
            job_description: clamped_jd.to_string(),
            notes: String::new(),
            next_action_at: None,
            next_action_notified_at: None,
            comp: String::new(),
            contact_name: String::new(),
            contact_email: String::new(),
            job_summary: meta.job_summary.clone(),
            recipient_name: String::new(),
            recipient_email: String::new(),
            salary_min: meta.salary_min,
            salary_max: meta.salary_max,
            salary_currency: meta.salary_currency.clone(),
        };
        // New row: its seed status event shares the same transaction.
        Self::write_row_conn(&tx, &app)?;
        Self::append_event_conn(
            &tx,
            &app.id,
            "",
            status.as_id(),
            "",
            EVENT_SOURCE_USER,
            true,
            now,
        )?;
        tx.commit()?;
        Ok(app.id)
    }

    /// Manual create from the `/applications` page. Optional url; everything else
    /// from `meta`. Always `applied` (a hand-tracked pursuit already applied to).
    pub fn track_manual(
        &self,
        job_url: &str,
        board: &str,
        meta: &ApplicationMeta,
    ) -> AppResult<String> {
        self.upsert_for_origin(job_url, board, meta, ApplicationOrigin::Manual, Some(true))
    }

    /// Transition an Application's status, appending one history event and bumping
    /// `updated_at`. Sets `applied_at` the first time it leaves `saved`.
    pub fn set_status(&self, id: &str, to: ApplicationStatus, note: &str) -> AppResult<()> {
        // The READ joins the same transaction as the two writes. The row UPDATE
        // and its append-only status event must land together or not at all —
        // otherwise a crash between them leaves the status changed with no
        // history row — and reading through a separately-released lock let a
        // concurrent transition land in the gap, so `from_status` below recorded
        // a status this row no longer had.
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        let existing = Self::row_by_id_conn(&tx, id)?
            .ok_or_else(|| AppError::Validation(format!("application not found: {id}")))?;
        let now = now_ms();
        let applied_at = if existing.applied_at.is_none() && !to.is_pre_apply() {
            Some(now)
        } else {
            existing.applied_at
        };
        tx.execute(
            "UPDATE applications SET status = ?2, applied_at = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, to.as_id(), applied_at.map(ts_to_db), ts_to_db(now)],
        )?;
        Self::append_event_conn(
            &tx,
            id,
            existing.status.as_id(),
            to.as_id(),
            note,
            EVENT_SOURCE_USER,
            true,
            now,
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Patch the user-editable tracking fields. Each `None` leaves its field
    /// unchanged; bumps `updated_at` whenever called.
    ///
    /// **Contact unification:** `recipient_name`/`recipient_email` are deprecated
    /// aliases of `contact_name`/`contact_email` (see
    /// [`Application::recipient_name`]) — both inbound names patch the SAME
    /// canonical storage. When a caller sends both, the canonical one wins.
    ///
    /// **Reminders:** changing (or clearing) `next_action_at` drops the
    /// `next_action_notified_at` dedupe marker in the same transaction, so a
    /// rescheduled follow-up notifies again exactly once.
    #[allow(clippy::too_many_arguments)]
    pub fn update_fields(
        &self,
        id: &str,
        notes: Option<String>,
        next_action_at: Option<Option<u64>>,
        comp: Option<String>,
        contact_name: Option<String>,
        contact_email: Option<String>,
        job_description: Option<String>,
        job_summary: Option<String>,
        recipient_name: Option<String>,
        recipient_email: Option<String>,
    ) -> AppResult<()> {
        // ONE lock/transaction spans lookup + write + the marker clear
        // (`row_by_id_conn`, not self-locking `get`): the write rewrites EVERY
        // column, so releasing the lock between them let a commit be lost, and
        // the marker clear must land with the row or not at all.
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        let existing = Self::row_by_id_conn(&tx, id)?
            .ok_or_else(|| AppError::Validation(format!("application not found: {id}")))?;
        let next_action_at = next_action_at.unwrap_or(existing.next_action_at);
        let reminder_rescheduled = next_action_at != existing.next_action_at;
        // Canonical pair wins over the deprecated alias when both are supplied;
        // otherwise whichever one was supplied patches it; absent in both leaves
        // the stored value alone.
        let contact_name = contact_name
            .or(recipient_name)
            .unwrap_or(existing.contact_name);
        let contact_email = contact_email
            .or(recipient_email)
            .unwrap_or(existing.contact_email);
        // A new (or cleared) due date is a NEW reminder — forget that the old one
        // was already announced so the scheduler can fire once for it.
        let next_action_notified_at = if reminder_rescheduled {
            None
        } else {
            existing.next_action_notified_at
        };
        let app = Application {
            notes: notes.unwrap_or(existing.notes),
            next_action_at,
            next_action_notified_at,
            comp: comp.unwrap_or(existing.comp),
            // Clamp Some(s) at the store boundary; None still preserves the stored JD
            // (the IPC arg is attacker-reachable and bypasses the renderer Zod cap).
            job_description: job_description
                .map(clamp_job_description)
                .unwrap_or(existing.job_description),
            job_summary: job_summary.unwrap_or(existing.job_summary),
            recipient_name: contact_name.clone(),
            recipient_email: contact_email.clone(),
            contact_name,
            contact_email,
            updated_at: now_ms(),
            ..existing
        };
        Self::write_row_conn(&tx, &app)?;
        tx.commit()?;
        Ok(())
    }

    /// Delete an Application and its status history. `keep_documents` is consumed
    /// at the command layer (it decides whether child generations are also
    /// deleted); this store owns only the Application + its events, so the flag is
    /// accepted for a uniform signature and does not change the row deletion here.
    pub fn delete(&self, id: &str, _keep_documents: bool) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM applications WHERE id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM status_events WHERE application_id = ?1",
            params![id],
        )?;
        Ok(())
    }
}
