//! Split out of `policy.rs` (R8's hard LOC cap — the same reason
//! `agent_call/tests.rs`/`documents/sql.rs`/`applications/reminders.rs`
//! exist) — this is tests only, no logic, so it earns its own file the
//! moment the combined module crosses the cap rather than growing the
//! production file further.
//!
//! Its own test count now exceeds the cap on its own, so it is a hub: one file
//! per topic, declared here. Every test below moved verbatim; nothing but the
//! `super::super::catalogue` path depth and the per-topic `use` lines changed.

mod catalogue_coverage;
mod catalogue_descriptions;
mod catalogue_wrapper_args;
mod handler_signatures;
mod proof_sources;
mod table_coverage;
mod table_pinned_rows;
mod table_reclassifications;
