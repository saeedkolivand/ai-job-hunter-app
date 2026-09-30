//! Unit tests for `mod.rs` — the shared `AiProvider` trait, registry, and
//! cross-adapter helpers — split by topic (R8b out-of-line layout). Being
//! CHILDREN of the `ai_provider` module, each topic's `use super::super::*`
//! still reaches every private item there.

mod chat_and_intent;
mod effort_tiers;
mod provider_routing;
mod shared_infra;
