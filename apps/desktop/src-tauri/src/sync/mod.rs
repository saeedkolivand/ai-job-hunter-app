//! Optional self-hosted multi-device sync — the client engine (ADR-051).
//!
//! Off by default; nothing here runs unless a later layer (settings, the HTTP
//! transport and the scheduler) calls [`engine::run`]. The wire contract is
//! `docs/knowledge/sync-protocol.md`.
//!
//! - [`envelope`]: protocol types, the record hash and the per-scope key.
//! - [`state`]: `sync_state.db`, this device's machine-local bookkeeping.
//! - [`rules`]: the pure conflict / tombstone / schema decision.
//! - [`engine`]: one run — held envelopes, pull + apply, diff + push.
//!
//! Change detection diffs the export bundle at sync time: each store exposes
//! its records through [`crate::data_store::SyncSection`], and no store keeps a
//! write-path change log.

pub mod engine;
pub mod envelope;
pub mod http;
pub mod legacy_float;
pub mod rules;
pub mod settings;
pub mod state;

#[cfg(test)]
mod tests;
