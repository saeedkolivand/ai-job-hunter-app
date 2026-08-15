//! The Projects seed type shared by the deterministic quality-depth Projects
//! normalization (`pipeline::resume::projects`) and its seeder
//! (`pipeline::resume::source`).
//!
//! This module used to carry the whole max-depth section-generation type
//! family (one typed answer per résumé section, plus the max-only judge,
//! shown to a model over JSON — which is why every type there was flat and
//! serde-round-trippable). That machinery was removed with the `max`
//! generation depth; [`ProjectOut`] survives as a plain in-process value —
//! it is also the seed type PR #990's deterministic Projects normalization
//! renders through `assemble::render_project`, unrelated to depth, and
//! nothing (de)serializes it any more.

/// ONE project entry, in the owner-locked source signature.
///
/// `name`, `links` and `stack` are seeded VERBATIM from the parsed source and
/// re-seeded after a draft is checked; `description` carries the SOURCE's own
/// description (empty when the source has none), which is what
/// `pipeline::resume::projects` uses to decide whether a rewritten
/// description may stand at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectOut {
    pub name: String,
    pub links: Vec<String>,
    pub stack: Vec<String>,
    pub description: String,
}
