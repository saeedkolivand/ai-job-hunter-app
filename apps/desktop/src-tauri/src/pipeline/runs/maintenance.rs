//! Retention and deletion: [`PipelineRunStore::prune`] (per-`(job_url, kind)`
//! retention), the factory-reset wipe, and the per-posting/bulk deletes that
//! back the Documents page's cascade.

use rusqlite::params;

use crate::observability::sanitize_reason;

use super::model::RETENTION_RUNS_PER_JOB;
use super::store::{normalized_job_url, PipelineRunStore};

impl PipelineRunStore {
    /// Keep only the newest [`RETENTION_RUNS_PER_JOB`] runs per
    /// `(job_url, kind)`, deleting the evicted runs' events with them.
    ///
    /// Called from the ADR-019 performance-tier hook
    /// (`commands::system::system_set_performance_mode`) alongside the cache
    /// prunes, because that is the app's one "reclaim disk now" moment. It
    /// deliberately ignores the tier's `cacheTtlSecs`/`cacheMaxRows` knobs: a
    /// run trail is USER HISTORY, not a cache, so the low-memory tier must not
    /// be able to silently delete the run a user is still looking at. The bound
    /// is the fixed per-`(job_url, kind)` count instead.
    ///
    /// Best-effort and transactional: the statements are SEQUENCED and the first
    /// failure returns WITHOUT committing, so the `Transaction`'s drop rolls the
    /// whole thing back and the tables are left exactly as they were, never
    /// half-pruned. (Evaluating all three results before matching on them would
    /// commit the partial work — the arm claiming "leaving history intact" would
    /// have run after the commit that did not.)
    ///
    /// Reports through [`crate::observability::Span`], not a bare `log::info!`.
    /// That is not style: `log::info!`'s implicit target is the module it is
    /// WRITTEN in (`…::pipeline::runs`), and `lib.rs`'s log filter is global
    /// `Warn` with a `level_for` exception for `…::observability` only — so an
    /// info line written here would never reach the log file. Every `Span`
    /// begin/end logs FROM that one module, which is exactly why the exception
    /// covers all of them. Counts only; no run ids, no artifacts (ADR-027).
    pub fn prune(&self) {
        let span = crate::observability::Span::begin("pipeline:runs", "op=prune");
        let mut guard = self.conn.lock();
        let tx = match guard.transaction() {
            Ok(tx) => tx,
            Err(e) => {
                log::warn!(
                    "[pipeline] run-store prune could not open a transaction: {}",
                    sanitize_reason(&e.to_string())
                );
                span.end(false);
                return;
            }
        };
        // Rank each run within its own (job_url, kind) newest-first and delete
        // past N. One statement, so a job with thousands of runs is still one
        // pass. `kind` is in the partition because these tables host every
        // staged run: without it, three résumé runs would evict the same
        // posting's agent-run history.
        let evicted = match tx.execute(
            "DELETE FROM pipeline_runs WHERE id IN (
                 SELECT id FROM (
                     SELECT id, ROW_NUMBER() OVER (
                         PARTITION BY job_url, kind ORDER BY started_at DESC, id DESC
                     ) AS rn
                     FROM pipeline_runs
                 ) WHERE rn > ?1
             )",
            params![RETENTION_RUNS_PER_JOB as i64],
        ) {
            Ok(runs) => runs,
            Err(e) => {
                // Return WITHOUT committing: dropping `tx` rolls back.
                log::warn!(
                    "[pipeline] run-store prune failed, leaving history intact: {}",
                    sanitize_reason(&e.to_string())
                );
                span.end(false);
                return;
            }
        };
        // Events are keyed by run_id with no FK (SQLite leaves those off by
        // default), so the orphan sweep is explicit. Written as "no matching
        // run" rather than "the ids we just deleted" so it also collects rows
        // orphaned by any earlier partial delete. NOT EXISTS rather than
        // `NOT IN`: `NOT IN` against a subquery containing a NULL is never TRUE
        // for any row, so a single null-id run would silently disable the sweep
        // forever (the schema's `NOT NULL` is the first line of that defense).
        let orphans = match tx.execute(
            "DELETE FROM pipeline_run_events
             WHERE NOT EXISTS (
                 SELECT 1 FROM pipeline_runs r WHERE r.id = pipeline_run_events.run_id
             )",
            [],
        ) {
            Ok(events) => events,
            Err(e) => {
                log::warn!(
                    "[pipeline] run-store orphan sweep failed, leaving history intact: {}",
                    sanitize_reason(&e.to_string())
                );
                span.end(false);
                return;
            }
        };
        match tx.commit() {
            Ok(()) => span.end_with(&format!("runs={evicted} events={orphans}"), true),
            Err(e) => {
                log::warn!(
                    "[pipeline] run-store prune could not commit: {}",
                    sanitize_reason(&e.to_string())
                );
                span.end(false);
            }
        }
    }

    /// Wipe every run and event (factory reset).
    ///
    /// Infallible BY SIGNATURE — `Resettable::reset` returns `()`, and the whole
    /// reset sweep continues past any one store — but never SILENT: a discarded
    /// `Err` here would report a privacy wipe as done while the rows the user
    /// asked to be gone are still on disk. Each failure is logged at `warn`
    /// naming the table, which is also the only level that survives: `lib.rs`'s
    /// global filter is `Warn`, with an exception for `…::observability` only, so
    /// an `info!` written from THIS module would never reach the log file.
    ///
    /// The two DELETEs are independent on purpose: the second must still run when
    /// the first fails, because a partial wipe beats no wipe when the goal is
    /// removing user data.
    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        if let Err(e) = conn.execute("DELETE FROM pipeline_run_events", []) {
            log::warn!(
                "[pipeline] factory reset failed to clear pipeline_run_events: {}",
                sanitize_reason(&e.to_string())
            );
        }
        if let Err(e) = conn.execute("DELETE FROM pipeline_runs", []) {
            log::warn!(
                "[pipeline] factory reset failed to clear pipeline_runs: {}",
                sanitize_reason(&e.to_string())
            );
        }
    }

    /// Delete every run of ONE posting, and its events with it. Returns how
    /// many RUNS went.
    ///
    /// **Deleting a posting has to reach this table.** A max-depth run (the
    /// `max` generation depth is gone, but an EXISTING run row from before its
    /// removal can still be sitting here, and there is no migration touching
    /// old rows) persisted its FULL `strategy` (the whole employment history)
    /// and its full `match_evidence` map (verbatim quotes out of the
    /// candidate's résumé) into `pipeline_run_events.artifact_json` — a
    /// deliberate DB decision at the time (ADR-027 governs only the LOG). But
    /// nothing else ever removes those rows for a posting the user deleted:
    /// [`prune`](Self::prune) partitions by `(job_url, kind)` and only evicts
    /// the FOURTH run of a posting that is still being run, and
    /// [`clear_all`](Self::clear_all) is the factory reset. So "delete this"
    /// left employment history and résumé quotes on disk indefinitely — and
    /// `export` ships every event row into the user's backups.
    ///
    /// An INDEXED delete over the normalized key, because
    /// [`super::store::normalized_job_url`] runs at every write site: both sides of the
    /// comparison are already in the same spelling, so this is
    /// `idx_pipeline_runs_job` rather than the full-scan-and-normalize-in-Rust
    /// it had to be while writers stored the raw url.
    ///
    /// Best-effort and transactional, like [`prune`](Self::prune): a failure
    /// returns without committing, so the trail is never half-deleted.
    pub fn delete_for_job(&self, job_url: &str) -> usize {
        self.delete_for_jobs(&[job_url.to_string()])
    }

    /// [`delete_for_job`](Self::delete_for_job) for several postings, in ONE
    /// transaction. Returns how many RUNS went.
    ///
    /// One transaction, not one per posting: the bulk cascade behind
    /// `ai_generations_remove_bulk` is a single user action, and N independent
    /// transactions leave it half-applied when the third of five fails —
    /// exactly the state the single-posting path already refuses to produce.
    /// The single case delegates here, so there is one delete rather than two
    /// that can drift.
    pub fn delete_for_jobs(&self, job_urls: &[String]) -> usize {
        // An unlinked run (a manual entry with no posting url) has no trail to
        // find, and `""` is what every one of them is stored under — matching
        // it would delete other postings' history.
        let wanted: Vec<String> = job_urls
            .iter()
            .map(|url| normalized_job_url(url))
            .filter(|url| !url.is_empty())
            .collect();
        if wanted.is_empty() {
            return 0;
        }
        let span = crate::observability::Span::begin("pipeline:runs", "op=delete_for_jobs");
        let mut guard = self.conn.lock();
        let tx = match guard.transaction() {
            Ok(tx) => tx,
            Err(e) => {
                log::warn!(
                    "[pipeline] could not open a transaction to delete a job's runs: {}",
                    sanitize_reason(&e.to_string())
                );
                span.end(false);
                return 0;
            }
        };
        // CHUNKED, inside the one transaction: the selection is the user's and
        // an unbounded `IN (?, …)` fails to prepare past
        // [`crate::db::MAX_SQL_PARAMS`]. Batching keeps the delete atomic — every chunk
        // commits together or none does.
        let mut removed = 0usize;
        for chunk in wanted.chunks(crate::db::MAX_SQL_PARAMS) {
            let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            // Events first: an interrupted delete that took the run rows and
            // left their events would leave rows the orphan sweep only reaches
            // on the next `prune`.
            if let Err(e) = tx.execute(
                &format!(
                    "DELETE FROM pipeline_run_events WHERE run_id IN
                         (SELECT id FROM pipeline_runs WHERE job_url IN ({placeholders}))"
                ),
                rusqlite::params_from_iter(chunk.iter()),
            ) {
                log::warn!(
                    "[pipeline] could not delete a job's run events: {}",
                    sanitize_reason(&e.to_string())
                );
                span.end(false);
                return 0; // dropping `tx` rolls the whole thing back
            }
            match tx.execute(
                &format!("DELETE FROM pipeline_runs WHERE job_url IN ({placeholders})"),
                rusqlite::params_from_iter(chunk.iter()),
            ) {
                Ok(rows) => removed += rows,
                Err(e) => {
                    log::warn!(
                        "[pipeline] could not delete a job's run rows: {}",
                        sanitize_reason(&e.to_string())
                    );
                    span.end(false);
                    return 0;
                }
            }
        }
        match tx.commit() {
            Ok(()) => {
                span.end_with(&format!("jobs={} runs={removed}", wanted.len()), true);
                removed
            }
            Err(e) => {
                log::warn!(
                    "[pipeline] could not commit a job's run deletion: {}",
                    sanitize_reason(&e.to_string())
                );
                span.end(false);
                0
            }
        }
    }

    /// Delete one run's events, leaving no row behind. Returns how many went.
    ///
    /// For the ONE case where a run has no row to hang them off: its posting was
    /// deleted while it was still in flight, so `delete_for_job` took the row
    /// and every event that existed at that moment — and the run then kept
    /// emitting into a `run_id` nothing points at. `prune`'s orphan sweep would
    /// collect them eventually, but "eventually, when some other posting's run
    /// finishes" is not a schedule a purge the user just asked for may run on.
    pub fn delete_events_for_run(&self, run_id: &str) -> usize {
        let conn = self.conn.lock();
        match conn.execute(
            "DELETE FROM pipeline_run_events WHERE run_id = ?1",
            params![run_id],
        ) {
            Ok(rows) => rows,
            Err(e) => {
                log::warn!(
                    "[pipeline] could not sweep an abandoned run's events: {}",
                    sanitize_reason(&e.to_string())
                );
                0
            }
        }
    }
}
