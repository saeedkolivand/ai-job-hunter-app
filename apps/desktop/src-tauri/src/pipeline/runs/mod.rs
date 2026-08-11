//! The pipeline run store — what a multi-step run DID, on disk.
//!
//! Two tables in their own `<dataDir>/pipeline_runs.db` (opened via
//! [`crate::db::open`], so WAL + busy_timeout per ADR-022):
//!
//! * `pipeline_runs` — one row per run: identity, what it was run against, how
//!   it ended, and a free-form but CLAMPED `metrics_json` blob (see
//!   [`METRICS_CAP_BYTES`]).
//! * `pipeline_run_events` — the ordered per-stage trail of one run, each event
//!   carrying a CLAMPED `artifact_json` (see [`ARTIFACT_CAP_BYTES`]).
//!
//! **Both free-form JSON columns are capped on BOTH paths** — at the write site
//! and again on import — so neither a runaway stage nor a hand-edited backup can
//! persist a multi-megabyte `metrics_json`/`artifact_json` value. The other
//! columns (id/job_url/kind/depth/status/stage/phase) are NOT clamped by
//! `import()` — a hand-edited backup can still restore an unbounded value
//! there.
//!
//! **`kind` is the discriminator, not the table name.** This store is also the
//! future home of agent runs: an agent run and a résumé-pipeline run have the
//! same shape (a budgeted, cancellable, staged run against one job), so they
//! share the tables and differ by `kind`. A second near-identical store is the
//! drift this codebase keeps re-discovering.
//!
//! **Retention is newest-N per `(job_url, kind)`**, not a global cap — see
//! [`PipelineRunStore::prune`].
//!
//! Wired like every other durable store: [`crate::data_store::DataStore`] for
//! backup/restore, `Resettable` for the factory reset (registered in
//! `commands::privacy`), and position-indexed APPEND-ONLY migrations. Tauri-free
//! (L2, same posture as [`crate::pipeline::cache`]) — the shell resolves it from
//! managed state and calls in.

use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::data_store::DataStore;
use crate::db::{run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::{AppError, AppResult};

/// Byte cap on ONE event's `artifact_json`, MARKER INCLUDED.
///
/// An artifact is a stage SUMMARY — counts, section keys, a stopped reason —
/// never the generated document, so 16 KiB is roughly two orders of magnitude
/// more than any honest payload needs. The cap exists for the dishonest one: a
/// stage that accidentally hands its whole model output to the recorder would
/// otherwise write a multi-megabyte row per stage, for every stage, forever.
/// Oversized values are TRUNCATED (never dropped) so the trail still shows the
/// stage ran — see [`clamp_artifact`], which cuts on a UTF-8 boundary and
/// appends [`TRUNCATION_MARKER`] so a reader can tell.
///
/// The marker is charged AGAINST the cap, not added on top of it: a cap that a
/// clamped value can exceed is not a cap.
pub const ARTIFACT_CAP_BYTES: usize = 16 * 1024;

/// Byte cap on ONE run's `metrics_json`, MARKER INCLUDED — the run-level twin
/// of [`ARTIFACT_CAP_BYTES`].
///
/// Smaller because the payload is smaller: run metrics are counts and durations
/// (token totals, per-stage milliseconds), never text. 4 KiB fits hundreds of
/// numeric fields while still bounding a caller that hands the recorder
/// something it should not have — and, on the import path, a hand-edited backup
/// that would otherwise restore a multi-megabyte row permanently.
pub const METRICS_CAP_BYTES: usize = 4 * 1024;

/// Appended to a clamped JSON column so truncation is visible rather than
/// silent. Deliberately not valid JSON: a truncated value is NOT a parseable
/// value, and a reader that tries must fail rather than read half an object as a
/// whole one.
pub const TRUNCATION_MARKER: &str = "…[truncated]";

// A cap smaller than the marker it reserves room for would underflow the clamp's
// body budget. Compile-time, so shrinking a cap past its marker fails the BUILD.
const _: () = assert!(ARTIFACT_CAP_BYTES > TRUNCATION_MARKER.len());
const _: () = assert!(METRICS_CAP_BYTES > TRUNCATION_MARKER.len());

/// Runs kept per `(job_url, kind)`. Three is "the current one plus the two you
/// might want to compare it against": run history is a debugging aid, and the
/// fourth attempt at the same posting has never been the interesting one.
///
/// The partition is per posting AND per [`RunRow::kind`], not global: a user who
/// runs one job repeatedly cannot evict every other job's history, and — since
/// these tables host every staged run — three résumé runs cannot evict the same
/// posting's agent-run trail. `kind` is this module's first-class discriminator,
/// so it discriminates retention too.
pub const RETENTION_RUNS_PER_JOB: usize = 3;

/// One recorded run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRow {
    pub id: String,
    /// The posting this run was for — half of the retention key, the other half
    /// being [`Self::kind`] (see [`PipelineRunStore::prune`]).
    pub job_url: String,
    /// Which kind of run this is (`"resume"`, `"agent"`, …). The discriminator
    /// that lets one pair of tables host every staged run.
    pub kind: String,
    /// The flow's depth/profile label (e.g. `"brief"`/`"full"`), free-form.
    pub depth: String,
    /// `"running"` until a terminal update lands.
    pub status: String,
    pub started_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<u64>,
    /// The wire form of [`crate::pipeline::budget::StoppedReason`], stored as
    /// TEXT rather than an enum column so a variant added later (Phase 3 adds
    /// three) needs no migration and an older bundle still restores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stopped_reason: Option<String>,
    /// Free-form run metrics as a JSON object string (token counts, durations),
    /// clamped to [`METRICS_CAP_BYTES`] at every write.
    #[serde(default = "empty_json_object")]
    pub metrics_json: String,
}

/// One stage event within a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEventRow {
    pub run_id: String,
    /// Monotonic per-run ordinal. Part of the primary key, so a replayed event
    /// overwrites rather than duplicating.
    pub seq: u32,
    pub ts: u64,
    pub stage: String,
    /// `"start"`/`"finish"`/… — the stage lifecycle phase.
    pub phase: String,
    /// The stage's summary payload, already clamped to [`ARTIFACT_CAP_BYTES`].
    pub artifact_json: String,
}

fn empty_json_object() -> String {
    "{}".to_string()
}

/// Clamp one free-form JSON column to `cap` BYTES INCLUSIVE of the truncation
/// marker, cutting on a UTF-8 character boundary and marking the cut.
///
/// Byte-based (not char-based) because the cap protects the DB file, and a
/// char-based cap on multi-byte text bounds nothing useful — a 16k-char CJK
/// artifact is 48 KB. Pure, so the boundary arithmetic is directly testable.
///
/// The marker's own length is RESERVED out of the cap, so the returned string is
/// never longer than `cap`. Both call sites pass a cap larger than the marker
/// (asserted at compile time above), so the subtraction cannot underflow.
fn clamp_json(value: &str, cap: usize) -> String {
    if value.len() <= cap {
        return value.to_string();
    }
    // Walk back to the last char boundary at or below the body budget so the
    // result is always valid UTF-8 (`String` cannot hold anything else).
    let mut end = cap - TRUNCATION_MARKER.len();
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{TRUNCATION_MARKER}", &value[..end])
}

/// Clamp one event's `artifact_json` to [`ARTIFACT_CAP_BYTES`].
pub fn clamp_artifact(artifact: &str) -> String {
    clamp_json(artifact, ARTIFACT_CAP_BYTES)
}

/// Clamp one run's `metrics_json` to [`METRICS_CAP_BYTES`].
pub fn clamp_metrics(metrics: &str) -> String {
    clamp_json(metrics, METRICS_CAP_BYTES)
}

pub struct PipelineRunStore {
    conn: Mutex<Connection>,
}

impl PipelineRunStore {
    /// POSITION-INDEXED migrations: `db::run_migrations` gates each entry on its
    /// INDEX via `PRAGMA user_version`, so this list is APPEND-ONLY. Editing or
    /// reordering an existing entry silently skips it on every already-migrated
    /// install.
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_pipeline_runs",
        up: |conn| {
            conn.execute_batch(
                // `id TEXT PRIMARY KEY` alone is NOT enough: SQLite permits NULL
                // in a TEXT PRIMARY KEY (the historical INTEGER-PRIMARY-KEY
                // exemption, kept for backwards compatibility), and ONE null-id
                // row turns any `NOT IN (SELECT id FROM pipeline_runs)` sweep
                // into a permanent silent no-op. `NOT NULL` closes that hole at
                // the schema; `Self::prune`'s NOT EXISTS closes it again in SQL.
                "CREATE TABLE IF NOT EXISTS pipeline_runs (
                    id             TEXT PRIMARY KEY NOT NULL,
                    job_url        TEXT NOT NULL,
                    kind           TEXT NOT NULL,
                    depth          TEXT NOT NULL,
                    status         TEXT NOT NULL,
                    started_at     INTEGER NOT NULL,
                    finished_at    INTEGER,
                    stopped_reason TEXT,
                    metrics_json   TEXT NOT NULL DEFAULT '{}'
                );
                 CREATE INDEX IF NOT EXISTS idx_pipeline_runs_job
                     ON pipeline_runs(job_url, started_at DESC);
                 CREATE TABLE IF NOT EXISTS pipeline_run_events (
                    run_id        TEXT NOT NULL,
                    seq           INTEGER NOT NULL,
                    ts            INTEGER NOT NULL,
                    stage         TEXT NOT NULL,
                    phase         TEXT NOT NULL,
                    artifact_json TEXT NOT NULL,
                    PRIMARY KEY (run_id, seq)
                 );",
            )
        },
    }];

    pub fn open(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("pipeline_runs.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Insert (or replace) a run row. Replace semantics so a terminal update is
    /// the same call as the initial insert — one code path, and a crash between
    /// the two leaves a `running` row rather than nothing.
    ///
    /// `metrics_json` is clamped HERE — at the single write site — exactly as
    /// [`append_event`](Self::append_event) clamps `artifact_json`, so no caller
    /// can bypass [`METRICS_CAP_BYTES`] by forgetting to.
    pub fn upsert_run(&self, run: &RunRow) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO pipeline_runs
                (id, job_url, kind, depth, status, started_at, finished_at,
                 stopped_reason, metrics_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.id,
                run.job_url,
                run.kind,
                run.depth,
                run.status,
                ts_to_db(run.started_at),
                run.finished_at.map(ts_to_db),
                run.stopped_reason,
                clamp_metrics(&run.metrics_json),
            ],
        )
        .map_err(|e| AppError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Append one stage event. `artifact_json` is clamped HERE — at the single
    /// write site — so no caller can bypass the cap by forgetting to.
    pub fn append_event(&self, event: &RunEventRow) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO pipeline_run_events
                (run_id, seq, ts, stage, phase, artifact_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.run_id,
                event.seq,
                ts_to_db(event.ts),
                event.stage,
                event.phase,
                clamp_artifact(&event.artifact_json),
            ],
        )
        .map_err(|e| AppError::Storage(e.to_string()))?;
        Ok(())
    }

    /// One run by id.
    pub fn run(&self, id: &str) -> Option<RunRow> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, job_url, kind, depth, status, started_at, finished_at,
                    stopped_reason, metrics_json
             FROM pipeline_runs WHERE id = ?1",
            params![id],
            row_to_run,
        )
        .ok()
    }

    /// Runs for one posting, newest first.
    pub fn runs_for_job(&self, job_url: &str) -> Vec<RunRow> {
        let conn = self.conn.lock();
        query_runs(
            &conn,
            "SELECT id, job_url, kind, depth, status, started_at, finished_at,
                    stopped_reason, metrics_json
             FROM pipeline_runs WHERE job_url = ?1 ORDER BY started_at DESC, id DESC",
            params![job_url],
        )
    }

    /// One run's events in `seq` order.
    pub fn events_for_run(&self, run_id: &str) -> Vec<RunEventRow> {
        let conn = self.conn.lock();
        query_events(
            &conn,
            "SELECT run_id, seq, ts, stage, phase, artifact_json
             FROM pipeline_run_events WHERE run_id = ?1 ORDER BY seq",
            params![run_id],
        )
    }

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
                log::warn!("[pipeline] run-store prune could not open a transaction: {e}");
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
                log::warn!("[pipeline] run-store prune failed, leaving history intact: {e}");
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
                log::warn!("[pipeline] run-store orphan sweep failed, leaving history intact: {e}");
                span.end(false);
                return;
            }
        };
        match tx.commit() {
            Ok(()) => span.end_with(&format!("runs={evicted} events={orphans}"), true),
            Err(e) => {
                log::warn!("[pipeline] run-store prune could not commit: {e}");
                span.end(false);
            }
        }
    }

    /// Wipe every run and event (factory reset).
    pub fn clear_all(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute("DELETE FROM pipeline_run_events", []);
        let _ = conn.execute("DELETE FROM pipeline_runs", []);
    }

    /// Every run, oldest first — a deterministic order for export.
    fn all_runs(&self) -> Vec<RunRow> {
        let conn = self.conn.lock();
        query_runs(
            &conn,
            "SELECT id, job_url, kind, depth, status, started_at, finished_at,
                    stopped_reason, metrics_json
             FROM pipeline_runs ORDER BY started_at, id",
            params![],
        )
    }

    /// Every event, in `(run_id, seq)` order.
    fn all_events(&self) -> Vec<RunEventRow> {
        let conn = self.conn.lock();
        query_events(
            &conn,
            "SELECT run_id, seq, ts, stage, phase, artifact_json
             FROM pipeline_run_events ORDER BY run_id, seq",
            params![],
        )
    }
}

/// The export/restore section: `{ "runs": [...], "events": [...] }`.
///
/// An OBJECT of two arrays rather than nested events-inside-runs, because the
/// two tables restore independently and a nested shape would make an event
/// whose run failed to deserialize silently vanish with it.
#[derive(Debug, Default, Serialize, Deserialize)]
struct RunsBundle {
    #[serde(default)]
    runs: Vec<RunRow>,
    #[serde(default)]
    events: Vec<RunEventRow>,
}

impl DataStore for PipelineRunStore {
    fn key(&self) -> &'static str {
        "pipelineRuns"
    }

    fn export(&self) -> serde_json::Value {
        serde_json::json!(RunsBundle {
            runs: self.all_runs(),
            events: self.all_events(),
        })
    }

    fn import(&self, data: &serde_json::Value) -> AppResult<usize> {
        // Deserialize EVERYTHING before mutating, so a malformed row aborts the
        // import without having cleared the tables (mirrors the other stores).
        let bundle: RunsBundle = serde_json::from_value(data.clone())
            .map_err(|e| AppError::Validation(format!("pipelineRuns: {e}")))?;

        let mut guard = self.conn.lock();
        let tx = guard.transaction()?;
        tx.execute("DELETE FROM pipeline_run_events", [])?;
        tx.execute("DELETE FROM pipeline_runs", [])?;
        for run in &bundle.runs {
            tx.execute(
                "INSERT OR REPLACE INTO pipeline_runs
                    (id, job_url, kind, depth, status, started_at, finished_at,
                     stopped_reason, metrics_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    run.id,
                    run.job_url,
                    run.kind,
                    run.depth,
                    run.status,
                    ts_to_db(run.started_at),
                    run.finished_at.map(ts_to_db),
                    run.stopped_reason,
                    // Re-clamp on import, exactly like `artifact_json` below: a
                    // hand-edited bundle must not be able to restore a row the
                    // live write path could never have produced.
                    clamp_metrics(&run.metrics_json),
                ],
            )?;
        }
        for event in &bundle.events {
            tx.execute(
                "INSERT OR REPLACE INTO pipeline_run_events
                    (run_id, seq, ts, stage, phase, artifact_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    event.run_id,
                    event.seq,
                    ts_to_db(event.ts),
                    event.stage,
                    event.phase,
                    // Re-clamp on import: a hand-edited or legacy bundle must
                    // not be able to write past the cap the live path enforces.
                    clamp_artifact(&event.artifact_json),
                ],
            )?;
        }
        tx.commit()?;
        Ok(bundle.runs.len())
    }
}

fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunRow> {
    Ok(RunRow {
        id: row.get(0)?,
        job_url: row.get(1)?,
        kind: row.get(2)?,
        depth: row.get(3)?,
        status: row.get(4)?,
        started_at: ts_from_db(row.get::<_, i64>(5)?),
        finished_at: row.get::<_, Option<i64>>(6)?.map(ts_from_db),
        stopped_reason: row.get(7)?,
        metrics_json: row.get(8)?,
    })
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunEventRow> {
    Ok(RunEventRow {
        run_id: row.get(0)?,
        seq: row.get::<_, i64>(1)? as u32,
        ts: ts_from_db(row.get::<_, i64>(2)?),
        stage: row.get(3)?,
        phase: row.get(4)?,
        artifact_json: row.get(5)?,
    })
}

/// Run a prepared SELECT and collect the rows, degrading to an empty Vec on a
/// read failure — a run trail is a debugging aid, so a transient read error
/// must never take a caller down. Logged, because a silent empty history is
/// indistinguishable from "there were no runs".
fn query_runs(conn: &Connection, sql: &str, args: impl rusqlite::Params) -> Vec<RunRow> {
    match conn.prepare(sql).and_then(|mut stmt| {
        stmt.query_map(args, row_to_run)
            .map(|r| r.flatten().collect())
    }) {
        Ok(rows) => rows,
        Err(e) => {
            log::warn!("[pipeline] run-store read failed, reporting no runs: {e}");
            Vec::new()
        }
    }
}

fn query_events(conn: &Connection, sql: &str, args: impl rusqlite::Params) -> Vec<RunEventRow> {
    match conn.prepare(sql).and_then(|mut stmt| {
        stmt.query_map(args, row_to_event)
            .map(|r| r.flatten().collect())
    }) {
        Ok(rows) => rows,
        Err(e) => {
            log::warn!("[pipeline] run-store event read failed, reporting none: {e}");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod test;
