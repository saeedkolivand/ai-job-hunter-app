//! [`PipelineRunStore`]: the SQLite handle, its migration, url normalization,
//! basic CRUD (insert/append/read), and the row ↔ struct mapping underneath
//! it. Retention/deletion live in [`super::maintenance`]; backup
//! import/export lives in [`super::import_export`].

use std::path::Path;

use parking_lot::Mutex;
use rusqlite::{params, Connection};

use crate::db::{run_migrations, ts_from_db, ts_to_db, Migration};
use crate::error::{AppError, AppResult};
use crate::observability::sanitize_reason;

use super::model::{clamp_artifact, clamp_metrics, RunEventRow, RunRow};

/// This store's single migration, hoisted out of the closure so it is a NAMED
/// STATIC the drift guard can read.
///
/// The `phase` CHECK was added to this entry IN PLACE rather than as a second
/// migration, which is legal exactly once: the table has never shipped, so no
/// install has run migration 0 yet. Every later change to the vocabulary must
/// APPEND (see [`PipelineRunStore::MIGRATIONS`]).
///
/// Static on purpose: the `phase` CHECK spells its vocabulary out as literals
/// rather than interpolating `ipc_contracts::events::PIPELINE_STAGE_PHASES`.
/// Generating it from the const would make a widened TS vocabulary apply to NEW
/// installs and silently NOT to migrated ones — two schemas, no failure
/// anywhere. Written out, a widened contract instead fails
/// `phase_check_matches_the_generated_contract` until someone APPENDS the
/// migration that widens the CHECK on existing installs too.
pub(super) const CREATE_PIPELINE_RUNS_SQL: &str =
    // `id TEXT PRIMARY KEY` alone is NOT enough: SQLite permits NULL in a TEXT
    // PRIMARY KEY (the historical INTEGER-PRIMARY-KEY exemption, kept for
    // backwards compatibility), and ONE null-id row turns any `NOT IN (SELECT id
    // FROM pipeline_runs)` sweep into a permanent silent no-op. `NOT NULL` closes
    // that hole at the schema; `PipelineRunStore::prune`'s NOT EXISTS closes it
    // again in SQL.
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
        -- CLOSED vocabulary, unlike `stopped_reason` one table up: that one is
        -- deliberately loose TEXT because its variants grow and an old bundle
        -- must still restore, whereas a stage has exactly these three lifecycle
        -- phases. Enforced here so it also holds for `import`, which writes rows
        -- straight from a file the user can edit.
        phase         TEXT NOT NULL CHECK (phase IN ('start', 'finish', 'error')),
        artifact_json TEXT NOT NULL,
        PRIMARY KEY (run_id, seq)
     );";

/// The ONE spelling of a posting url this table stores.
///
/// **Normalized at the WRITE site, matched exactly by every reader.** The two
/// halves used to disagree: `commands::resume_pipeline::execute` wrote the
/// postings cache's RAW url while `delete_for_job` normalized before comparing,
/// so a delete correctly removed the trail of a posting whose link carried a
/// `utm_*` param or a fragment — and `runs_for_job`, called with the
/// application's own (normalized) url, could not find that same run to list it.
/// A store where the delete and the list disagree about which rows belong to a
/// posting is a store that reports one thing and does another.
///
/// Normalizing at the write site is the same choice [`clamp_metrics`] and
/// [`clamp_artifact`] make, for the same reason: a rule enforced where the
/// value enters cannot be forgotten by a future caller. An empty url stays
/// empty — an unlinked run is a real state.
///
/// **The by-url READERS normalize their argument too, and that is not a second
/// seam — it is this one, applied at every boundary a url crosses.** The review
/// that found the split-brain prescribed "normalize on write, keep readers
/// exact-match", on the assumption that callers hold the application's
/// normalized key. They do not: `usePipelineRunsForJob(posting.url)` passes the
/// postings cache's RAW link, so exact-match readers would have moved the bug
/// rather than fixed it — the runs panel would go empty for every posting whose
/// link carries a `utm_*` param or a fragment. Normalizing both sides is what
/// actually makes "what the list shows" and "what the delete removes" the same
/// set.
pub(super) fn normalized_job_url(job_url: &str) -> String {
    crate::applications::normalize_job_url(job_url)
}

/// Rewrite any pre-existing row whose `job_url` is not in its normalized
/// spelling — a ONE-TIME sweep at open.
///
/// Not a migration: `MIGRATIONS` is position-indexed and append-only, and this
/// is idempotent data repair rather than a schema change, so running it every
/// open is both cheaper to reason about and self-healing if a future writer
/// regresses. The table is bounded by retention (three runs per
/// `(job_url, kind)`), so the scan is small; the UPDATE only touches rows that
/// actually differ, so a normalized store does no writes at all.
///
/// **LOAD-BEARING, not cosmetic — and best-effort is a real cost here.** Once
/// [`super::maintenance::PipelineRunStore::delete_for_job`] became an indexed exact match, an un-normalized row stopped
/// being merely hard to list: it cannot be DELETED either, and the delete still
/// reports success, so its strategy/evidence detail outlives the owner that was
/// supposed to remove it. A failure here therefore leaves those rows readable
/// exactly as they were AND undeleteable until the next successful open — which
/// is the state the app shipped with, but it is not harmless, and it is why the
/// write sites (`upsert_run` and `import`) both normalize rather than leaning on
/// this. With `import` fixed, the only rows this can still find are genuinely
/// LEGACY ones written by an older build.
fn normalize_existing_job_urls(conn: &Connection) {
    let Ok(mut stmt) = conn.prepare("SELECT id, job_url FROM pipeline_runs") else {
        return;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return;
    };
    let stale: Vec<(String, String)> = rows
        .filter_map(Result::ok)
        .filter_map(|(id, raw)| {
            let normalized = normalized_job_url(&raw);
            (normalized != raw).then_some((id, normalized))
        })
        .collect();
    for (id, normalized) in stale {
        if let Err(e) = conn.execute(
            "UPDATE pipeline_runs SET job_url = ?1 WHERE id = ?2",
            params![normalized, id],
        ) {
            log::warn!(
                "[pipeline] could not normalize a legacy run's job_url: {}",
                sanitize_reason(&e.to_string())
            );
        }
    }
}

pub struct PipelineRunStore {
    pub(super) conn: Mutex<Connection>,
}

impl PipelineRunStore {
    /// POSITION-INDEXED migrations: `db::run_migrations` gates each entry on its
    /// INDEX via `PRAGMA user_version`, so this list is APPEND-ONLY. Editing or
    /// reordering an existing entry silently skips it on every already-migrated
    /// install.
    const MIGRATIONS: &'static [Migration] = &[Migration {
        name: "create_pipeline_runs",
        up: |conn| conn.execute_batch(CREATE_PIPELINE_RUNS_SQL),
    }];

    pub fn open(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join("pipeline_runs.db");
        let mut conn = crate::db::open(&path)?;
        run_migrations(&mut conn, Self::MIGRATIONS)?;
        normalize_existing_job_urls(&conn);
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
    /// can bypass [`super::model::METRICS_CAP_BYTES`] by forgetting to.
    pub fn upsert_run(&self, run: &RunRow) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO pipeline_runs
                (id, job_url, kind, depth, status, started_at, finished_at,
                 stopped_reason, metrics_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.id,
                // NORMALIZED at the single write site, exactly like
                // `clamp_metrics` below — see `normalized_job_url`.
                normalized_job_url(&run.job_url),
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
    ///
    /// Takes the url in ANY spelling: it goes through [`normalized_job_url`],
    /// the same seam the write site uses, so a caller holding the postings
    /// cache's raw link and a caller holding the application's normalized key
    /// resolve to the same rows — and to the same rows
    /// [`super::maintenance::PipelineRunStore::delete_for_job`] would remove.
    ///
    /// An empty result for an empty key, mirroring that function's own guard.
    /// `normalized_job_url` maps anything it cannot read as an http(s) url to
    /// `""` (a `javascript:` scheme, a control-character paste, whitespace), and
    /// `""` is also what every UNLINKED run is stored under — so without this a
    /// renderer-supplied junk url arriving through
    /// `resume_pipeline_list_for_job` would list every unlinked run in the
    /// store, i.e. other postings' history under a url that names none of them.
    pub fn runs_for_job(&self, job_url: &str) -> Vec<RunRow> {
        let wanted = normalized_job_url(job_url);
        if wanted.is_empty() {
            return Vec::new();
        }
        let conn = self.conn.lock();
        query_runs(
            &conn,
            "SELECT id, job_url, kind, depth, status, started_at, finished_at,
                    stopped_reason, metrics_json
             FROM pipeline_runs WHERE job_url = ?1 ORDER BY started_at DESC, id DESC",
            params![wanted],
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

    /// Every run, oldest first — a deterministic order for export.
    pub(super) fn all_runs(&self) -> Vec<RunRow> {
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
    pub(super) fn all_events(&self) -> Vec<RunEventRow> {
        let conn = self.conn.lock();
        query_events(
            &conn,
            "SELECT run_id, seq, ts, stage, phase, artifact_json
             FROM pipeline_run_events ORDER BY run_id, seq",
            params![],
        )
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
            log::warn!(
                "[pipeline] run-store read failed, reporting no runs: {}",
                sanitize_reason(&e.to_string())
            );
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
            log::warn!(
                "[pipeline] run-store event read failed, reporting none: {}",
                sanitize_reason(&e.to_string())
            );
            Vec::new()
        }
    }
}
