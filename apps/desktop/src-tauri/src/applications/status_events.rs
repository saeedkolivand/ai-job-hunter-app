//! `status_events` — the append-only audit trail for an [`super::
//! Application`]'s status, split out of [`super`] so that file stays under
//! the architecture LOC cap (`tests/architecture.rs` R8) — same reason
//! `migrations.rs`/`reminders.rs` were split out. Persistence still lives
//! entirely inside [`super::ApplicationStore`]; this is the SAME store,
//! just its status-history slice: reads ([`ApplicationStore::events`]),
//! writes ([`ApplicationStore::transition_status_if`]/[`ApplicationStore::
//! transition_status_if_sourced`], and the connection-scoped [`append_event_conn`]
//! every row write in [`super`] still calls through `Self::`), and v2 slice
//! 2's accept/reject/reject-replay-guard trio.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::db::{now_ms, ts_from_db, ts_to_db};
use crate::error::AppResult;

use super::{ApplicationStatus, ApplicationStore};

/// Who/what asserted a [`StatusEvent`] — the user (directly, or via any
/// in-app/extension action) driving every write before v2 slice 2.
pub(crate) const EVENT_SOURCE_USER: &str = "user";
/// Email tracking v2's auto-write (`crate::email_watch::auto_write`) — the
/// ONLY source that may ever write a row with `confirmed: false`. See
/// [`StatusEvent::confirmed`]'s doc for why nothing else may.
pub(crate) const EVENT_SOURCE_EMAIL: &str = "email";
/// The reversal row [`ApplicationStore::reject_latest_status_event`] appends
/// when the user rejects an [`EVENT_SOURCE_EMAIL`] write — a DISTINCT source
/// value (not [`EVENT_SOURCE_USER`]) so [`ApplicationStore::
/// was_transition_rejected`] can precisely recognize "the user explicitly
/// rejected this exact transition" and never mistake an ordinary manual
/// status change for one.
pub(crate) const EVENT_SOURCE_EMAIL_REJECT: &str = "email_reject";

/// One append-only status-history row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEvent {
    pub application_id: String,
    /// Empty for the seed event of a freshly-created Application.
    pub from_status: String,
    pub to_status: String,
    pub at: u64,
    #[serde(default)]
    pub note: String,
    /// [`EVENT_SOURCE_USER`] (every pre-v2 row, and every user-driven write
    /// today) or [`EVENT_SOURCE_EMAIL`]/[`EVENT_SOURCE_EMAIL_REJECT`] (v2
    /// slice 2). A free-form `String`, not an enum, so a future source
    /// doesn't need a schema migration to add — deliberately mirrors
    /// `from_status`/`to_status` above, which are already raw strings at
    /// this storage layer for the same reason.
    ///
    /// `StatusEvent` is not part of [`crate::data_store::DataStore::export`]/
    /// [`crate::data_store::DataStore::import`] (only `Application` is —
    /// status history is audit-only and excluded from the portable bundle,
    /// see `ApplicationStore`'s `impl DataStore` in `super`), so this
    /// field's default has no backup-compat obligation; it exists purely so
    /// an in-process construction/round-trip (e.g. a test) can't silently
    /// produce an empty-string source instead of a real one.
    #[serde(default = "default_event_source")]
    pub source: String,
    /// Whether a human has reviewed this transition. Every pre-v2 row (and
    /// every [`EVENT_SOURCE_USER`] write today) is `true`. **Nothing may
    /// ever write `false` except the email-derived auto-write itself** — an
    /// unconfirmed row is the whole safety model for a classifier with a
    /// recorded precision limit (see `crate::email_watch::intent`'s module
    /// doc). Cleared in place by [`ApplicationStore::
    /// accept_latest_status_event`]/[`ApplicationStore::
    /// reject_latest_status_event`] — never by editing `from_status`/
    /// `to_status`/`at`, so the append-only trail always still shows exactly
    /// what the email claimed.
    #[serde(default = "default_event_confirmed")]
    pub confirmed: bool,
}

fn default_event_source() -> String {
    EVENT_SOURCE_USER.to_string()
}

fn default_event_confirmed() -> bool {
    true
}

impl ApplicationStore {
    /// History for one Application, oldest-first.
    pub fn events(&self, id: &str) -> Vec<StatusEvent> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT application_id, from_status, to_status, at, note, source, confirmed
             FROM status_events WHERE application_id = ?1 ORDER BY at ASC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(params![id], |row| {
                Ok(StatusEvent {
                    application_id: row.get(0)?,
                    from_status: row.get(1)?,
                    to_status: row.get(2)?,
                    at: ts_from_db(row.get::<_, i64>(3)?),
                    note: row.get(4)?,
                    source: row.get(5)?,
                    confirmed: row.get::<_, i64>(6)? != 0,
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    /// Atomic compare-and-set: transition an Application's status ONLY if its
    /// CURRENT status is exactly `from` — the read-check-write happens under
    /// ONE lock/transaction (`UPDATE ... WHERE id=? AND status=?`), unlike a
    /// caller doing its own `.get()` status check and then calling
    /// [`Self::set_status`] (which re-locks separately and writes
    /// unconditionally) — that pattern can lose a race between the check and
    /// the write. `pub(crate)`: the extension bridge's `status.update` guard is
    /// the first caller (`extension_bridge::status_update::resolve_status_update`).
    ///
    /// Thin, user-sourced, always-confirmed wrapper over [`Self::
    /// transition_status_if_sourced`] — kept as its OWN entry point (rather
    /// than widening this signature) specifically so its existing
    /// extension-bridge caller needs no change.
    ///
    /// Returns `Ok(true)` iff exactly one row matched `from` and was
    /// transitioned (with its status event appended, same transaction — an
    /// event-insert failure propagates and rolls the whole transaction back,
    /// so the status flip and its history row always commit or roll back
    /// together); `Ok(false)` when zero rows matched (no such id, or its
    /// status had already moved off `from` since the caller last checked —
    /// never a partial write). Mirrors `set_status`'s field semantics for the
    /// matched row: `updated_at` always bumps; `applied_at` is
    /// first-applied-wins (only set when currently `NULL`) whenever `to` is
    /// not pre-apply — a `saved` row CAN already carry a prior `applied_at`
    /// from an earlier applied -> saved demotion via the stage picker, and
    /// that timestamp must survive a re-transition back to `applied`; the
    /// event's `note` defaults to `""` when `None`.
    pub(crate) fn transition_status_if(
        &self,
        id: &str,
        from: ApplicationStatus,
        to: ApplicationStatus,
        note: Option<&str>,
    ) -> AppResult<bool> {
        self.transition_status_if_sourced(id, from, to, note, EVENT_SOURCE_USER, true)
    }

    /// Same atomic compare-and-set as [`Self::transition_status_if`], but
    /// records WHO/WHAT asserted the transition and whether it is already
    /// confirmed — see [`StatusEvent::source`]/[`StatusEvent::confirmed`].
    /// `pub(crate)`: `crate::email_watch::auto_write` is the email-derived
    /// caller (always `EVENT_SOURCE_EMAIL`, `confirmed: false`);
    /// [`Self::reject_latest_status_event`] is the other (always
    /// `EVENT_SOURCE_EMAIL_REJECT`, `confirmed: true`).
    pub(crate) fn transition_status_if_sourced(
        &self,
        id: &str,
        from: ApplicationStatus,
        to: ApplicationStatus,
        note: Option<&str>,
        source: &str,
        confirmed: bool,
    ) -> AppResult<bool> {
        let now = now_ms();
        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        let rows = if !to.is_pre_apply() {
            tx.execute(
                "UPDATE applications SET status = ?2, applied_at = COALESCE(applied_at, ?3), updated_at = ?4
                 WHERE id = ?1 AND status = ?5",
                params![id, to.as_id(), ts_to_db(now), ts_to_db(now), from.as_id()],
            )?
        } else {
            tx.execute(
                "UPDATE applications SET status = ?2, updated_at = ?3 WHERE id = ?1 AND status = ?4",
                params![id, to.as_id(), ts_to_db(now), from.as_id()],
            )?
        };
        if rows == 0 {
            tx.commit()?;
            return Ok(false);
        }
        Self::append_event_conn(
            &tx,
            id,
            from.as_id(),
            to.as_id(),
            note.unwrap_or(""),
            source,
            confirmed,
            now,
        )?;
        tx.commit()?;
        Ok(true)
    }

    /// Whether an email-derived `from -> to` transition for `id` was already
    /// rejected by the user — i.e. `status_events` already contains the
    /// reversal row [`Self::reject_latest_status_event`] appends for this
    /// EXACT pair (source [`EVENT_SOURCE_EMAIL_REJECT`], `from_status = to`,
    /// `to_status = from` — the reversed shape, since a reversal event walks
    /// back the ORIGINAL transition). `crate::email_watch::auto_write`
    /// consults this before every write so a later email carrying the SAME
    /// misfired signal (the classifier's own recorded precision limit — see
    /// `crate::email_watch::intent`'s module doc) can't re-apply a
    /// transition the user has already told us was wrong.
    ///
    /// Matching on [`EVENT_SOURCE_EMAIL_REJECT`] specifically (not a bare
    /// reversed-pair scan) means an ordinary, UNRELATED manual status change
    /// that happens to walk the same two statuses backward can never
    /// false-positive this gate — only [`Self::reject_latest_status_event`]
    /// ever writes that source value.
    pub(crate) fn was_transition_rejected(
        &self,
        id: &str,
        from: ApplicationStatus,
        to: ApplicationStatus,
    ) -> bool {
        use rusqlite::OptionalExtension;
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT 1 FROM status_events
             WHERE application_id = ?1 AND source = ?2 AND from_status = ?3 AND to_status = ?4
             LIMIT 1",
            params![id, EVENT_SOURCE_EMAIL_REJECT, to.as_id(), from.as_id()],
            |_| Ok(()),
        )
        .optional()
        .unwrap_or(None)
        .is_some()
    }

    /// Accept the most recent email-derived, unconfirmed transition for
    /// `id`: clears its `confirmed` flag IN PLACE. Never touches
    /// `applications.status` (the auto-write already applied it — accepting
    /// only marks it reviewed) and never edits `from_status`/`to_status`/
    /// `at`/`note`, so the append-only trail is unchanged; only the
    /// confirmation bit flips. One lock held for the whole read-then-write
    /// (no separate transaction needed — nothing here can partially apply).
    ///
    /// `Ok(false)`: no unconfirmed email-derived row to accept for `id` — a
    /// no-op, not an error (a caller need not check first).
    pub fn accept_latest_status_event(&self, id: &str) -> AppResult<bool> {
        use rusqlite::OptionalExtension;
        let conn = self.conn.lock();
        let rowid: Option<i64> = conn
            .query_row(
                "SELECT rowid FROM status_events
                 WHERE application_id = ?1 AND source = ?2 AND confirmed = 0
                 ORDER BY at DESC LIMIT 1",
                params![id, EVENT_SOURCE_EMAIL],
                |row| row.get(0),
            )
            .optional()?;
        let Some(rowid) = rowid else {
            return Ok(false);
        };
        conn.execute(
            "UPDATE status_events SET confirmed = 1 WHERE rowid = ?1",
            params![rowid],
        )?;
        Ok(true)
    }

    /// Reject the most recent email-derived, unconfirmed transition for
    /// `id`: reverts `applications.status` back to that event's
    /// `from_status`, but ONLY by compare-and-set — [`Self::
    /// transition_status_if_sourced`] (reused, not reimplemented) only
    /// succeeds if the CURRENT status still equals the event's `to_status`.
    /// **A status the user changed by hand in the meantime is never
    /// clobbered**: when the CAS loses, the provisional row is simply
    /// DISMISSED (its `confirmed` flag cleared below) rather than reverted.
    ///
    /// On a successful revert this APPENDS a reversal event (source
    /// [`EVENT_SOURCE_EMAIL_REJECT`]) recording that an email got it wrong —
    /// `status_events` stays append-only: the original transition row is
    /// never edited or deleted, only marked reviewed (`confirmed = 1`). That
    /// reversal row is also what [`Self::was_transition_rejected`] later
    /// consults to stop a later email from re-applying the same transition.
    ///
    /// **Not one atomic transaction** — the read (this fn's first step) and
    /// the CAS-revert (inside `transition_status_if_sourced`, its own
    /// lock/transaction) are two separate lock acquisitions, with the final
    /// `confirmed` flip a third. Each step alone is atomic and idempotent
    /// (the CAS only ever succeeds or safely no-ops; flipping `confirmed` to
    /// `1` twice is a no-op the second time), so a genuinely concurrent
    /// second call could at worst repeat a step harmlessly — never corrupt
    /// data. Not a realistic risk for a human-driven accept/reject click in
    /// a single-user desktop app; a fully serialized version would be
    /// over-engineering for a race that does not materialize here.
    ///
    /// Returns whether the status was actually reverted (`false` on a lost
    /// CAS/dismissal, or when there was no unconfirmed email-derived row to
    /// act on at all — both are legitimate no-ops, not errors).
    pub fn reject_latest_status_event(&self, id: &str) -> AppResult<bool> {
        use rusqlite::OptionalExtension;
        let pending: Option<(i64, String, String)> = {
            let conn = self.conn.lock();
            conn.query_row(
                "SELECT rowid, from_status, to_status FROM status_events
                 WHERE application_id = ?1 AND source = ?2 AND confirmed = 0
                 ORDER BY at DESC LIMIT 1",
                params![id, EVENT_SOURCE_EMAIL],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
        };
        let Some((rowid, from_status, to_status)) = pending else {
            return Ok(false);
        };
        let from = ApplicationStatus::from_id(&from_status);
        let to = ApplicationStatus::from_id(&to_status);

        let reverted = self.transition_status_if_sourced(
            id,
            to,
            from,
            Some("reverted: email-derived status change rejected by the user"),
            EVENT_SOURCE_EMAIL_REJECT,
            true,
        )?;

        let conn = self.conn.lock();
        conn.execute(
            "UPDATE status_events SET confirmed = 1 WHERE rowid = ?1",
            params![rowid],
        )?;
        Ok(reverted)
    }

    /// Connection-scoped status-event append, callable inside a transaction.
    /// Propagates an insert failure (`?`) rather than swallowing it — every
    /// caller runs this inside the SAME transaction as its row write, so an
    /// event-insert error rolls the whole transaction back on drop instead of
    /// leaving a status flip with no history row.
    ///
    /// `source`/`confirmed` are explicit at every call site (never a hidden
    /// default) — see [`StatusEvent::source`]/[`StatusEvent::confirmed`].
    /// `pub(super)`: `super`'s own `upsert_internal`/`import`/`set_status`
    /// call this too — same store, just its row-write half.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn append_event_conn(
        conn: &Connection,
        id: &str,
        from: &str,
        to: &str,
        note: &str,
        source: &str,
        confirmed: bool,
        at: u64,
    ) -> AppResult<()> {
        conn.execute(
            "INSERT INTO status_events (application_id, from_status, to_status, at, note, source, confirmed)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![id, from, to, ts_to_db(at), note, source, i64::from(confirmed)],
        )?;
        Ok(())
    }
}
