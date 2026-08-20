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
    /// SQLite's implicit `rowid` for this row — `status_events` has no
    /// declared primary key, so `rowid` is the only stable per-row identity
    /// available. Stable for the row's lifetime (this table is append-only,
    /// never `VACUUM`ed by any code path in this crate — a `VACUUM` is the
    /// one operation that can renumber rowids). Exists on the wire SPECIFICALLY
    /// so [`ApplicationStore::accept_status_event`]/[`ApplicationStore::
    /// reject_status_event`] can target the EXACT row the user clicked,
    /// never "whichever unconfirmed row happens to be newest" — see those
    /// fns' docs for the bug this closes.
    pub event_id: i64,
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
    /// History for one Application, oldest-first. `rowid` is a tie-break
    /// ONLY (two events landing in the same millisecond still order
    /// deterministically) — `at` remains the primary sort key so the
    /// timeline reads chronologically.
    pub fn events(&self, id: &str) -> Vec<StatusEvent> {
        let conn = self.conn.lock();
        conn.prepare(
            "SELECT rowid, application_id, from_status, to_status, at, note, source, confirmed
             FROM status_events WHERE application_id = ?1 ORDER BY at ASC, rowid ASC",
        )
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map(params![id], |row| {
                Ok(StatusEvent {
                    event_id: row.get(0)?,
                    application_id: row.get(1)?,
                    from_status: row.get(2)?,
                    to_status: row.get(3)?,
                    at: ts_from_db(row.get::<_, i64>(4)?),
                    note: row.get(5)?,
                    source: row.get(6)?,
                    confirmed: row.get::<_, i64>(7)? != 0,
                })
            })
            .ok()
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
    }

    /// Whether the MOST RECENT status_events row for `id` is itself
    /// [`EVENT_SOURCE_EMAIL`] and unconfirmed — i.e. whether `id`'s CURRENT
    /// status (this store's own invariant: the row's `status` always equals
    /// the last event's `to_status`, since every write path appends the
    /// event in the SAME transaction as the row UPDATE) was set by a
    /// still-unreviewed, email-derived write. `crate::email_watch::intent::
    /// next_status` uses this to decide whether a terminal status may be
    /// superseded by a later email — see that fn's doc — instead of
    /// absorbing forever, which is what let one attacker-supplied email
    /// permanently freeze an application's tracking.
    ///
    /// `false` for an application with no history at all (should not
    /// happen — every creation path seeds one event — but this reads total
    /// rather than assume it).
    ///
    /// `ORDER BY at DESC, rowid DESC`: two events landing in the same
    /// millisecond (a fast auto-write immediately followed by another) would
    /// otherwise give SQLite an ARBITRARY winner for the bare `at DESC`
    /// ordering, which could flip whether this reads as "still speculation"
    /// or "settled" — `rowid DESC` breaks the tie deterministically toward
    /// whichever row was actually inserted last.
    pub(crate) fn current_status_is_unconfirmed_email_write(&self, id: &str) -> bool {
        use rusqlite::OptionalExtension;
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT source, confirmed FROM status_events
             WHERE application_id = ?1 ORDER BY at DESC, rowid DESC LIMIT 1",
            params![id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .ok()
        .flatten()
        .is_some_and(|(source, confirmed)| source == EVENT_SOURCE_EMAIL && confirmed == 0)
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

    /// Whether the user has already rejected an email-derived write LANDING
    /// AT `to`, for `id` — i.e. `status_events` already contains a
    /// [`Self::reject_status_event`] reversal row (source
    /// [`EVENT_SOURCE_EMAIL_REJECT`]) whose `from_status` is `to` (a
    /// reversal event's `from_status` always records the ORIGINAL target
    /// being walked back — see [`Self::reject_status_event`]'s doc).
    /// `crate::email_watch::auto_write` consults this before every write so
    /// a later email can't re-apply a target the user has already told us
    /// was wrong for this application.
    ///
    /// **Keyed on `to` ALONE, not the `(from, to)` pair** — deliberately, to
    /// close a detour: keying on the pair let the user reject exactly
    /// `Applied -> Rejected`, and a later `Interview` email (unblocked,
    /// different pair: `Applied -> Interviewing`) followed by a second
    /// `Rejection` email (unblocked, different pair again:
    /// `Interviewing -> Rejected`) land the SAME disputed `Rejected` target
    /// right back, through a path the pair-keyed guard never saw. Once the
    /// user rejects a write landing at `to`, no FUTURE email-derived write —
    /// from any live status — may land there again for this application;
    /// they can always re-set it by hand if a later rejection is genuinely
    /// real. This app would rather auto-write less than silently re-apply a
    /// verdict the user already disputed.
    ///
    /// Matching on [`EVENT_SOURCE_EMAIL_REJECT`] specifically (not a bare
    /// `from_status` scan) means an ordinary, UNRELATED manual status change
    /// that happens to walk back through the same status can never
    /// false-positive this gate — only [`Self::reject_status_event`] ever
    /// writes that source value.
    pub(crate) fn was_transition_rejected(&self, id: &str, to: ApplicationStatus) -> bool {
        use rusqlite::OptionalExtension;
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT 1 FROM status_events
             WHERE application_id = ?1 AND source = ?2 AND from_status = ?3
             LIMIT 1",
            params![id, EVENT_SOURCE_EMAIL_REJECT, to.as_id()],
            |_| Ok(()),
        )
        .optional()
        .unwrap_or(None)
        .is_some()
    }

    /// Accept the SPECIFIC email-derived, unconfirmed status-event row
    /// `event_id` names — clears its `confirmed` flag IN PLACE. Never
    /// touches `applications.status` (the auto-write already applied it —
    /// accepting only marks it reviewed) and never edits `from_status`/
    /// `to_status`/`at`/`note`, so the append-only trail is unchanged; only
    /// the confirmation bit flips.
    ///
    /// **HIGH-1 fix**: `event_id` (the row's [`StatusEvent::event_id`], i.e.
    /// `rowid`) is matched EXACTLY in the `WHERE` clause — this used to
    /// resolve "the newest unconfirmed row for `id`" with no way to name
    /// which one, so two coexisting provisional rows (an ordinary sequence:
    /// a confirmation email, then a later rejection email, both still
    /// unreviewed) meant a click on the OLDER row's Accept button silently
    /// confirmed the NEWER, unrelated one instead — see
    /// `applications::test::accept_targets_the_specific_row_requested_never_whichever_landed_most_recently`.
    /// A single `UPDATE ... WHERE` is now sufficient (no read-then-write): a
    /// wrong/foreign/already-reviewed `event_id` (a different application's
    /// row, a `user`-sourced row, an already-`confirmed` row) matches zero
    /// rows and is a safe no-op, never a cross-application write.
    ///
    /// `Ok(false)`: `event_id` did not resolve to a pending email-derived
    /// row for `id` — a no-op, not an error (a caller need not check first).
    pub fn accept_status_event(&self, id: &str, event_id: i64) -> AppResult<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            "UPDATE status_events SET confirmed = 1
             WHERE rowid = ?1 AND application_id = ?2 AND source = ?3 AND confirmed = 0",
            params![event_id, id, EVENT_SOURCE_EMAIL],
        )?;
        Ok(affected > 0)
    }

    /// Reject the SPECIFIC email-derived, unconfirmed status-event row
    /// `event_id` names: reverts `applications.status` back to THAT event's
    /// `from_status`, but ONLY by compare-and-set — [`Self::
    /// transition_status_if_sourced`] (reused, not reimplemented) only
    /// succeeds if the CURRENT status still equals the event's `to_status`.
    /// **A status that has since moved on — by the user's own hand, or by a
    /// LATER email write — is never clobbered**: when the CAS loses, the
    /// row is simply DISMISSED (its `confirmed` flag cleared below) rather
    /// than reverted.
    ///
    /// **HIGH-1 fix**: `event_id` (the row's [`StatusEvent::event_id`], i.e.
    /// `rowid`) is matched EXACTLY in the initial `SELECT` — this used to
    /// resolve "the newest unconfirmed row for `id`" with no way to name
    /// which one, so two coexisting provisional rows meant a click on the
    /// OLDER row's Reject button silently dismissed-or-reverted the NEWER,
    /// unrelated one instead — see `applications::test::
    /// reject_targets_the_specific_row_requested_never_whichever_landed_most_recently`.
    /// A wrong/foreign/already-reviewed `event_id` matches zero rows in that
    /// initial `SELECT` and is a safe no-op.
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
    /// CAS/dismissal, or when `event_id` did not resolve to a pending
    /// email-derived row for `id` at all — both are legitimate no-ops, not
    /// errors).
    pub fn reject_status_event(&self, id: &str, event_id: i64) -> AppResult<bool> {
        use rusqlite::OptionalExtension;
        let pending: Option<(String, String)> = {
            let conn = self.conn.lock();
            conn.query_row(
                "SELECT from_status, to_status FROM status_events
                 WHERE rowid = ?1 AND application_id = ?2 AND source = ?3 AND confirmed = 0",
                params![event_id, id, EVENT_SOURCE_EMAIL],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
        };
        let Some((from_status, to_status)) = pending else {
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
            params![event_id],
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

#[cfg(test)]
mod tests {
    use super::*;

    // Pins these `pub(crate)` constants against hand-written literals — the TS
    // mirror (`packages/shared/src/types/index.ts`, adjacent to `StatusEvent`)
    // pins its own exported `EVENT_SOURCE_*` values against these SAME
    // literals, since `pub(crate)` here means the renderer cannot import this
    // constant directly and would otherwise silently re-declare its own copy
    // (which is exactly what had happened before that TS-side fix). This value
    // crosses that boundary with nothing else checking it: a rename on either
    // side makes `isProvisionalEvent` (the renderer's `source`/`confirmed`
    // check) stop matching, so a provisional email-derived row renders as
    // settled history and the Accept/Reject affordance silently disappears —
    // adjudication is the entire safety model that justifies auto-write
    // defaulting ON, so losing it without a failing test is the worst
    // available outcome. Compared against literals, not against each other or
    // a derived value, so renaming both constants in lockstep still fails.
    #[test]
    fn event_source_user_is_user() {
        assert_eq!(EVENT_SOURCE_USER, "user");
    }

    #[test]
    fn event_source_email_is_email() {
        assert_eq!(EVENT_SOURCE_EMAIL, "email");
    }

    #[test]
    fn event_source_email_reject_is_email_reject() {
        assert_eq!(EVENT_SOURCE_EMAIL_REJECT, "email_reject");
    }
}
