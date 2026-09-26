//! `found-jobs`' candidate-list build + per-row projection + the pure resolve core — split out of
//! `found_jobs.rs` under the R8 LOC cap.

use std::collections::HashSet;

use serde_json::{json, Value};

use crate::autopilot::{Autopilot, FoundJob};
use crate::error::{AppError, AppResult};
use crate::scraping::boards::common::canonical_job_key;

use super::super::best_matches::fence_posting_display_fields;
use super::super::job::project_value;
use super::cursor::{found_jobs_cursor_issuer, AUTOPILOT_NOT_FOUND_MESSAGE};
use super::filters::{passes_filters, FoundJobsFilters};
use super::{
    base_envelope_cost, cap_autopilot_name, fence_found_jobs_description, trim_page_to_budget,
    FoundJobSlice,
};

/// The ordered, filtered candidate list across every autopilot in `scoped` — ready to be sliced
/// `[offset, offset + limit)`. `dedupe_across_autopilots` (issue #1168) additionally collapses
/// rows sharing the same [`canonical_job_key`] to their FIRST occurrence THAT ALSO PASSES this
/// call's filters (round 2 fix, B3-r1-F1 — filtering before dedup, or a copy that fails a filter
/// could consume the slot a later, passing copy needed) — needed ONLY for a spanning traversal,
/// where the same posting can legitimately surface in more than one autopilot's own list. Scoped
/// to exactly one autopilot, `false`: that list is already deduped at merge time, and
/// `automations`' own `foundJobsTotal` promises `total` here equals that list's plain
/// `found_jobs.len()` — re-deduping would silently break that promise the moment stored data isn't
/// ALREADY deduped for some other reason.
fn candidate_jobs<'a>(
    scoped: &[&'a Autopilot],
    filters: &FoundJobsFilters,
    applied_urls: &HashSet<String>,
    dedupe_across_autopilots: bool,
) -> Vec<(&'a Autopilot, &'a FoundJob, bool)> {
    // Built ONCE per call, not per row (round-4 perf fix — see
    // `agent_read::applied_url_identities`'s own doc): the loop below can run
    // over every found job across every scoped autopilot, and re-decoding +
    // re-parsing the whole `applied_urls` set per row was the hot path.
    let applied_identities = super::super::job::resolve::applied_url_identities(applied_urls);
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for &ap in scoped {
        for job in &ap.found_jobs {
            // FILTER first, dedup second (B3-r1-F1 — the reverse order let a
            // posting that failed a filter under the FIRST autopilot holding
            // it consume the dedup slot and vanish entirely, even when a
            // LATER autopilot's copy of the same posting would have passed;
            // `minScore` is per-autopilot-scored — `build_found_job` scores
            // each autopilot's own copy against ITS OWN `resume_text` — so
            // this was silently under-reporting `total` on the very filter
            // this resource exists to serve). The first PASSING occurrence
            // in store order now wins the dedup, not merely the first one.
            let is_applied = super::super::job::resolve::job_is_applied_indexed(
                &job.url,
                applied_urls,
                &applied_identities,
            );
            if !passes_filters(job, filters, is_applied) {
                continue;
            }
            if dedupe_across_autopilots {
                let key = canonical_job_key(&job.url, &job.title, &job.company);
                if !seen.insert(key) {
                    continue;
                }
            }
            out.push((ap, job, is_applied));
        }
    }
    out
}

/// Project one row: [`FoundJobSlice`]'s allowlist round trip, plus the three fields it can't carry
/// — `applied` (precomputed), `autopilotId`/`autopilotName` (the PARENT record's, fenced), and
/// `description` (only when `include_description`).
///
/// `is_applied` is `None` when the applications store is unavailable (round-4 fix T3) — the row
/// OMITS the `applied` key entirely rather than shipping a confident `false` (absent ≠ false; the
/// unsafe direction for an autonomous caller deciding whether to re-apply).
/// [`resolve_found_jobs_for_store`]'s envelope carries the matching `appliedUnavailable: true`.
///
/// INFALLIBLE, never `Option<Value>` (round-4 fix T5 — the prior fallible signature fed a
/// `filter_map` that silently dropped a "failure" while still counting it in `total`, so a page
/// whose every candidate failed returned a non-terminating cursor). [`FoundJobSlice`]'s required
/// fields are a same-typed subset of `FoundJob`'s own, so there is no value for which this
/// projection can actually fail.
///
/// The infallibility argument is a type-shape claim, not one the compiler enforces — release is
/// `panic = "abort"`, so an `.expect()` here would turn a future field-type drift into the whole
/// app dying with no crash report, not merely one dropped row. A `debug_assert!` catches the drift
/// in dev/test; release degrades to a minimal row (bare `url`) instead.
fn project_found_job_row(
    job: &FoundJob,
    autopilot: &Autopilot,
    include_description: bool,
    is_applied: Option<bool>,
) -> Value {
    let mut value = project_value::<_, FoundJobSlice>(job).unwrap_or_else(|| {
        debug_assert!(
            false,
            "FoundJobSlice is a same-typed subset of FoundJob and cannot fail to project"
        );
        json!({ "url": job.url })
    });
    if include_description {
        if let Some(desc) = &job.description {
            value["description"] = json!(desc);
            fence_found_jobs_description(&mut value);
        }
    }
    fence_posting_display_fields(&mut value);
    if let Some(applied) = is_applied {
        value["applied"] = json!(applied);
    }
    value["autopilotId"] = json!(autopilot.id);
    value["autopilotName"] = json!(cap_autopilot_name(&autopilot.name));
    value
}

/// Directly unit-testable with hand-built `Autopilot` records, no `AppHandle` — same pure/impure
/// split as `agent_read::resolve_job`. `pub(super)` because `agent_read`'s own
/// `no_resource_output_ever_carries_a_forbidden_key` test calls this directly.
///
/// Assumes the applications store is present; see [`resolve_found_jobs_for_store`] for the
/// store-unavailable path (round-4 fix T3) — this wrapper exists only so existing store-present
/// tests keep their original call shape.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(in crate::extension_bridge::agent_read) fn resolve_found_jobs(
    records: &[Autopilot],
    autopilot_id: Option<&str>,
    filters: &FoundJobsFilters,
    applied_urls: &HashSet<String>,
    offset: usize,
    limit: usize,
) -> AppResult<Value> {
    resolve_found_jobs_for_store(
        records,
        autopilot_id,
        filters,
        applied_urls,
        offset,
        limit,
        true,
    )
}

/// [`resolve_found_jobs`] plus the store-unavailable path (round-4 fix T3):
/// when `store_present` is `false`, every row OMITS its `applied` key (see
/// [`project_found_job_row`]'s own doc) and the envelope carries
/// `appliedUnavailable: true`.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_found_jobs_for_store(
    records: &[Autopilot],
    autopilot_id: Option<&str>,
    filters: &FoundJobsFilters,
    applied_urls: &HashSet<String>,
    offset: usize,
    limit: usize,
    store_present: bool,
) -> AppResult<Value> {
    let scoped: Vec<&Autopilot> = match autopilot_id {
        Some(id) => {
            let ap = records
                .iter()
                .find(|a| a.id == id)
                .ok_or_else(|| AppError::Validation(AUTOPILOT_NOT_FOUND_MESSAGE.to_string()))?;
            vec![ap]
        }
        None => records.iter().collect(),
    };

    let candidates = candidate_jobs(&scoped, filters, applied_urls, autopilot_id.is_none());
    let total = candidates.len();

    // `project_found_job_row` is INFALLIBLE (round-4 fix T5 — see its own
    // doc), so `.map` here always yields exactly one row per candidate in
    // this window; the only way `page` (below, post-`trim_page_to_budget`)
    // can be shorter than this window is byte-budget trimming, which is
    // meant to be retried next page.
    let page_values: Vec<Value> = candidates
        .iter()
        .skip(offset)
        .take(limit)
        .map(|(ap, job, is_applied)| {
            project_found_job_row(
                job,
                ap,
                filters.include_description,
                store_present.then_some(*is_applied),
            )
        })
        .collect();

    let cursor_issuer = found_jobs_cursor_issuer(autopilot_id, filters);
    let single = match (autopilot_id, scoped.as_slice()) {
        (Some(_), [ap]) => Some(*ap),
        _ => None,
    };
    let autopilot_name_capped = single.map(|ap| cap_autopilot_name(&ap.name));

    let base_cost = base_envelope_cost(
        &cursor_issuer,
        single
            .zip(autopilot_name_capped.as_deref())
            .map(|(ap, name)| (ap.id.as_str(), name)),
        total,
    );

    let page = trim_page_to_budget(page_values, base_cost);

    let returned = page.len();
    let next_offset = offset + returned;
    let next_cursor = if next_offset < total {
        Some(format!("{cursor_issuer}:{next_offset}"))
    } else {
        None
    };

    let mut envelope = json!({
        "jobs": page,
        "nextCursor": next_cursor,
        "total": total,
    });
    if let (Some(ap), Some(name)) = (single, autopilot_name_capped) {
        envelope["autopilotId"] = json!(ap.id);
        envelope["autopilotName"] = json!(name);
    }
    if !store_present {
        envelope["appliedUnavailable"] = json!(true);
    }
    Ok(envelope)
}
