//! Store tests for [`JobPreferencesStore`].
//!
//! Split by topic: reads, writes and the semantic mirror ([`store`]); the clamped
//! free-text columns and their single-column setters ([`clamped_columns`]); the
//! serialized wire shape ([`wire`]); the migration chain ([`migrations`]); and the
//! atomic read-merge-write `update` ([`update`]).
//! [`support`] holds the fixtures the siblings share.

use tempfile::TempDir;

use super::*;

mod clamped_columns;
mod migrations;
mod store;
mod support;
mod update;
mod wire;
