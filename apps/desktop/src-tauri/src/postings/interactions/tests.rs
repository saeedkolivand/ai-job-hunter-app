//! Tests for the interaction record, its store and the join onto cached
//! postings, split by topic: the in-memory store operations ([`store`]), the
//! on-disk file — atomic writes and corrupt-file handling ([`persistence`]) — and
//! the posting join ([`join`]). [`support`] holds the fixtures the siblings share.

use tempfile::TempDir;

use super::*;

mod join;
mod persistence;
mod store;
mod support;
