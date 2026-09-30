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
//! **Every column is bounded on the import path**, in one of three ways, and
//! the difference between them is deliberate:
//!
//! * the two free-form JSON columns are CLAMPED at the write site and again on
//!   import (see [`ARTIFACT_CAP_BYTES`]/[`METRICS_CAP_BYTES`]) — a truncated
//!   summary still tells the truth about a run;
//! * `phase` is a closed vocabulary enforced by a schema CHECK (see
//!   `store::CREATE_PIPELINE_RUNS_SQL`), which holds on both paths;
//! * every identity/text column (id, job_url, kind, depth, status,
//!   stopped_reason, stage) is REJECTED past its byte cap on import — see
//!   `import_export::check_run` — because truncating an identity does not shorten
//!   it, it changes what it points at. The bundle's ROW COUNTS are bounded the
//!   same way ([`IMPORT_MAX_RUNS`]/[`IMPORT_MAX_EVENTS`]).
//!
//! A rejected bundle aborts the WHOLE import with the pre-existing history
//! intact, the same semantics as a malformed row — the caps fail BEFORE the
//! transaction opens, and the schema CHECK fails inside it and rolls back.
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
//!
//! **One reader lives outside this crate.** `scripts/dump-run-metrics.mjs` opens
//! `pipeline_runs.db` read-only and aggregates `metrics_json` per `depth` for
//! offline depth A/B (it never touches `pipeline_run_events`, whose
//! `artifact_json` carries the strategy/evidence detail).
//!
//! That mirror is MACHINE-CHECKED, not a hand copy: `dump-run-metrics.test.mjs`
//! reads `CREATE_PIPELINE_RUNS_SQL` out of `store.rs` to build its fixture, and
//! extracts the `metrics_json` keys out of `RunLedger::metrics` and
//! `commands::resume_pipeline::execute` to check that every key the dump reads is
//! still written. So renaming a column, that const, or a metrics key fails the JS
//! suite — which the `rust` path filter runs, since `pnpm test:coverage` covers
//! the whole workspace including the `scripts` project.
//!
//! Split by responsibility: [`model`] (row types, byte caps, the JSON clamp),
//! [`store`] ([`PipelineRunStore`] itself: open, url normalization, basic
//! CRUD, and the row ↔ struct mapping underneath it), [`maintenance`]
//! (retention + deletion), and [`import_export`] (the backup `DataStore`
//! surface).

mod import_export;
mod maintenance;
mod model;
mod store;

pub use model::{
    clamp_artifact, clamp_metrics, RunEventRow, RunRow, ARTIFACT_CAP_BYTES, IMPORT_ID_CAP_BYTES,
    IMPORT_JOB_URL_CAP_BYTES, IMPORT_LABEL_CAP_BYTES, IMPORT_MAX_EVENTS, IMPORT_MAX_RUNS,
    METRICS_CAP_BYTES, RETENTION_RUNS_PER_JOB, TRUNCATION_MARKER,
};
pub use store::PipelineRunStore;

#[cfg(test)]
mod tests;
