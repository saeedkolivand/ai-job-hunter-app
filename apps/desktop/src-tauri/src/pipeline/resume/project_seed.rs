//! The Projects seed type shared by the deterministic quality-depth Projects
//! normalization (`pipeline::resume::projects`) and its seeder
//! (`pipeline::resume::source`). [`ProjectOut`] is a plain in-process value —
//! nothing (de)serializes it — rendered through
//! `project_render::render_project`.

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
