//! Tests for `projects.rs`'s Projects-section normalization, split by topic:
//! [`normalize`] (the `normalize_projects*` behavior) and [`seeding`] (source
//! seed extraction, the whole-bail guards, and `seeds_are_plausible`). Shared
//! fixtures in [`support`].

mod normalize;
mod seeding;
mod support;
