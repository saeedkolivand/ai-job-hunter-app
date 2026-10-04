//! Store tests for [`AiGenerationStore`].
//!
//! Split by topic: the per-job aggregate save and the unique-url index ([`aggregate`]);
//! the targeted edits, deletes and url reads ([`edits`]); the pure record merge
//! ([`merge`]); the `quality_report` cap and its save-path behaviour ([`report_cap`]);
//! export / import ([`import_export`]); and the migrations ([`schema_migrations`], and
//! the PDF-mojibake repair with its write-path twins in [`mojibake`]). [`support`]
//! holds the fixtures the siblings share.

use rusqlite::params;
use tempfile::TempDir;

use super::quality_report::{sanitize_quality_report, QUALITY_REPORT_MAX_BYTES};
use super::record::merge_application;
use super::*;
use crate::data_store::DataStore;
use crate::db::now_ms;

mod aggregate;
mod edits;
mod import_export;
mod merge;
mod mojibake;
mod report_cap;
mod schema_migrations;
mod support;
