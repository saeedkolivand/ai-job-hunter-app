//! `found-jobs` resource (issue #1115) — split out of `agent_read`'s own module under R8's hard
//! LOC cap. Private to `agent_read`; only the two `limit` constants reach past it, to
//! `agent_cli::mcp`'s tool schema (issue #1129).
//!
//! ## Compact rows + server-side filters (issue #1167)
//! A row is compact by default (no `description`) since the old always-detailed shape put an
//! ordinary page over what a real MCP client accepts in-band; `description` is opt-in via
//! `includeDescription: true`. Five server-side filters (`minScore`/`country`/`remote`/`applied`/
//! `query`) apply BEFORE paging, so `total` always means "rows this call's filters actually
//! match".
//!
//! ## Spanning every autopilot (issue #1168)
//! `autopilotId` is optional; omitted, the traversal spans every autopilot in store order. Rows
//! sharing the same [`canonical_job_key`](crate::scraping::boards::common::canonical_job_key)
//! collapse to the FIRST occurrence that also PASSES this call's filters (B3-r1-F1 — filtering
//! before dedup, or a posting failing one autopilot's filter could consume the slot a later,
//! passing copy needed). The cursor is `<issuer>:<offset>`, `issuer` being
//! `<autopilotId or __all__>|<filter fingerprint>` (B3-r1-F4, see [`found_jobs_cursor_issuer`]) —
//! valid only for a later call with the SAME scope AND filters.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;

use super::list_autopilots;
use crate::error::AppResult;
use crate::extension_bridge::paging;

/// `found-jobs` resource's per-row COMPACT payload — a SMALLER allowlist than `agent_read::AgentJob`
/// over the same `autopilot::FoundJob` source: also excludes `board`/`salaryMin`/`salaryMax`/
/// `salaryCurrency`/`scoreSource`/`postedAt`/`trust`/`clusterMembers` (issue #1167 — a caller
/// wanting full detail already has `job`, keyed by this same `url`). `applied`/`autopilotId`/
/// `autopilotName`/`description` are NOT part of this struct's serde round trip —
/// [`project_found_job_row`] injects them afterward, since none is a plain passthrough.
///
/// `score_provisional` stays IN (B3-r1-F5) — this is the one resource that FILTERS by `minScore`,
/// and a title-only/aggregator-snippet score is flagged provisional so a filtered caller doesn't
/// treat it as fully trusted.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FoundJobSlice {
    title: String,
    company: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f64>,
    score_provisional: bool,
    found_at: u64,
    is_agency: bool,
}

/// `description`'s fence cap for `found-jobs`, distinct from `crate::prompt_fence::JOB_CAP` — a
/// caller needing the full posting text already has `job`, keyed by this row's own `url`. 2,000
/// chars (up from 500 — pre-PR review round 2: 500 was mostly boilerplate, not enough to actually
/// qualify/dismiss a posting). Only applies when a caller opts in via `includeDescription: true`.
const FOUND_JOBS_DESCRIPTION_PREVIEW_CAP: usize = 2_000;

/// Server-side default/cap for `found-jobs`' `limit` — a CEILING on work per call, never the
/// transport-size guarantee (that's [`PAGE_BYTE_BUDGET`]). `pub(in crate::extension_bridge)`
/// (issue #1129) so `agent_cli::mcp` derives its tool schema's `limit` description from these
/// numbers instead of a hand-typed copy that can drift (as the advertised 50/100 once did from the
/// enforced 25/50).
pub(in crate::extension_bridge) const DEFAULT_FOUND_JOBS_LIMIT: usize = 25;
pub(in crate::extension_bridge) const MAX_FOUND_JOBS_LIMIT: usize = 50;

/// The REAL per-response safety net (pre-PR review round 2, HIGH — a row-count limit alone cannot
/// bound a page's byte size, since a legitimate posting's title/company/location can each reach
/// `crate::prompt_fence::JOB_CAP` = 8,000 chars). [`trim_page_to_budget`] checks the ACTUAL
/// serialized bytes and drops rows from the end. Target: half of
/// `agent_cli::mcp::MCP_RESULT_MAX_BYTES`, leaving margin for the MCP transport wrapper —
/// [`trim_page_to_budget`]'s `base_cost` accounts for the rest of this resource's own envelope.
const PAGE_BYTE_BUDGET: usize = 150_000;

/// Cap on `autopilotName` before it enters the response envelope or a row
/// (CodeRabbit finding, PR #1117 review — an autopilot's name is user-typed
/// and unbounded). 200 chars is generous for the short single-line name the
/// CreationWizard collects, while making the cap a CONCRETE bound rather
/// than "trust the UI never lets this grow" — a migrated/imported record
/// could still carry something longer.
const AUTOPILOT_NAME_CAP: usize = 200;

/// Sentinel cursor issuer for a traversal spanning EVERY autopilot (issue
/// #1168 — `autopilotId` is optional). Never a value
/// [`Uuid::new_v4`](uuid::Uuid::new_v4) (the real id generator, see
/// `Autopilot::create`) can produce, so it can never collide with a real
/// autopilot id and be misread as a scoped cursor.
pub(super) const ALL_AUTOPILOTS_CURSOR_ISSUER: &str = "__all__";

/// Cap `name` to [`AUTOPILOT_NAME_CAP`] chars, char-boundary safe, and neutralize any forged
/// transcript boundary (issue #1157 — an autopilot name is the CALLER'S OWN first-party data,
/// never board-scraped, so it no longer gets [`crate::prompt_fence::fenced`]'s `job_posting`
/// wrapper the way a `job` row's real third-party `title`/`company`/`location` does).
/// `.chars().take(n)` rather than a byte slice — slicing at an arbitrary byte offset can land
/// mid-codepoint and panic (`panic = "abort"` in release).
///
/// AC-5 MEDIUM: dropping the `fenced` wrapper also dropped its boundary defence, not only its
/// label — `jobs[].description` carries real `<job_posting>` fences in the SAME response, so a
/// name containing `</job_posting>` would be a forged boundary. Neutralizing restores that half
/// without re-adding the label the issue asked to remove.
pub(super) fn cap_autopilot_name(name: &str) -> String {
    let capped: String = name.chars().take(AUTOPILOT_NAME_CAP).collect();
    crate::prompt_fence::neutralize_transcript_boundaries(&capped)
}

/// This resource's own default/max applied to the shared clamp — the numbers
/// are resource-specific (sized against THIS row shape), the clamping rule is
/// not (`extension_bridge::paging`).
fn clamp_found_jobs_limit(payload: &Value) -> usize {
    paging::clamp_limit(payload, DEFAULT_FOUND_JOBS_LIMIT, MAX_FOUND_JOBS_LIMIT)
}

/// This resource's own [`PAGE_BYTE_BUDGET`] applied to the shared trim
/// (`extension_bridge::paging::trim_to_byte_budget`, which carries the full
/// rationale and the forward-progress guarantee). Named differently from the
/// primitive it wraps ON PURPOSE (backend-architect review): a wrapper that
/// shares its callee's name but takes one fewer argument reads like an
/// overload at every call site, and shadows the real thing inside this module.
pub(super) fn trim_page_to_budget(candidates: Vec<Value>, base_cost: usize) -> Vec<Value> {
    paging::trim_to_byte_budget(candidates, base_cost, PAGE_BYTE_BUDGET)
}

/// Every envelope byte OTHER than the `jobs` array itself, measured (not assumed) against the REAL
/// `autopilotId`/[`cap_autopilot_name`]-capped `autopilotName` — the `base_cost`
/// [`trim_page_to_budget`] subtracts from [`PAGE_BYTE_BUDGET`]. `None` when spanning every
/// autopilot (issue #1168), which carries neither envelope field. Via
/// [`super::envelope_cost_estimate`] — see that fn's own doc for why `nextCursor`'s placeholder is
/// safe to over-count against.
pub(super) fn base_envelope_cost(
    cursor_issuer: &str,
    single: Option<(&str, &str)>,
    total: usize,
) -> usize {
    let mut base_envelope = json!({
        "jobs": [],
        "nextCursor": format!("{cursor_issuer}:{total}"),
        "total": total,
    });
    if let Some((id, name)) = single {
        base_envelope["autopilotId"] = json!(id);
        base_envelope["autopilotName"] = json!(name);
    }
    super::envelope_cost_estimate(&base_envelope)
}
pub(super) fn fence_found_jobs_description(value: &mut Value) {
    let Some(desc) = value.get("description").and_then(Value::as_str) else {
        return;
    };
    let fenced =
        crate::prompt_fence::fenced("job_posting", desc, FOUND_JOBS_DESCRIPTION_PREVIEW_CAP);
    value["description"] = json!(fenced);
}

mod cursor;
mod filters;
mod resolve;

use cursor::{
    check_applied_filter_available, found_jobs_cursor_issuer, parse_autopilot_id_arg,
    parse_found_jobs_cursor,
};
#[cfg(test)]
pub(super) use cursor::{
    APPLIED_FILTER_UNAVAILABLE_MESSAGE, AUTOPILOT_NOT_FOUND_MESSAGE, BLANK_AUTOPILOT_ID_MESSAGE,
    MALFORMED_CURSOR_MESSAGE, WRONG_AUTOPILOT_CURSOR_MESSAGE,
};
pub(super) use filters::{trimmed_lowercase_filter, FoundJobsFilters};
#[cfg(test)]
pub(super) use resolve::resolve_found_jobs;
use resolve::resolve_found_jobs_for_store;

pub(super) fn found_jobs_resource(app: &AppHandle, payload: &Value) -> AppResult<Value> {
    let autopilot_id = parse_autopilot_id_arg(payload)?;
    let filters = FoundJobsFilters::from_payload(payload)?;
    // `store_present` derives from the SAME checked read as `applied_urls`
    // (round-4 fix T3-cont) — a `try_state().is_some()` alone can't tell a
    // managed-but-unreadable store from a genuinely-empty one; see
    // `commands::autopilot::applied_job_urls_checked`'s own doc.
    let applied = crate::commands::autopilot::applied_job_urls_checked(app);
    let store_present = applied.is_some();
    check_applied_filter_available(store_present, &filters)?;
    let cursor_issuer = found_jobs_cursor_issuer(autopilot_id.as_deref(), &filters);
    let offset = parse_found_jobs_cursor(payload, &cursor_issuer)?;
    let limit = clamp_found_jobs_limit(payload);
    let records = list_autopilots(app)?;
    let applied_urls = applied.unwrap_or_default();
    resolve_found_jobs_for_store(
        &records,
        autopilot_id.as_deref(),
        &filters,
        &applied_urls,
        offset,
        limit,
        store_present,
    )
}

#[cfg(test)]
mod tests;
