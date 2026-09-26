//! The `job` resource: allowlist projections shared with `best_matches.rs`, and the impure
//! caller-side identity/dispatch shell — split out of `agent_read.rs` under the R8 LOC cap. The
//! pure resolve core lives in `job/resolve.rs`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;

use crate::error::{AppError, AppResult};

use super::list_autopilots;

pub(super) mod resolve;

use resolve::resolve_job_for_store;

// ── Allowlist projections ───────────────────────────────────────────────────

/// Round-trip `source` through JSON into `T` — `T`'s field set IS the
/// allowlist. See the module doc for why this is what makes a forbidden key
/// absent BY CONSTRUCTION rather than by remembering to omit it.
fn project<S, T>(source: &S) -> Option<T>
where
    S: Serialize,
    T: serde::de::DeserializeOwned,
{
    serde_json::to_value(source)
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
}

/// [`project`], then re-serialize to a plain [`Value`] for the wire.
pub(in crate::extension_bridge::agent_read) fn project_value<S, T>(source: &S) -> Option<Value>
where
    S: Serialize,
    T: Serialize + serde::de::DeserializeOwned,
{
    project::<S, T>(source).and_then(|t| serde_json::to_value(t).ok())
}

/// One cluster member, projected off `scraping::cluster::ClusterMemberRef` —
/// drops `key` (an opaque cluster id, not a usable identity off this surface;
/// see the module doc's "`url` is the cross-resource key" note).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentClusterMember {
    #[serde(skip_serializing_if = "Option::is_none")]
    board: Option<String>,
    url: String,
}

/// Projection of `scraping::trust::TrustAssessment` — nested inside both
/// `AgentJob` and `AgentBestMatch`. A dedicated allowlist struct, not the
/// source type reused verbatim (MEDIUM fix — security review): `project`'s
/// "absent by construction" guarantee only holds at the TOP level of its
/// round trip. A field added to `TrustAssessment` tomorrow would otherwise
/// ride straight through — this struct's own explicit field set is what
/// makes the SAME guarantee hold one level down.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::extension_bridge::agent_read) struct AgentTrust {
    score: u8,
    level: crate::scraping::trust::TrustLevel,
    flags: Vec<crate::scraping::trust::TrustFlag>,
}

/// `job` resource payload — projected off `autopilot::FoundJob`. Excludes
/// `assistantNotes` (forbidden), `clusterId`/`clusterCanonical` (internal
/// grouping detail with no meaning off this surface).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::extension_bridge::agent_read) struct AgentJob {
    title: String,
    company: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    board: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salary_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salary_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salary_currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f64>,
    score_provisional: bool,
    score_source: crate::autopilot::ScoreSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    posted_at: Option<i64>,
    found_at: u64,
    is_new: bool,
    /// NOT a plain passthrough of the stored `FoundJob::applied` (issue #1166/#1169) — that
    /// field's own doc says the stored value is ALWAYS `false`. [`resolve_job`] overwrites this
    /// with a value derived off `applied_job_urls`, the same set `found_jobs`/`best_matches`
    /// derive theirs from.
    applied: bool,
    is_agency: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    trust: Option<AgentTrust>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cluster_members: Vec<AgentClusterMember>,
}

/// The CALLER side of `job`'s identity pipeline, extracted from [`job_resource`] so it is
/// unit-testable without an `AppHandle` — the counterpart to [`resolve_job`]'s stored side (issue
/// #1128). Empty means "not a usable http(s) url", exactly as `normalize_job_url` reports it.
///
/// Same canonicalize-then-normalize pipeline `applied.check`/`answers.save` use, plus an
/// unreserved-only decode FIRST, so the canonicalizer reads the real path: a `%2D`-spelled
/// LinkedIn slug is byte-different but semantically identical (RFC 3986 §6.2.2.2). The scheme
/// guard still runs AFTER the decode, inside `normalize_job_url`, so `%6Aavascript:…` is caught
/// rather than smuggled past a raw-byte check.
///
/// That decode makes this READ deliberately more lenient than the WRITES (MEDIUM fix, round 4):
/// `answers.save`/`answer_assist`/`applied.check` all key on the UNDECODED spelling — their keys
/// are already-stored identities, so decoding at the write boundary would split existing rows off
/// from their own history. The consequence is a caller-side rule stated on the `job` verb's own
/// `--help`: reuse the `url` this resource RETURNS rather than a re-encoded spelling of your own.
/// Both HALVES of this lookup decode (see [`resolve_job`] for the stored side) — symmetric within
/// the read, never a one-sided rewrite.
pub(in crate::extension_bridge::agent_read) fn job_lookup_key(raw_url: &str) -> String {
    let decoded = crate::applications::decode_unreserved(raw_url);
    let canonical = crate::scraping::scrape_url::canonical_job_url(&decoded);
    crate::applications::normalize_job_url(canonical.as_deref().unwrap_or(&decoded))
}

/// The CALLER side of [`resolve_job`]'s identity compare (issue #1166) — the
/// identity counterpart to [`job_lookup_key`]'s normalized-string caller key,
/// run over the SAME unreserved-decoded input so a percent-escaped LinkedIn
/// slug still extracts the same id [`resolve_job`]'s stored-side extraction
/// computes. `None` for a board with no stable id space (or an unparseable
/// url) — [`resolve_job`] falls back to the normalized-string compare then.
pub(in crate::extension_bridge::agent_read) fn job_caller_identity(
    raw_url: &str,
) -> Option<(&'static str, String)> {
    let decoded = crate::applications::decode_unreserved(raw_url);
    crate::scraping::scrape_url::job_identity(&decoded)
}

pub(super) fn job_resource(app: &AppHandle, payload: &Value) -> AppResult<Value> {
    let raw_url = payload
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if raw_url.is_empty() {
        return Err(AppError::Validation("url is required".to_string()));
    }
    let normalized = job_lookup_key(raw_url);
    if normalized.is_empty() {
        return Err(AppError::Validation(
            "url is not a valid http(s) URL".to_string(),
        ));
    }
    let caller_identity = job_caller_identity(raw_url);
    let records = list_autopilots(app)?;
    // `store_present` derives from the SAME checked read as `applied_urls`
    // (round-4 fix T3-cont) — a `try_state().is_some()` alone can't tell a
    // managed-but-unreadable store from a genuinely-empty one; see
    // `commands::autopilot::applied_job_urls_checked`'s own doc.
    let applied = crate::commands::autopilot::applied_job_urls_checked(app);
    let store_present = applied.is_some();
    let applied_urls = applied.unwrap_or_default();
    resolve_job_for_store(
        &records,
        caller_identity,
        &normalized,
        &applied_urls,
        store_present,
    )
}
