//! `found-jobs`' server-side filter argument parsing and predicate (issue #1167) — split out of
//! `found_jobs.rs` under the R8 LOC cap.

use serde_json::Value;

use crate::autopilot::FoundJob;
use crate::error::{AppError, AppResult};
use crate::scraping::engine::location_filter::REMOTE_MARKERS;

/// Server-side filters for `found-jobs` (issue #1167). Every predicate here
/// is the app's OWN, already-established one — never a fresh matcher invented
/// for this surface:
/// - `remote` is THREE-valued, matching
///   [`remote_determination`]'s truth table, not a plain boolean read of
///   `location` text: `job.board_remote` (the board's own per-posting
///   classification), [`crate::scraping::boards::is_all_remote_board`] (the
///   board's REGISTRY-level "every posting is remote" declaration —
///   retroactive for a `FoundJob` persisted before `board_remote` existed,
///   round-4 fix T1), or a
///   [`REMOTE_MARKERS`](crate::scraping::engine::location_filter::REMOTE_MARKERS)
///   hit in `location` text all decide `true`; a non-empty `location` with
///   none of those decides `false`; an empty/absent `location` with none of
///   those is UNDECIDED and matches neither `remote: true` nor
///   `remote: false` (round-4 fix T2 — the old two-valued read reported an
///   unknown row as a confident `false`).
/// - `country` is a case-insensitive substring match against `location` —
///   the SAME predicate the Jobs page's own free-text filter applies to a
///   posting's location (`(p.location ?? '').toLowerCase().includes(q)` in
///   `JobsPage`), not a structured country-code compare: `FoundJob` carries
///   no `countryCode` field, and `commands::match_resume::constraints`
///   documents that a bare country code "contributes nothing to the
///   matchable token" for this exact reason — only place text does.
/// - `query` mirrors that same JobsPage substring filter's title/company
///   half (its location half becomes the separate `country` filter above).
/// - `applied` reads the SAME derived-at-read-time set
///   [`crate::commands::autopilot::applied_job_urls`] produces for
///   `best-matches`/`autopilot_list`, never the stale stored bit.
// `pub(super)` — `agent_read::tests`' `no_resource_output_ever_carries_a_forbidden_key` and
// `automations_found_jobs_total_matches_found_jobs_own_total` build a `FoundJobsFilters` to call
// `resolve_found_jobs` directly (a sibling module, not a descendant of this one, needs the same
// widening `found_jobs::tests` gets automatically as a child).
#[derive(Debug)]
pub(in crate::extension_bridge::agent_read) struct FoundJobsFilters {
    pub(super) min_score: Option<f64>,
    /// Lowercased.
    pub(super) country: Option<String>,
    pub(super) remote: Option<bool>,
    pub(super) applied: Option<bool>,
    /// Lowercased.
    pub(super) query: Option<String>,
    pub(super) include_description: bool,
}

/// One shared refusal for every filter argument below that is PRESENT but
/// not readable as its declared shape (B3-r1-F3 — a wrong-typed value, e.g.
/// `{"minScore": "70"}` off the raw `agent.query` payload path, used to
/// vanish silently through `.and_then(Value::as_*)` returning `None` for a
/// mismatch exactly like it does for "absent") — and, for the two string
/// filters, also PRESENT-but-blank (B3-r2-F2, see
/// [`trimmed_lowercase_filter`]'s own doc). The caller got an UNFILTERED
/// page back with a `total` it read as filtered. Refusing here instead means
/// the filter this call asked for either applies or the call fails loudly —
/// never a third, silent option. Names the KEY, not the caller's value
/// (never echoed) — the key is this resource's own static schema, not
/// caller data.
fn unreadable_filter_message(key: &str) -> AppError {
    AppError::Validation(format!(
        "{key} was present but not usable as its declared type — remove it or fix its value"
    ))
}

/// `payload.get(key)`, refusing anything present that is neither absent/
/// `null` nor a non-blank JSON string. A PRESENT-but-blank/whitespace-only
/// string now refuses too (round 2 fix, B3-r2-F2 — it used to read as
/// "filter not set", silently widening the call to the entire corpus with a
/// `total` the caller reads as the filtered count; the canonical repro is a
/// shell caller forwarding an unset variable, e.g. `--query "$ROLE"` with
/// `ROLE` empty). There is no legitimate caller that types an explicitly
/// empty filter, so this now mirrors `parse_autopilot_id_arg`'s blank-must-
/// refuse rule even though `query`/`country` are additive filters, not
/// selectors — only the OMITTED key still means "no filter".
///
/// `pub(super)` (round 2 fix, B3-r2-F1) so `agent_read::best_matches_resource`
/// reuses this SAME fallible parse for its own `query` argument rather than
/// the bare `.and_then(Value::as_str)` combinator that let a wrong-typed or
/// blank `query` collapse silently to "absent" on that resource too.
pub(in crate::extension_bridge::agent_read) fn trimmed_lowercase_filter(
    payload: &Value,
    key: &str,
) -> AppResult<Option<String>> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Err(unreadable_filter_message(key))
            } else {
                Ok(Some(trimmed.to_lowercase()))
            }
        }
        Some(_) => Err(unreadable_filter_message(key)),
    }
}

/// `payload.get(key)`, refusing anything present that is neither absent/
/// `null` nor a JSON boolean.
fn bool_filter(payload: &Value, key: &str) -> AppResult<Option<bool>> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(unreadable_filter_message(key)),
    }
}

impl FoundJobsFilters {
    /// Fallible (B3-r1-F3) — a filter key that IS present must either parse
    /// as its declared shape or refuse the whole call; it can no longer
    /// silently collapse to "no filter" the way `.and_then(Value::as_*)`
    /// alone would for a wrong-typed value.
    ///
    /// `minScore`'s `is_finite()` guard is defense-in-depth, not the fix for
    /// the non-finite `--min-score` repro (`1e400`/`inf`/`nan`): RFC 8259
    /// has no `Infinity`/`NaN` token, so `json!(non_finite_f64)` collapses
    /// to `null` BEFORE this ever runs, and `None | Some(Value::Null) =>
    /// None` below already treats that the same as "absent" — the
    /// established, intentional convention for every filter/cursor here,
    /// not a bug. The load-bearing half of that fix is upstream, at the
    /// CLI's own `--min-score` parse (`agent_cli::parse_found_jobs`), which
    /// refuses the non-finite value before it is ever handed to `json!` —
    /// see that fn's own doc and
    /// `found_jobs::tests::found_jobs_filters_from_payload_treats_a_null_min_score_as_absent`.
    pub(in crate::extension_bridge::agent_read) fn from_payload(
        payload: &Value,
    ) -> AppResult<Self> {
        let min_score = match payload.get("minScore") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_f64()
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| unreadable_filter_message("minScore"))?,
            ),
        };
        Ok(Self {
            min_score,
            country: trimmed_lowercase_filter(payload, "country")?,
            remote: bool_filter(payload, "remote")?,
            applied: bool_filter(payload, "applied")?,
            query: trimmed_lowercase_filter(payload, "query")?,
            include_description: bool_filter(payload, "includeDescription")?.unwrap_or(false),
        })
    }
}

/// THREE-valued remote determination for `job` (round-4 fix T1/T2— advisory
/// findings on PR #1182). `Some(true)`: `job.board_remote` (the board's own
/// per-posting classification, set at scrape time — see `build_found_job`),
/// [`crate::scraping::boards::is_all_remote_board`] (the board's
/// REGISTRY-level "every posting is remote" declaration, checked against the
/// stored `board` id — retroactive, so a `FoundJob` persisted before
/// `board_remote` existed, or scraped from a board that only started
/// setting the flag later, still resolves correctly), or a
/// [`REMOTE_MARKERS`] hit in `location` text. `Some(false)`: a non-empty
/// `location` with none of the above — a real place, stated. `None`
/// ("undecided"): an empty/absent `location` with none of the above —
/// genuinely unknown, not a negative. [`passes_filters`]'s `remote` filter
/// matches NEITHER `true` nor `false` for `None`, so an unknown row is
/// excluded from both directions rather than silently counted as "not
/// remote".
fn remote_determination(job: &FoundJob) -> Option<bool> {
    let loc = job.location.as_deref().unwrap_or("").trim().to_lowercase();
    if job.board_remote
        || crate::scraping::boards::is_all_remote_board(job.board.as_deref().unwrap_or(""))
        || REMOTE_MARKERS.iter().any(|m| loc.contains(m))
    {
        return Some(true);
    }
    if loc.is_empty() {
        return None;
    }
    Some(false)
}

/// True when `job` survives every filter set in `filters`. `is_applied` is
/// passed in (precomputed once per job by [`candidate_jobs`]) rather than
/// recomputed here, so the SAME derivation backs both this filter and the
/// row's own `applied` field.
pub(super) fn passes_filters(job: &FoundJob, filters: &FoundJobsFilters, is_applied: bool) -> bool {
    if let Some(min) = filters.min_score {
        match job.score {
            Some(s) if s >= min => {}
            _ => return false,
        }
    }
    if filters.country.is_some() || filters.remote.is_some() {
        let loc = job.location.as_deref().unwrap_or("").to_lowercase();
        if let Some(country) = &filters.country {
            if !loc.contains(country.as_str()) {
                return false;
            }
        }
        if let Some(want_remote) = filters.remote {
            // THREE-valued (round-4 fix T2) — an UNDECIDED row (see
            // `remote_determination`'s own doc) matches neither `true` nor
            // `false`, so it is excluded from both, never miscounted as a
            // confident negative the way the old two-valued read did.
            match remote_determination(job) {
                Some(actual) if actual == want_remote => {}
                _ => return false,
            }
        }
    }
    if let Some(want_applied) = filters.applied {
        if is_applied != want_applied {
            return false;
        }
    }
    if let Some(q) = &filters.query {
        let title = job.title.to_lowercase();
        let company = job.company.to_lowercase();
        if !title.contains(q.as_str()) && !company.contains(q.as_str()) {
            return false;
        }
    }
    true
}
