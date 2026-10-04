//! Store tests for [`DocumentStore`] — the CRUD, the embedding-vector and derived-cache tables, the
//! migrations that build them, and the backup surface.
//!
//! Split by topic: document CRUD and the factory-reset wipe ([`store`]); export / import
//! ([`import_export`]); document vectors and the active space ([`document_vectors`]); the
//! `posting_vectors` / `match_scores` / `help_vectors` caches ([`posting_vectors`],
//! [`match_scores`], [`help_cache`]); their eviction ([`pruning`], [`ttl`]); and the migrations
//! ([`schema_migrations`], [`mojibake`]). [`support`] holds the fixtures the siblings share.

use std::path::PathBuf;

use rusqlite::{params, Connection};
use serial_test::serial;
use tempfile::TempDir;

use super::*;
use crate::commands::ai_provider::{EmbeddingSpace, EmbeddingVector, EMBEDDING_VECTOR_VERSION};

mod document_vectors;
mod help_cache;
mod import_export;
mod match_scores;
mod mojibake;
mod posting_vectors;
mod pruning;
mod schema_migrations;
mod store;
mod support;
mod ttl;
