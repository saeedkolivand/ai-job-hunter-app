//! `best-matches` resource — allowlist projection, cursor paging, query filter, and the
//! byte-budget trim. Split out of `agent_read.rs` (R8 relief, PR4 — the Prep tab addition pushed
//! that module toward the hard LOC cap) — same pattern as `found_jobs.rs`/`documents.rs`:
//! behaviourally identical, only the file it lives in moved. `pub(super)` throughout (`super` =
//! `agent_read`) so `agent_read.rs` (which still owns the dispatch match and
//! `resolve_job_for_store`, `fence_posting_display_fields`'s OTHER caller) reaches every name it
//! needs via the qualified `best_matches::name` path — the same convention `found_jobs.rs`'s own
//! items already use, no blanket re-export. `DEFAULT_BEST_MATCHES_LIMIT`/`MAX_BEST_MATCHES_LIMIT`
//! go one step further, `pub(in crate::extension_bridge)`, so `agent_cli::mcp` can derive its tool
//! schema from them the same way it already does for `found_jobs`'s own pair.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::error::AppResult;

use super::AgentTrust;

/// One contributing autopilot on a `best-matches` row — projected off
/// `commands::autopilot::best_matches::BestMatchSource`'s wire shape. Its own
/// explicit field set (not the source type reused verbatim) is what gives
/// this the SAME nested allowlist guarantee [`AgentTrust`] exists for — a
/// field added to `BestMatchSource` tomorrow is absent here BY CONSTRUCTION,
/// pinned by `best_match_projection_has_exact_keys`' descent into `sources`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentBestMatchSource {
    autopilot_id: String,
    autopilot_name: String,
    paused: bool,
    found_at: u64,
}

/// `best-matches` resource's per-row payload — projected off
/// `commands::autopilot::best_matches::BestMatchRow`'s wire (JSON) shape.
/// Excludes `key` (an opaque cluster id — see the module doc),
/// `assistantNotes` (forbidden), and `clusterMembers` (grouping detail with
/// no meaning off this surface, same call as `AgentJob`'s).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentBestMatch {
    title: String,
    company: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    board: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salary_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salary_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salary_currency: Option<String>,
    score: f64,
    score_source: crate::autopilot::ScoreSource,
    score_provisional: bool,
    /// Mirrors `commands::autopilot::best_matches::BestMatchRow::score_url`
    /// field-for-field (issue #1106/#1104 cross-scope fix): present only when
    /// `score`/`scoreSource`/`scoreProvisional` belong to a DIFFERENT cluster
    /// member than the one `url` names — see that field's own doc for why a
    /// row's displayed score and displayed url aren't always the same real
    /// posting. Passthrough, not forbidden — no fencing needed (it's a url,
    /// not free text).
    #[serde(skip_serializing_if = "Option::is_none")]
    score_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    posted_at: Option<i64>,
    found_at: u64,
    applied: bool,
    is_agency: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    trust: Option<AgentTrust>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sources: Vec<AgentBestMatchSource>,
}

mod cursor;

pub(super) use cursor::{
    best_matches_cursor_issuer, clamp_best_matches_limit, parse_best_matches_args,
};
// Test-only from this module's own perspective — `parse_best_matches_cursor` is called
// internally within `cursor.rs`; `agent_read::tests::best_matches_paging` reaches it and the two
// refusal sentinels only via this qualified `best_matches::name` re-export.
#[cfg(test)]
pub(super) use cursor::{
    parse_best_matches_cursor, BEST_MATCHES_MALFORMED_CURSOR_MESSAGE,
    BEST_MATCHES_WRONG_QUERY_CURSOR_MESSAGE,
};
pub(in crate::extension_bridge) use cursor::{DEFAULT_BEST_MATCHES_LIMIT, MAX_BEST_MATCHES_LIMIT};

/// Pure core of `best-matches`: project, optionally `query`-filter, then
/// `offset`/`limit`-page an already-computed row set (issue #1146 P11 — the
/// same cursor `found-jobs` already has, reusing `extension_bridge::paging`'s
/// clamp/cursor primitives at the call site). Directly unit-testable with
/// hand-built `Value` rows, no `AppHandle` — the impure half
/// ([`best_matches_resource`]) only resolves
/// `commands::autopilot::autopilot_best_matches`'s output.
///
/// `total` here is the count of rows THIS call's `query` actually matches —
/// never the command's own pre-cap qualifying count
/// (`commands::autopilot::best_matches::BestMatchesOutcome::total`, which
/// this fn never receives): `rows` itself is already capped at
/// `BEST_MATCHES_CAP` upstream, so a caller paging this cursor to `null`
/// only ever reaches what `rows` actually holds — reporting the pre-cap
/// number here would promise a page count this traversal cannot deliver.
/// Raising that upstream cap is a job-matching-domain change, out of scope
/// here.
///
/// `nextCursor` is `<query fingerprint>:<offset>` (round 2 fix, B3-r1-F4),
/// not a bare offset — `query` here is already the SAME normalized value
/// [`best_matches_resource`] fingerprinted to parse the incoming `offset`,
/// so both halves of the format always agree.
///
/// [`trim_best_matches_page_to_budget`]s the fenced page before returning it
/// (issue #1165, HIGH — the sibling `found-jobs` resource added this exact
/// guard for the exact same reason: a row-count `limit` alone cannot bound a
/// page's byte size, since a legitimate, non-adversarial posting's
/// `title`/`company`/`location` can each independently reach
/// `crate::prompt_fence::JOB_CAP` = 8,000 chars). Each row is fenced (per
/// [`fence_posting_display_fields`]) BEFORE trimming, not after, so the
/// bytes the budget measures are the exact bytes that leave the process.
pub(super) fn resolve_best_matches(
    rows: &[Value],
    offset: usize,
    limit: usize,
    query: Option<&str>,
) -> Value {
    let mut matches: Vec<AgentBestMatch> = rows
        .iter()
        .filter_map(|row| serde_json::from_value(row.clone()).ok())
        .collect();
    if let Some(q) = query {
        matches
            .retain(|m| m.title.to_lowercase().contains(q) || m.company.to_lowercase().contains(q));
    }
    let total = matches.len();
    let cursor_issuer = best_matches_cursor_issuer(query);
    let page_values: Vec<Value> = matches
        .into_iter()
        .skip(offset)
        .take(limit)
        .filter_map(|m| serde_json::to_value(m).ok())
        .map(|mut row| {
            fence_posting_display_fields(&mut row);
            row
        })
        .collect();
    let base_cost = best_matches_base_envelope_cost(&cursor_issuer, total);
    let page = trim_best_matches_page_to_budget(page_values, base_cost);
    let returned = page.len();
    let next_offset = offset + returned;
    let next_cursor = if next_offset < total {
        Some(format!("{cursor_issuer}:{next_offset}"))
    } else {
        None
    };
    json!({
        "matches": page,
        "total": total,
        "returned": returned,
        "nextCursor": next_cursor,
    })
}

/// Fence `title`/`company`/`location` on ONE object — shared by
/// [`resolve_best_matches`] (one call per `best-matches` row) and
/// `agent_read::resolve_job_for_store` (one call on the single job object),
/// so the identical primitive/tag/cap can never drift between the two
/// curated-tier surfaces that both carry these fields (MUST FIX — pre-PR
/// gate: `resolve_job` used to call only `fence_description`, leaving
/// `job`'s own title/company/location bare while `best-matches` and the
/// generic tier's own `agent_call::FENCE_FIELD_NAMES` both fenced them —
/// same threat, same session, one hole).
pub(super) fn fence_posting_display_fields(value: &mut Value) {
    for field in ["title", "company", "location"] {
        if let Some(s) = value.get(field).and_then(Value::as_str) {
            let fenced =
                crate::prompt_fence::fenced("job_posting", s, crate::prompt_fence::JOB_CAP);
            value[field] = json!(fenced);
        }
    }
}

/// The REAL per-response safety net for `best-matches` (issue #1165) —
/// mirrors `found_jobs::PAGE_BYTE_BUDGET`'s own target one resource over:
/// half of `agent_cli::mcp::MCP_RESULT_MAX_BYTES` (256 KiB), leaving real
/// margin for the MCP `content[]`/`isError` wrapper this payload rides
/// inside on the MCP transport.
const BEST_MATCHES_PAGE_BYTE_BUDGET: usize = 150_000;

/// This resource's own [`BEST_MATCHES_PAGE_BYTE_BUDGET`] applied to the
/// shared trim (`extension_bridge::paging::trim_to_byte_budget`, which
/// carries the full rationale and the forward-progress guarantee). Named
/// differently from the primitive it wraps for the same reason
/// `found_jobs::trim_page_to_budget` is.
fn trim_best_matches_page_to_budget(candidates: Vec<Value>, base_cost: usize) -> Vec<Value> {
    crate::extension_bridge::paging::trim_to_byte_budget(
        candidates,
        base_cost,
        BEST_MATCHES_PAGE_BYTE_BUDGET,
    )
}

/// Every envelope byte OTHER than `matches` itself — mirrors
/// `found_jobs::base_envelope_cost`'s own reasoning one resource over.
/// `nextCursor` isn't known until after trimming, so it's measured in the
/// SAME `<issuer>:<offset>` shape a real cursor has, with `total` standing in
/// for both the offset and `returned` — a real offset/returned count can
/// never exceed `total`, so this can only ever OVER-count and thus only trim
/// MORE aggressively than strictly required, never less (the safe direction
/// for a byte budget).
fn best_matches_base_envelope_cost(cursor_issuer: &str, total: usize) -> usize {
    let base_envelope = json!({
        "matches": [],
        "total": total,
        "returned": total,
        "nextCursor": format!("{cursor_issuer}:{total}"),
    });
    serde_json::to_string(&base_envelope)
        .map_or(usize::MAX, |s| s.len())
        .saturating_sub(2)
}

pub(super) async fn best_matches_resource(app: &AppHandle, payload: &Value) -> AppResult<Value> {
    let limit = clamp_best_matches_limit(payload);
    let (query, offset) = parse_best_matches_args(payload)?;
    let raw = crate::commands::autopilot::autopilot_best_matches(app.clone()).await;
    let rows = raw
        .get("matches")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(resolve_best_matches(&rows, offset, limit, query.as_deref()))
}
