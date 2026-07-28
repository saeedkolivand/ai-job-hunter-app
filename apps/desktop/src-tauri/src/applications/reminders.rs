//! Follow-up reminder bookkeeping — the data half of
//! [`crate::reminder_scheduler`].
//!
//! Two operations, deliberately kept together: the sweep's READ
//! ([`super::ApplicationStore::follow_up_candidates`]) and the atomic CLAIM that
//! decides whether a candidate may actually be announced
//! ([`super::ApplicationStore::mark_next_action_notified`]). Everything the read
//! decided is re-asserted by the claim's `WHERE`, so the two must be read
//! together to see that the window between them is closed.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8), same as the `contact`/`job_url`/`migrations`
//! siblings. No behaviour lives anywhere else.

use rusqlite::params;

use super::{ApplicationStatus, ApplicationStore};
use crate::db::{ts_from_db, ts_to_db};
use crate::error::AppResult;

/// One Application's follow-up reminder state — the minimal projection
/// [`crate::reminder_scheduler`] sweeps. Not a wire type: it is a query
/// projection, and the scheduler is its only consumer.
#[derive(Debug, Clone)]
pub struct FollowUpCandidate {
    pub id: String,
    pub status: ApplicationStatus,
    pub title: String,
    pub company: String,
    /// The user-set reminder timestamp (ms). Always `Some` as returned by
    /// [`ApplicationStore::follow_up_candidates`] (the query filters NULLs), but
    /// kept optional so the decision function owns the "unset" case explicitly.
    pub next_action_at: Option<u64>,
    /// When a notification was already raised for the CURRENT `next_action_at`
    /// (ms). `None` = not yet announced; cleared whenever the due date changes.
    pub notified_at: Option<u64>,
}

/// The terminal status ids as a SQL `IN` list (`'accepted','rejected',…`),
/// derived from [`ApplicationStatus::is_terminal`] so a new terminal stage can
/// never be forgotten in SQL. Every id is a fixed ASCII literal from
/// [`ApplicationStatus::as_id`] — no user input reaches this string.
fn terminal_status_sql_list() -> String {
    ApplicationStatus::ALL
        .iter()
        .filter(|s| s.is_terminal())
        .map(|s| format!("'{}'", s.as_id()))
        .collect::<Vec<_>>()
        .join(",")
}

impl ApplicationStore {
    /// Every Application carrying a follow-up reminder, with the dedupe marker
    /// the scheduler needs — the read side of [`crate::reminder_scheduler`].
    ///
    /// Rows without a `next_action_at` are filtered out in SQL (nothing to
    /// remind about), so the sweep stays cheap on a large tracker.
    ///
    /// A SQL failure yields an empty sweep — but it is LOGGED, never silent: a
    /// swallowed error here would kill every reminder for the rest of the
    /// process with no diagnostic anywhere.
    pub fn follow_up_candidates(&self) -> Vec<FollowUpCandidate> {
        let conn = self.conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT id, status, title, company, next_action_at, next_action_notified_at
             FROM applications WHERE next_action_at IS NOT NULL",
        ) {
            Ok(stmt) => stmt,
            Err(e) => {
                log::warn!("[applications] follow-up query failed to prepare: {e}");
                return Vec::new();
            }
        };
        let rows = stmt.query_map([], |row| {
            let status_raw: String = row.get(1)?;
            Ok(FollowUpCandidate {
                id: row.get(0)?,
                status: ApplicationStatus::from_id(&status_raw),
                title: row.get(2)?,
                company: row.get(3)?,
                next_action_at: row.get::<_, Option<i64>>(4)?.map(ts_from_db),
                notified_at: row.get::<_, Option<i64>>(5)?.map(ts_from_db),
            })
        });
        match rows {
            Ok(rows) => rows
                .filter_map(|r| {
                    r.inspect_err(|e| log::warn!("[applications] skipped a follow-up row: {e}"))
                        .ok()
                })
                .collect(),
            Err(e) => {
                log::warn!("[applications] follow-up query failed: {e}");
                Vec::new()
            }
        }
    }

    /// Atomically CLAIM the right to notify about `id`'s follow-up: stamp "a
    /// notification was raised for the due date `due_at`" and report whether
    /// this call is the one that won. Cleared by
    /// [`ApplicationStore::update_fields`] whenever the due date moves.
    ///
    /// Every precondition the scheduler evaluated on its (separately-locked)
    /// read lives in this ONE statement's `WHERE`, so anything the user changes
    /// in between makes the claim fail instead of notifying on stale state:
    ///
    /// - `next_action_at = ?2` — the user rescheduled. An unconditional stamp
    ///   would mark the row notified for a due date the caller never evaluated,
    ///   silencing the NEW reminder forever.
    /// - `status NOT IN (<terminal>)` — the user rejected/accepted/withdrew the
    ///   pursuit after the read. Re-checking `is_terminal` in Rust could not fix
    ///   this: any check outside this statement has the same window it is trying
    ///   to close. The list is DERIVED from [`ApplicationStatus::is_terminal`]
    ///   (see [`terminal_status_sql_list`]) so the two cannot drift.
    ///
    /// Returns `Ok(true)` iff exactly one row matched. The caller MUST NOT
    /// notify on `Ok(false)`.
    ///
    /// Deliberately a targeted single-column `UPDATE` rather than a row rewrite:
    /// the scheduler must never clobber a field the user edited between the read
    /// and this write.
    pub fn mark_next_action_notified(&self, id: &str, due_at: u64, at: u64) -> AppResult<bool> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            &format!(
                "UPDATE applications SET next_action_notified_at = ?3
                 WHERE id = ?1 AND next_action_at = ?2
                   AND status NOT IN ({})",
                terminal_status_sql_list()
            ),
            params![id, ts_to_db(due_at), ts_to_db(at)],
        )?;
        // `id` is the PRIMARY KEY, so a match is always exactly one row; assert
        // that explicitly rather than accepting any non-zero count.
        Ok(changed == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sql_terminal_list_is_derived_from_is_terminal() {
        // Drift guard: the SQL predicate must name EXACTLY the statuses
        // `is_terminal` reports, so adding a terminal stage cannot leave the
        // claim's WHERE behind. `ghosted` is soft-terminal and must stay out.
        let list = terminal_status_sql_list();
        for status in ApplicationStatus::ALL {
            let quoted = format!("'{}'", status.as_id());
            assert_eq!(
                list.contains(&quoted),
                status.is_terminal(),
                "{status:?} membership in the SQL list must follow is_terminal()"
            );
        }
        assert_eq!(list, "'accepted','rejected','withdrawn'");
    }
}
