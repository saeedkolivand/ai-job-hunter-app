//! Generated IPC request types.
//!
//! Files in this module are emitted by `pnpm gen:ipc` from the Zod schemas in
//! `packages/shared/src/schemas`. Do not edit them by hand — change the schema
//! and regenerate. CI runs `pnpm gen:ipc --check` to catch drift.

pub mod ai;
pub mod applications;
pub mod autopilot;
pub mod documents;
pub mod event_payloads;
#[cfg(test)]
mod event_payloads_test;
pub mod events;
pub mod matching;
pub mod provider_slots;
pub mod referrals;
pub mod resume;
pub mod scrape;
