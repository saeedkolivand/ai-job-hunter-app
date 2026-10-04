//! Tests for the autopilot store, split by topic under the R8 LOC cap. [`support`] holds the
//! fixtures the topic modules share (and that the other autopilot-family test modules reuse).

mod board_health;
mod corrupt_file;
mod crud;
mod description_updates;
mod found_jobs_db;
mod merge_dedup;
mod merge_resurface;
mod new_job_counts;
mod persistence;
mod record_serde;
mod relax;
mod run_status;
pub(crate) mod support;
mod target_serde;
