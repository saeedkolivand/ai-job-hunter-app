//! `autopilot_list` as a summary on the agent surface (#1495) — the renderer's own wire shape,
//! which needs both fields, is untouched.

use serde_json::Value;

/// The command whose reply is summarised.
pub(in crate::extension_bridge::agent_call) const AUTOPILOT_LIST_COMMAND: &str = "autopilot_list";

/// Replaces `foundJobs` (every stored posting, descriptions included) with `foundJobsTotal`, and
/// `resumeText` (up to hundreds of KB per row) with `hasResumeText`, and drops `coverLetter`
/// (also unbounded), on every row of an
/// `autopilot_list` reply. A handful of autopilots otherwise blows the MCP output cap. The jobs
/// themselves stay reachable through the `found-jobs` resource; one full record through
/// `autopilot_get`. Any other command, or a non-array reply, is left alone.
pub(in crate::extension_bridge::agent_call) fn summarize_autopilot_list(
    command: &str,
    data: &mut Value,
) {
    if command != AUTOPILOT_LIST_COMMAND {
        return;
    }
    let Value::Array(rows) = data else { return };
    for row in rows.iter_mut().filter_map(Value::as_object_mut) {
        let found = row
            .remove("foundJobs")
            .and_then(|v| v.as_array().map(Vec::len))
            .unwrap_or(0);
        let has_resume = row
            .remove("resumeText")
            .and_then(|v| v.as_str().map(|s| !s.trim().is_empty()))
            .unwrap_or(false);
        row.remove("coverLetter");
        row.insert("foundJobsTotal".into(), found.into());
        row.insert("hasResumeText".into(), has_resume.into());
    }
}
