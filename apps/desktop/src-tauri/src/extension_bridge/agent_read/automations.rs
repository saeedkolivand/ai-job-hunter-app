//! The `automations` resource — split out of `agent_read.rs` under the R8 LOC cap.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::error::AppResult;

use super::list_autopilots;

/// `automations` resource's per-row payload — projected off `autopilot::Autopilot`. Excludes
/// `resumeText`/`coverLetter`/`assistant`/`assistantProvider`/`assistantModel`/`assistantBaseUrl`/
/// `foundJobs`/`lastRunSummaries`/`totalApplied` — the first four forbidden outright, the next two
/// out of scope for a status listing, and `totalApplied` dropped (issue #1171): the field is dead
/// on the source struct too — nothing ever writes it past its zero default.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::extension_bridge::agent_read) struct AgentAutomation {
    /// Reads off the source's `_id` (see `Autopilot::id`'s `#[serde(rename)]`)
    /// but serializes back out as plain `id` — the agent wire format has no
    /// reason to carry the on-disk Mongo-style key name.
    #[serde(alias = "_id")]
    id: String,
    name: String,
    status: crate::autopilot::AutopilotStatus,
    target: AgentAutomationTarget,
    /// Jobs the MOST RECENT run kept after filtering — `AutopilotStore::record_run`
    /// OVERWRITES `Autopilot::total_found` on every run, so this is a per-run
    /// figure, never a cumulative one, and it can be far smaller than the stored
    /// list (issue #1132). Any surface describing this resource points HERE for
    /// the meaning of the two counts rather than restating it.
    total_found: u32,
    /// The traversable count: how many jobs this autopilot has stored across
    /// every run, i.e. exactly how many `found-jobs` will page through
    /// (`found_jobs::resolve_found_jobs`' own `total` — the same
    /// `found_jobs.len()` expression, so the two surfaces agree by construction).
    /// This is the number a caller asking "how many jobs did this find?" wants.
    found_jobs_total: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_status: Option<crate::autopilot::RunStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_run_at: Option<u64>,
    created_at: u64,
    updated_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::extension_bridge::agent_read) struct AgentAutomationTarget {
    boards: Vec<String>,
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
}

/// Direct field-by-field projection — NOT [`project_value`]'s serialize-then-deserialize round
/// trip (MEDIUM fix, security review): that round-trips the WHOLE source through JSON, serializing
/// `found_jobs`/`resume_text`/`cover_letter` just to discard the result. **Measured** (debug build,
/// 50 autopilots × 1000 found jobs each): the round trip cost ~320ms against ~1ms for this direct
/// construction. Trivial against the 1-req/sec refill this bucket already enforces, but pure waste
/// for a resource that already knows exactly which fields it wants. `job`'s own `project_value`
/// call stays unchanged — it projects ONE already-found `FoundJob`, never the whole store.
pub(in crate::extension_bridge::agent_read) fn project_automation(
    ap: &crate::autopilot::Autopilot,
) -> AgentAutomation {
    AgentAutomation {
        id: ap.id.clone(),
        name: ap.name.clone(),
        status: ap.status.clone(),
        target: AgentAutomationTarget {
            boards: ap.target.boards.clone(),
            query: ap.target.query.clone(),
            location: ap.target.location.clone(),
        },
        total_found: ap.total_found,
        found_jobs_total: ap.found_jobs.len() as u32,
        run_status: ap.run_status.clone(),
        last_run_at: ap.last_run_at,
        created_at: ap.created_at,
        updated_at: ap.updated_at,
    }
}

pub(in crate::extension_bridge::agent_read) fn resolve_automations(
    records: &[crate::autopilot::Autopilot],
) -> Value {
    let automations: Vec<Value> = records
        .iter()
        .filter_map(|ap| serde_json::to_value(project_automation(ap)).ok())
        .collect();
    json!({ "automations": automations })
}

pub(super) fn automations_resource(app: &AppHandle) -> AppResult<Value> {
    Ok(resolve_automations(&list_autopilots(app)?))
}
