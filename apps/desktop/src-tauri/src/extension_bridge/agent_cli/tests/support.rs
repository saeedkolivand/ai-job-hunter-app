//! The fixtures every sub-module's own test suite needs: the argv helper and
//! the fully-defaulted `Verb::FoundJobs` builder. `pub(crate)` rather than
//! `pub(super)` because the consumers are SIBLING units' `tests` modules
//! (`verb`, `parse`, `parse_found_jobs`, …), not descendants of this one.

use super::super::Verb;

pub(crate) fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// A fully-defaulted `Verb::FoundJobs` — every field named explicitly (Rust's
/// struct-update `..base` syntax does not exist for enum variants), so a NEW
/// field added later is a compile error at every one of these call sites
/// instead of a silent `None`/`false` default nobody notices.
#[allow(clippy::too_many_arguments)]
pub(crate) fn found_jobs(
    autopilot_id: Option<&str>,
    limit: Option<u64>,
    cursor: Option<&str>,
    min_score: Option<f64>,
    country: Option<&str>,
    remote: Option<bool>,
    applied: Option<bool>,
    query: Option<&str>,
    include_description: bool,
) -> Verb {
    Verb::FoundJobs {
        autopilot_id: autopilot_id.map(str::to_string),
        limit,
        cursor: cursor.map(str::to_string),
        min_score,
        country: country.map(str::to_string),
        remote,
        applied,
        query: query.map(str::to_string),
        include_description,
    }
}
