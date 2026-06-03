//! Generated IPC request types.
//!
//! Files in this module are emitted by `pnpm gen:ipc` from the Zod schemas in
//! `packages/shared/src/schemas`. Do not edit them by hand — change the schema
//! and regenerate. CI runs `pnpm gen:ipc --check` to catch drift.

pub mod ai;
pub mod autopilot;
pub mod conversations;
pub mod documents;
pub mod matching;
pub mod resume;
pub mod scrape;
pub mod search;
