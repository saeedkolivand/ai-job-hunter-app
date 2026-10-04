//! Unit tests for `commands::match_resume` — the metric-label contract
//! (one pre-processing pipeline per "Match %" surface), the cache-key
//! self-invalidation, and the pure pieces of the combined formula.
//!
//! One topic per file under `tests/`; the shared fakes and fixtures are in
//! `tests/support.rs`.

use std::collections::HashSet;

use parking_lot::Mutex;
use serde_json::{json, Value};

use super::*;
use crate::applications::{clamp_to_bytes, MAX_JOB_DESCRIPTION_BYTES};
use crate::commands::ai_provider::{EmbeddingVector, EMBEDDING_VECTOR_VERSION};
use crate::documents::evidence::rank_bullets;
use crate::documents::keywords::{apply_stemmer, keyword_coverage, keywords};
use crate::documents::{
    sha256_hex, DocumentRecord, DocumentStore, Embedder, EmbeddingConfig, MatchScoreKey,
};
use crate::ipc_contracts::matching::ResumeTrimSuggestionsRequest;
use crate::locale::LocaleProfile;
use support::*;

mod ad_text;
mod budget;
mod cache_key;
mod combined_score;
mod keywords;
mod resolve;
mod support;
mod surface;
mod trim;
