//! Guards for the per-stage override table.
//!
//! Every test below was mutation-checked: the comment names the change that
//! makes it fail, and each was applied and reverted rather than assumed.
//!
//! Split by topic: the writer's round-trip and validation ([`writes`]); the
//! stage vocabulary and the free stages ([`vocabulary`]); backup round-trip and
//! import hardening ([`backup`]); and the restore/seed paths that must tolerate
//! one bad row ([`lenient_restore`]). [`support`] holds the fixture the
//! siblings share; the store fixture is the parent's.

use super::{is_overridable_stage, is_pipeline_stage, StageOverride, MAX_STAGE_OVERRIDES};
use crate::ai_config::tests::support::new_store;
use crate::ai_config::AiConfigSnapshot;
use crate::data_store::DataStore;
use crate::ipc_contracts::events::PIPELINE_STAGES;

mod backup;
mod lenient_restore;
mod support;
mod vocabulary;
mod writes;
