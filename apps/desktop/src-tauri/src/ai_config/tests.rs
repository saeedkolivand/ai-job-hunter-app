//! Round-trip, validation, seed-gating, reset, and backup tests for
//! [`AiConfigStore`] — the invariant lock for the backend-owned provider store.
//!
//! Split by topic: the defaults, the switch-vs-edit split and the writer's
//! validation ([`writer`]); the PATCH semantics ([`patch`]); the seed, factory
//! reset and backup round-trip ([`seed_import`]). [`support`] holds the fixtures
//! the siblings share.

use tempfile::TempDir;

use super::{AiConfigSnapshot, AiConfigStore, ProviderConfig, ProviderSettingsPatch};
use crate::data_store::DataStore;

mod patch;
mod seed_import;
pub(in crate::ai_config) mod support;
mod writer;
