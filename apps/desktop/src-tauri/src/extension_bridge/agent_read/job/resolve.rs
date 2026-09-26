//! The `job` resource's pure resolve core — split out of `agent_read/job.rs` under the same R8
//! LOC cap. `resolve_job_for_store`'s impure caller-side shell (`job_resource`) stays one level
//! up, in `job.rs` itself.

use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

use super::{project_value, AgentJob};

/// Fixed sentinel — no autopilot has surfaced a job at this url. No dynamic
/// content (wire-error discipline, matches every other verb in this bridge).
pub(in crate::extension_bridge::agent_read) const JOB_NOT_FOUND_MESSAGE: &str =
    "no job found for this url";

/// Fixed detail attached to [`JOB_NOT_FOUND_MESSAGE`] (issue #1166) — names
/// where a caller that still misses can read the url the app actually
/// stored, rather than being left at a dead end.
pub(in crate::extension_bridge::agent_read) const JOB_NOT_FOUND_DETAIL: &str =
    "the stored url for a posting can be read from the `best-matches` or `found-jobs` resource";

/// Pure core of the `job` resource: find the first `FoundJob` across every
/// (non-filtered — every status, not just active) autopilot record whose
/// identity matches, then project it. Mirrors `applied_check::
/// resolve_applied_check`'s pure/impure split — directly unit-testable with
/// hand-built `Autopilot` records, no `AppHandle`.
///
/// Two independent compares, either one wins (issue #1166):
///
/// 1. **Identity** — `caller_identity` (already extracted from the raw
///    caller url by [`job_resource`] via
///    [`crate::scraping::scrape_url::job_identity`]) against the SAME
///    extraction run on each stored url. This is what makes
///    `de.linkedin.com/jobs/view/<id>`, `www.linkedin.com/jobs/view/<id>`,
///    the numeric-only and slugged `/jobs/view/` forms, and the
///    `currentJobId=<id>` query form all resolve to one posting — none of
///    that is a byte-for-byte url difference the string compare below could
///    ever bridge.
/// 2. **Normalized string** — the pre-#1166 fallback, unchanged, for boards
///    with no stable id space.
///
/// Both sides of BOTH compares run through
/// [`decode_unreserved`](crate::applications::decode_unreserved) first (issue
/// #1128): a STORED url can carry the percent-encoded spelling just as easily
/// as a caller-supplied one, so decoding only the caller's half would fix the
/// reported direction and leave the mirror image broken. `normalized_url` is
/// pre-decoded by [`job_resource`]; this is the stored half.
///
/// `applied_urls` is [`crate::commands::autopilot::applied_job_urls`]'s
/// output (issue #1166/#1169, HIGH — before this fix `AgentJob::applied` was
/// a plain passthrough of `FoundJob::applied`, whose own doc says the stored
/// value is ALWAYS `false` and only the read path ever fills it in; the two
/// read paths that DO fill it in — `commands::autopilot::enrich_applied` and
/// `found_jobs::project_found_job_row` — never ran on this one, so `job`
/// reported every posting as not-applied even after a real application
/// existed, the exact duplicate-application hazard this surface exists to
/// prevent). Derived here through [`job_is_applied`], the SAME identity-aware
/// helper `found_jobs::candidate_jobs` derives its own `applied` from (round-4
/// fix T4 — before this, the two surfaces disagreed the moment an
/// application was recorded under a different host/path spelling than the
/// one currently stored on the found job), off the SAME set, so `job` and
/// `found-jobs` agree by construction on one url.
///
/// Assumes the applications store is present; see
/// [`resolve_job_for_store`] for the store-unavailable path (round-4 fix T3).
/// `job_resource` calls [`resolve_job_for_store`] directly (it always knows
/// whether the store is present) — this default-store wrapper exists only so
/// the many existing store-present tests keep their original call shape.
#[cfg(test)]
pub(in crate::extension_bridge::agent_read) fn resolve_job(
    records: &[crate::autopilot::Autopilot],
    caller_identity: Option<(&'static str, String)>,
    normalized_url: &str,
    applied_urls: &std::collections::HashSet<String>,
) -> AppResult<Value> {
    resolve_job_for_store(records, caller_identity, normalized_url, applied_urls, true)
}

/// Whether `job_url` (a `FoundJob`'s own RAW, never-normalized url) counts as
/// applied against `applied_urls` (`commands::autopilot::applied_job_urls`'s
/// already-normalized set) — round-4 fix T4. Byte-comparing two normalized
/// strings misses a LinkedIn regional host (`de.linkedin.com` vs a stored
/// `linkedin.com`) or a slugged `/jobs/view/` path against a bare numeric
/// one, the SAME identity gap #1166 closed for `resolve_job`'s own posting
/// lookup. Tries [`crate::scraping::scrape_url::job_identity`] first (a board
/// with a stable id space folds every host/path variant onto one id) and
/// falls back to the plain normalized-string compare for a board with none.
/// Shared by [`resolve_job_for_store`] and `found_jobs::candidate_jobs` so
/// the two surfaces can never disagree about the same job.
pub(in crate::extension_bridge::agent_read) fn job_is_applied(
    job_url: &str,
    applied_urls: &std::collections::HashSet<String>,
) -> bool {
    let decoded = crate::applications::decode_unreserved(job_url);
    // Check BOTH the raw and the unreserved-decoded spelling (round-4 fix
    // T4-cont — `applied_urls` is keyed by `normalize_job_url(raw)`, never
    // decoded per that fn's own doc, so a job whose stored url and recorded
    // application agree on a percent-escaped spelling, e.g. `%2D`, only
    // matched when the raw side was compared too; decoding first turned an
    // exact-spelling match into a miss on any board `job_identity` doesn't
    // cover).
    if applied_urls.contains(&crate::applications::normalize_job_url(job_url))
        || applied_urls.contains(&crate::applications::normalize_job_url(&decoded))
    {
        return true;
    }
    let Some(identity) = crate::scraping::scrape_url::job_identity(&decoded) else {
        return false;
    };
    applied_urls.iter().any(|stored| {
        let stored_decoded = crate::applications::decode_unreserved(stored);
        crate::scraping::scrape_url::job_identity(&stored_decoded).as_ref() == Some(&identity)
    })
}

/// Precompute [`job_identity`](crate::scraping::scrape_url::job_identity) for
/// every entry in `applied_urls`, once — round-4 perf fix (PR #1182 round-5):
/// [`found_jobs::candidate_jobs`] calls the identity fallback below once per
/// STORED row, and re-decoding + re-parsing the whole `applied_urls` set on
/// every one of those calls was O(found jobs × applications) `Url::parse` +
/// allocation, ahead of `limit` ever applying. [`job_is_applied_indexed`]
/// takes this index instead of re-deriving it; [`job_is_applied`] (the
/// single-lookup `job` resource path, called once per call, never in a loop)
/// keeps its own inline scan — building an index there would cost the same
/// as the scan it replaces.
pub(in crate::extension_bridge::agent_read) fn applied_url_identities(
    applied_urls: &std::collections::HashSet<String>,
) -> std::collections::HashSet<(&'static str, String)> {
    applied_urls
        .iter()
        .filter_map(|stored| {
            let decoded = crate::applications::decode_unreserved(stored);
            crate::scraping::scrape_url::job_identity(&decoded)
        })
        .collect()
}

/// Same contract as [`job_is_applied`], but takes a precomputed
/// [`applied_url_identities`] index instead of re-deriving one per call —
/// see that fn's own doc for why. Must stay behaviourally identical to
/// `job_is_applied` for the same inputs; `found_jobs::tests` pins the two
/// against each other.
pub(in crate::extension_bridge::agent_read) fn job_is_applied_indexed(
    job_url: &str,
    applied_urls: &std::collections::HashSet<String>,
    applied_identities: &std::collections::HashSet<(&'static str, String)>,
) -> bool {
    let decoded = crate::applications::decode_unreserved(job_url);
    if applied_urls.contains(&crate::applications::normalize_job_url(job_url))
        || applied_urls.contains(&crate::applications::normalize_job_url(&decoded))
    {
        return true;
    }
    let Some(identity) = crate::scraping::scrape_url::job_identity(&decoded) else {
        return false;
    };
    applied_identities.contains(&identity)
}

/// [`resolve_job`] plus the store-unavailable path (round-4 fix T3): when
/// `store_present` is `false`, the caller's `applied_urls` is unconditionally
/// empty (`applied_job_urls`'s own doc — a missing store collapses to "the
/// user has applied to nothing"), so reporting `applied: false` from it would
/// be a confident, WRONG answer for the unsafe direction — an autonomous
/// caller could re-apply to a job it already applied to. Omitting the key
/// (absent ≠ false) plus a `appliedUnavailable: true` marker lets a caller
/// tell "definitely not applied" from "cannot tell right now" apart.
pub(in crate::extension_bridge::agent_read) fn resolve_job_for_store(
    records: &[crate::autopilot::Autopilot],
    caller_identity: Option<(&'static str, String)>,
    normalized_url: &str,
    applied_urls: &std::collections::HashSet<String>,
    store_present: bool,
) -> AppResult<Value> {
    let found = records
        .iter()
        .find_map(|ap| {
            ap.found_jobs.iter().find(|j| {
                let decoded = crate::applications::decode_unreserved(&j.url);
                if let Some(caller) = &caller_identity {
                    if crate::scraping::scrape_url::job_identity(&decoded).as_ref() == Some(caller)
                    {
                        return true;
                    }
                }
                crate::applications::normalize_job_url(&decoded) == normalized_url
            })
        })
        .ok_or_else(|| AppError::Validation(JOB_NOT_FOUND_MESSAGE.to_string()))?;
    let mut value = project_value::<_, AgentJob>(found)
        .ok_or_else(|| AppError::Message("failed to project job".to_string()))?;
    if store_present {
        let is_applied = job_is_applied(&found.url, applied_urls);
        value["applied"] = json!(is_applied);
    } else if let Value::Object(map) = &mut value {
        map.remove("applied");
        map.insert("appliedUnavailable".to_string(), json!(true));
    }
    fence_description(&mut value);
    super::super::best_matches::fence_posting_display_fields(&mut value);
    Ok(value)
}

/// Fence `description` in place — this is raw, uncapped, third-party-authored
/// scraped text handed to a consumer whose entire purpose is "an AI agent
/// reads this" (ADR-010, HIGH — security review). `answer_assist.rs`'s
/// `build_user_message` fences the IDENTICAL string for the identical
/// reason; this is the same primitive, the same cap, the same tag, so a
/// scraped posting reads as untrusted DATA (never instructions) on every
/// surface it reaches. `title`/`company`/`location` share this provenance —
/// the follow-up this doc once deferred landed as
/// [`best_matches::fence_posting_display_fields`], called separately by both this fn's own
/// caller ([`resolve_job`]) and `best_matches::resolve_best_matches` (one row at a time,
/// per [`best_matches::fence_posting_display_fields`]'s own doc).
fn fence_description(value: &mut Value) {
    let Some(desc) = value.get("description").and_then(Value::as_str) else {
        return;
    };
    let fenced = crate::prompt_fence::fenced("job_posting", desc, crate::prompt_fence::JOB_CAP);
    value["description"] = json!(fenced);
}
