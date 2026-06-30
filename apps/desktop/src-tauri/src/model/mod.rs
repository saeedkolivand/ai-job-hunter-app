//! Canonical document model and pure transforms — the format-agnostic core the
//! resume / cover-letter backends will render from.
//!
//! Phase 1 (foundations) introduces these types ALONGSIDE the existing
//! text-based renderers, which do not consume them yet. The model therefore has
//! no non-test callers until Phase 2+ wires the backends onto it, so
//! `dead_code` is allowed crate-internally here — mirroring the existing
//! `#[allow(dead_code)]` on the legacy parsed types in `export::types`. Remove
//! this once the backends consume the model.
#![allow(dead_code)]

pub mod adapter;
pub mod document;
pub mod rich;
pub mod transform;
pub mod version;
