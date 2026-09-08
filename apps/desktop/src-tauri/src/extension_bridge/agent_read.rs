//! `agent.query` → `agent.result` — the read-only agent/CLI surface (issue
//! #1084, PR 1). Six resources, one dispatch table ([`RESOURCES`]):
//! `best-matches` (optional `limit`), `job` (`url` required), `profile`,
//! `automations`, `schema`, `found-jobs` (issue #1115 — `autopilotId`
//! required, optional `limit`/`cursor`). `url` is the CROSS-RESOURCE KEY for
//! `job` — not an id (a `best-matches` row's own `key` is a cluster id,
//! never echoed here); `found-jobs` instead keys off `autopilotId` since it
//! must survive across autopilots that legitimately share a posting.
//!
//! ## Allowlist projections, absent by construction
//! Every payload below is built by [`project`]: round-trip the SOURCE value
//! through JSON into an allowlist struct (`AgentJob`, `AgentAutomation`,
//! `AgentBestMatch`, …). `serde`'s default "ignore unknown fields" behavior
//! on `Deserialize` means any field the source carries that the target
//! struct doesn't declare a slot for is silently dropped in that second
//! step — so `resume_text` / `cover_letter` / `assistant_notes` /
//! `assistant_provider` / `assistant_model` / `assistant_base_url` (and a
//! cluster's opaque `key`) are absent BY CONSTRUCTION: there is no field to
//! remember to omit. `profile` is the one exception — it reuses
//! `super::resolve_profile`/`super::AutofillProfile::from_contact` VERBATIM
//! (the exact function `profile.get` calls), so there is exactly one
//! profile projection in this crate, never two.
//!
//! ## Untrusted text still crosses one boundary: [`prompt_fence`](crate::prompt_fence)
//! An allowlist projection stops a FORBIDDEN FIELD from crossing; it says
//! nothing about a field that IS on the allowlist but carries raw,
//! third-party-authored scraped text into a consumer whose entire purpose is
//! "an AI agent reads this". `job`'s `description` is [`fence_description`]d
//! the same way `answer_assist::build_user_message` fences the identical
//! string before it reaches a model (ADR-010).
//!
//! ## Ungated, but not un-gated where it matters
//! Every agent verb is ungated by explicit owner decision (issue #1084) — no
//! new opt-in file. [`AgentQueryThrottle`] is the DoS bound, not a consent
//! gate. `profile` is the one resource that still refuses when autofill is
//! OFF, because it rides `profile.get`'s OWN pre-existing consent gate
//! (reusing that handling, not adding a second one) — that gate was never
//! about the agent surface, so "ungated" doesn't touch it.
//!
//! ## Throttle, not a compute cap
//! `best-matches` calls the already-public
//! `commands::autopilot::autopilot_best_matches` unmodified rather than
//! re-wrapping its private blocking fn or duplicating its clustering — see
//! [`AgentQueryThrottle`]'s doc for why that leaves the underlying compute
//! itself un-truncated and how the throttle compensates.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

// R8 LOC-cap split (`docs/architecture-rules.md`) — `found-jobs` (issue
// #1115) is large enough (allowlist struct + pagination + its own tests)
// that inlining it here pushed this module over the hard cap; see that
// file's own doc for why it can still reach every private item here.
//
// `pub(super)` (issue #1129) purely so `agent_cli::mcp` can DERIVE the
// `found-jobs` tool schema's advertised limits from
// `found_jobs::{DEFAULT,MAX}_FOUND_JOBS_LIMIT` (both
// `pub(in crate::extension_bridge)`) instead of retyping them — a
// hand-typed copy is what drifted to 50/100 against the enforced 25/50.
// Everything else in there stays private to this module.
pub(super) mod found_jobs;

// ── Resource table (schema's single source of truth) ───────────────────────

const RES_BEST_MATCHES: &str = "best-matches";
const RES_JOB: &str = "job";
const RES_PROFILE: &str = "profile";
const RES_AUTOMATIONS: &str = "automations";
const RES_SCHEMA: &str = "schema";
const RES_FOUND_JOBS: &str = "found-jobs";

/// `(name, description)` — `schema` maps this directly; [`handle_agent_query`]'s
/// `match` uses these SAME constants as its patterns (never a second literal),
/// so a rename here can't silently drift from what the dispatcher recognizes.
/// The `schema_lists_every_known_resource`/`dispatch_rejects_an_unknown_resource`
/// tests below pin the one drift this convention alone can't prevent — a new
/// arm added with its own fresh literal instead of reusing a constant here.
///
/// `pub(super)` for the bridge's own tests only (MEDIUM fix, review round 4):
/// `agent_cli`'s `both_automations_descriptions_name_both_totals` reads the
/// `automations` row here against its own `VERB_TABLE` row, because the
/// `totalFound`/`foundJobsTotal` distinction is written on BOTH surfaces and
/// nothing tied them together.
pub(super) const RESOURCES: &[(&str, &str)] = &[
    (
        RES_BEST_MATCHES,
        "Strongest jobs across every autopilot. Optional `limit` (default 20, max 50).",
    ),
    (RES_JOB, "Full detail for one posting. `url` required."),
    (
        RES_PROFILE,
        "Contact-profile fields for autofill — same consent gate as `profile.get`.",
    ),
    (
        RES_AUTOMATIONS,
        "Every autopilot and its status. `totalFound` is the LAST run's kept count; \
         `foundJobsTotal` is the whole stored list `found-jobs` pages through.",
    ),
    (RES_SCHEMA, "This resource list."),
    (
        RES_FOUND_JOBS,
        "Paginated traversal of ONE autopilot's complete found-jobs list (issue #1115). \
         `autopilotId` required, optional `limit`/`cursor` — repeat with the returned \
         `nextCursor` until it is `null`. A `nextCursor` is opaque and only valid for \
         the autopilot that returned it.",
    ),
];

fn schema_value() -> Value {
    json!({
        "resources": RESOURCES
            .iter()
            .map(|(name, description)| json!({ "name": name, "description": description }))
            .collect::<Vec<_>>(),
    })
}

// ── Throttle (on BridgeState, not per-connection — see module doc) ─────────

/// Minimal token bucket — the exact math `match_live::MatchLiveThrottle` uses,
/// but parameterized (`burst`/`refill_secs` are fields, not consts) because
/// [`AgentQueryThrottle`] needs TWO differently-tuned instances, not one.
struct TokenBucket {
    tokens: f64,
    last: std::time::Instant,
    burst: f64,
    refill_secs: f64,
}

impl TokenBucket {
    fn new(burst: f64, refill_secs: f64) -> Self {
        Self {
            tokens: burst,
            last: std::time::Instant::now(),
            burst,
            refill_secs,
        }
    }

    fn try_acquire_at(&mut self, now: std::time::Instant) -> bool {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed / self.refill_secs).min(self.burst);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Milliseconds until this bucket would hold one full token, computed from its CURRENT
    /// fractional `tokens` count (issue #1155) — a pure read, not a second clock advance. Only
    /// meaningful called right after a failed [`Self::try_acquire_at`] in the SAME tick: that
    /// call already set `self.tokens`/`self.last` to "now", so there is nothing left to advance.
    fn retry_after_ms(&self) -> u64 {
        if self.tokens >= 1.0 {
            return 0;
        }
        let needed_secs = (1.0 - self.tokens) * self.refill_secs;
        (needed_secs * 1000.0).ceil() as u64
    }
}

/// Cheap-read bucket (`job`/`profile`/`automations`/`schema`): burst 10,
/// refilling one token/second — generous for a scripted CLI polling loop.
const AGENT_CHEAP_BURST: f64 = 10.0;
const AGENT_CHEAP_REFILL_SECS: f64 = 1.0;
/// `best-matches` bucket: burst 1, refilling one token every 30s. Sized off
/// the measured worst case in `commands::autopilot::autopilot_best_matches`'s
/// own doc (3.03s at 2000 found-jobs, 12.3s at 4000) — this PR calls that
/// command UNMODIFIED (issue #1084's own preference: prefer the already-public
/// fn over re-wrapping its private blocking half or duplicating its
/// clustering, both of which either widen visibility across a domain
/// boundary this PR doesn't own — `commands::autopilot` — or fork a second
/// copy of `compute_best_matches`'s logic). That leaves the compute itself
/// UN-truncated per call; this bucket is what stops repeated invocation from
/// stacking that cost, not a pre-clustering cap on `found_jobs`. A follow-up
/// in the matching domain could add a real compute-side cap if that's not
/// enough — flagged in the PR1 handoff.
const AGENT_BEST_MATCHES_BURST: f64 = 1.0;
// `pub(super)` (issue #1155) — `extension_bridge::test`'s
// `bridge_state_agent_retry_after_ms_reads_the_same_bucket_try_acquire_agent_drew_from` anchors to
// this value directly, so a `BridgeState`-level test can't be satisfied by any hardcoded constant.
pub(super) const AGENT_BEST_MATCHES_REFILL_SECS: f64 = 30.0;

/// Token-bucket throttle for `agent.query`, shared across EVERY connection for
/// this pairing (lives on `BridgeState`, not per-connection) for the same
/// reason as `match_live::MatchLiveThrottle`: a CLI invocation is a fresh
/// process + fresh socket every time, so a per-connection bucket would be
/// bypassed by construction. A SEPARATE struct from `MatchLiveThrottle` (not
/// a generic shared one) — that struct's own doc reserves exactly this
/// scenario ("a future compute-heavy verb") for its own instance, since
/// per-verb cost profiles differ; `best-matches` alone does real CPU work
/// while the other five resources are cheap in-memory reads, so this struct
/// carries TWO independently-sized buckets rather than one shared bucket.
pub(super) struct AgentQueryThrottle {
    cheap: TokenBucket,
    best_matches: TokenBucket,
}

impl AgentQueryThrottle {
    pub(super) fn new() -> Self {
        Self {
            cheap: TokenBucket::new(AGENT_CHEAP_BURST, AGENT_CHEAP_REFILL_SECS),
            best_matches: TokenBucket::new(
                AGENT_BEST_MATCHES_BURST,
                AGENT_BEST_MATCHES_REFILL_SECS,
            ),
        }
    }

    /// Try to consume one token at `now` (explicit clock — directly
    /// unit-testable without a real sleep; production always goes through
    /// [`Self::try_acquire`]). An unrecognized `resource` draws from the
    /// cheap bucket — harmless, since it will fail resource-name validation
    /// right after in [`handle_agent_query`] anyway.
    fn try_acquire_at(&mut self, resource: &str, now: std::time::Instant) -> bool {
        if resource == RES_BEST_MATCHES {
            self.best_matches.try_acquire_at(now)
        } else {
            self.cheap.try_acquire_at(now)
        }
    }

    pub(super) fn try_acquire(&mut self, resource: &str) -> bool {
        self.try_acquire_at(resource, std::time::Instant::now())
    }

    /// [`TokenBucket::retry_after_ms`] for whichever bucket `resource` draws from — same routing
    /// [`Self::try_acquire_at`] uses, so the two can never disagree about which bucket a resource
    /// belongs to. `pub(super)` (issue #1155) — `BridgeState::agent_retry_after_ms` is the one
    /// caller, reached right after a failed `try_acquire` for the same resource.
    pub(super) fn retry_after_ms(&self, resource: &str) -> u64 {
        if resource == RES_BEST_MATCHES {
            self.best_matches.retry_after_ms()
        } else {
            self.cheap.retry_after_ms()
        }
    }
}

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
fn project_value<S, T>(source: &S) -> Option<Value>
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
struct AgentTrust {
    score: u8,
    level: crate::scraping::trust::TrustLevel,
    flags: Vec<crate::scraping::trust::TrustFlag>,
}

/// `job` resource payload — projected off `autopilot::FoundJob`. Excludes
/// `assistantNotes` (forbidden), `clusterId`/`clusterCanonical` (internal
/// grouping detail with no meaning off this surface).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentJob {
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
    applied: bool,
    is_agency: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    trust: Option<AgentTrust>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cluster_members: Vec<AgentClusterMember>,
}

/// Fixed sentinel — no autopilot has surfaced a job at this url. No dynamic
/// content (wire-error discipline, matches every other verb in this bridge).
const JOB_NOT_FOUND_MESSAGE: &str = "no job found for this url";

/// Pure core of the `job` resource: find the first `FoundJob` across every
/// (non-filtered — every status, not just active) autopilot record whose
/// normalized url matches, then project it. Mirrors
/// `applied_check::resolve_applied_check`'s pure/impure split — directly
/// unit-testable with hand-built `Autopilot` records, no `AppHandle`.
///
/// Both sides of the compare run through
/// [`decode_unreserved`](crate::applications::decode_unreserved) first (issue
/// #1128): a STORED url can carry the percent-encoded spelling just as easily
/// as a caller-supplied one, so decoding only the caller's half would fix the
/// reported direction and leave the mirror image broken. `normalized_url` is
/// pre-decoded by [`job_resource`]; this is the stored half.
fn resolve_job(records: &[crate::autopilot::Autopilot], normalized_url: &str) -> AppResult<Value> {
    let found = records
        .iter()
        .find_map(|ap| {
            ap.found_jobs.iter().find(|j| {
                crate::applications::normalize_job_url(&crate::applications::decode_unreserved(
                    &j.url,
                )) == normalized_url
            })
        })
        .ok_or_else(|| AppError::Validation(JOB_NOT_FOUND_MESSAGE.to_string()))?;
    let mut value = project_value::<_, AgentJob>(found)
        .ok_or_else(|| AppError::Message("failed to project job".to_string()))?;
    fence_description(&mut value);
    fence_posting_display_fields(&mut value);
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
/// [`fence_posting_display_fields`], called separately by both this fn's own
/// caller ([`resolve_job`]) and [`fence_best_match_fields`].
fn fence_description(value: &mut Value) {
    let Some(desc) = value.get("description").and_then(Value::as_str) else {
        return;
    };
    let fenced = crate::prompt_fence::fenced("job_posting", desc, crate::prompt_fence::JOB_CAP);
    value["description"] = json!(fenced);
}

/// `automations` resource's per-row payload — projected off `autopilot::Autopilot`.
/// Excludes `resumeText`/`coverLetter`/`assistant`/`assistantProvider`/
/// `assistantModel`/`assistantBaseUrl`/`foundJobs`/`lastRunSummaries` — the
/// first four forbidden outright, the last two out of scope for a status
/// listing (`best-matches` and `job` already cover found-jobs detail).
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentAutomation {
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
    total_applied: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_status: Option<crate::autopilot::RunStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_run_at: Option<u64>,
    created_at: u64,
    updated_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentAutomationTarget {
    boards: Vec<String>,
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
}

/// Direct field-by-field projection — NOT [`project_value`]'s
/// serialize-then-deserialize round trip (MEDIUM fix, "the cheap bucket's
/// premise is false" — security review). `project_value` round-trips the
/// WHOLE source through JSON first; for `Autopilot` that means serializing
/// `found_jobs` (every entry's full description) and
/// `resume_text`/`cover_letter` just to discard the result and keep a
/// handful of small fields. **Measured** (debug build, 50 autopilots × 1000 found jobs
/// each — an extreme but reachable scale, since `found_jobs` is never
/// truncated, see `commands/autopilot.rs`'s own doc): the round trip cost
/// ~320ms against ~1ms for this direct construction; the store's own
/// `list()` clone (shared with `job`, not owned by this module) adds another
/// ~50ms at that scale. Both are trivial against the 1-req/sec refill this
/// bucket already enforces, so no third bucket is warranted — but the round
/// trip was pure waste for a resource that already knows exactly which
/// fields it wants, so it's removed. `job`'s own `project_value` call stays
/// unchanged: it projects ONE already-found `FoundJob`, never the whole
/// store, so it was never the expensive half.
fn project_automation(ap: &crate::autopilot::Autopilot) -> AgentAutomation {
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
        total_applied: ap.total_applied,
        run_status: ap.run_status.clone(),
        last_run_at: ap.last_run_at,
        created_at: ap.created_at,
        updated_at: ap.updated_at,
    }
}

fn resolve_automations(records: &[crate::autopilot::Autopilot]) -> Value {
    let automations: Vec<Value> = records
        .iter()
        .filter_map(|ap| serde_json::to_value(project_automation(ap)).ok())
        .collect();
    json!({ "automations": automations })
}

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

/// Server-side default/cap for `best-matches`' `limit` — applied BEFORE
/// serialization (never trust an unbounded client-supplied number), well
/// under `MAX_FRAME_BYTES` even at the max.
///
/// `pub(super)` (issue #1129) for the same reason `found_jobs`' pair is
/// `pub(in crate::extension_bridge)`: `agent_cli::mcp` derives the
/// `best-matches` tool schema's advertised default/cap from THESE numbers
/// rather than a hand-typed copy that can silently drift out of sync.
pub(super) const DEFAULT_BEST_MATCHES_LIMIT: usize = 20;
pub(super) const MAX_BEST_MATCHES_LIMIT: usize = 50;

fn clamp_best_matches_limit(payload: &Value) -> usize {
    payload
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_BEST_MATCHES_LIMIT)
        .min(MAX_BEST_MATCHES_LIMIT)
}

/// Pure core of `best-matches`: project + `limit`-truncate an already-computed
/// row set. Directly unit-testable with hand-built `Value` rows, no
/// `AppHandle` — the impure half ([`best_matches_resource`]) only resolves
/// `commands::autopilot::autopilot_best_matches`'s output and `limit`.
fn resolve_best_matches(rows: &[Value], total: u64, limit: usize) -> Value {
    let matches: Vec<AgentBestMatch> = rows
        .iter()
        .filter_map(|row| serde_json::from_value(row.clone()).ok())
        .take(limit)
        .collect();
    let returned = matches.len();
    let mut value = json!({ "matches": matches, "total": total, "returned": returned });
    fence_best_match_fields(&mut value);
    value
}

/// Fence `title`/`company`/`location` on ONE object — shared by
/// [`fence_best_match_fields`] (one call per `best-matches` row) and
/// [`resolve_job`] (one call on the single job object), so the identical
/// primitive/tag/cap can never drift between the two curated-tier surfaces
/// that both carry these fields (MUST FIX — pre-PR gate: `resolve_job` used
/// to call only [`fence_description`], leaving `job`'s own title/company/
/// location bare while `best-matches` and the generic tier's own
/// `agent_call::FENCE_FIELD_NAMES` both fenced them — same threat, same
/// session, one hole).
fn fence_posting_display_fields(value: &mut Value) {
    for field in ["title", "company", "location"] {
        if let Some(s) = value.get(field).and_then(Value::as_str) {
            let fenced =
                crate::prompt_fence::fenced("job_posting", s, crate::prompt_fence::JOB_CAP);
            value[field] = json!(fenced);
        }
    }
}

/// Fence `title`/`company`/`location` on every `best-matches` row (MEDIUM
/// fix, MCP security critique — the MCP server is the first surface where a
/// model reads these fields with NO surrounding prompt at all, while also
/// holding `call-reversible` dispatch in the same session). Delegates to
/// [`fence_posting_display_fields`] per row.
fn fence_best_match_fields(value: &mut Value) {
    let Some(matches) = value.get_mut("matches").and_then(Value::as_array_mut) else {
        return;
    };
    for row in matches {
        fence_posting_display_fields(row);
    }
}

async fn best_matches_resource(app: &AppHandle, payload: &Value) -> AppResult<Value> {
    let limit = clamp_best_matches_limit(payload);
    let raw = crate::commands::autopilot::autopilot_best_matches(app.clone()).await;
    let total = raw.get("total").and_then(Value::as_u64).unwrap_or(0);
    let rows = raw
        .get("matches")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(resolve_best_matches(&rows, total, limit))
}

/// Shared `AutopilotStore` read for the `job`/`automations` resources —
/// `try_state` (never the panicking `.state()`), degrading to a config error
/// rather than a panic inside a frame handler (this crate builds with
/// `panic = "abort"` in release).
fn list_autopilots(app: &AppHandle) -> AppResult<Vec<crate::autopilot::Autopilot>> {
    app.try_state::<std::sync::Arc<parking_lot::Mutex<crate::autopilot::AutopilotStore>>>()
        .map(|s| s.lock().list())
        .ok_or_else(|| AppError::Config("autopilot store unavailable".to_string()))
}

/// The CALLER side of `job`'s identity pipeline, extracted from
/// [`job_resource`] so it is unit-testable without an `AppHandle` — the
/// counterpart to [`resolve_job`]'s stored side, and the only place the two
/// halves can be compared for symmetry (issue #1128). Empty means "not a
/// usable http(s) url", exactly as `normalize_job_url` reports it.
///
/// Same canonicalize-then-normalize pipeline `applied.check`/`answers.save`
/// use, plus an unreserved-only decode FIRST, so the canonicalizer reads the
/// real path: a `%2D`-spelled LinkedIn slug is byte-different but
/// semantically identical (RFC 3986 §6.2.2.2), and neither
/// `canonical_job_url` nor `normalize_job_url` decodes anything. The scheme
/// guard still runs AFTER the decode, inside `normalize_job_url`, so
/// `%6Aavascript:…` is caught rather than smuggled past a raw-byte check.
///
/// That decode makes this READ deliberately more lenient than the WRITES
/// (MEDIUM fix, security review round 4 — this doc used to claim the lookup
/// "resolves to the exact identity an import would", which it does not).
/// `answers.save`, `answer_assist` and `applied.check` all key on the
/// UNDECODED spelling, and widening them is out of scope here: their keys are
/// already-stored identities, so decoding at the write boundary would split
/// existing rows off from their own history. The consequence is a caller-side
/// rule, stated on the `job` verb's own `--help`/tool description
/// (`agent_cli::VERB_TABLE`): reuse the `url` this resource RETURNS rather
/// than a re-encoded spelling of your own, and every surface agrees on which
/// posting you mean. Both HALVES of this lookup decode (see [`resolve_job`]
/// for the stored side) — the leniency is symmetric within the read, never a
/// one-sided rewrite.
fn job_lookup_key(raw_url: &str) -> String {
    let decoded = crate::applications::decode_unreserved(raw_url);
    let canonical = crate::scraping::scrape_url::canonical_job_url(&decoded);
    crate::applications::normalize_job_url(canonical.as_deref().unwrap_or(&decoded))
}

fn job_resource(app: &AppHandle, payload: &Value) -> AppResult<Value> {
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
    let records = list_autopilots(app)?;
    resolve_job(&records, &normalized)
}

fn automations_resource(app: &AppHandle) -> AppResult<Value> {
    Ok(resolve_automations(&list_autopilots(app)?))
}

fn profile_resource(app: &AppHandle) -> AppResult<Value> {
    // Reuses `super::profile_outcome` VERBATIM — the exact consent gate +
    // `AutofillProfile` projection `profile.get` uses (see
    // `super::handle_profile`). There is exactly one profile projection in
    // this crate. `agent_read` is a CHILD module of `extension_bridge`, so it
    // can already see `mod.rs`'s private `profile_outcome` — nothing needed
    // widening for this resource.
    super::profile_outcome(app)
        .and_then(|p| serde_json::to_value(&p).map_err(|e| AppError::Message(e.to_string())))
}

// ── Dispatch ─────────────────────────────────────────────────────────────

/// The resource named by an `agent.query` payload — `""` when absent/not a
/// string. Used both to route dispatch and to pick the throttle bucket.
pub(super) fn resource_name(payload: &Value) -> &str {
    payload
        .get("resource")
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn agent_result_reply(req_id: &str, resource: &str, outcome: AppResult<Value>) -> String {
    let payload = match outcome {
        Ok(data) => json!({ "ok": true, "resource": resource, "data": data }),
        // Wire-error discipline: `AppError`'s `Display` here is always a fixed
        // sentinel or an echo of the CALLER'S OWN `resource`/`url` input
        // (never path/PII content) — mirrors `advance_authenticated`'s
        // "unknown message type" reply.
        Err(e) => json!({ "ok": false, "resource": resource, "error": e.to_string() }),
    };
    json!({
        "type": super::msg::AGENT_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

// ── Bounded refusals (issue #1151 — this tier had no equivalent to
// `agent_call::refusal_reply`/`enforce_frame_cap`, so a refusal built from a near-cap `resource`/
// `reqId` could itself exceed the frame cap on the way out, and a legitimately oversized SUCCESS
// reply — an uncapped `job`/`best-matches` payload — closed the socket with no refusal at all) ──

/// A [`super::agent_call::clamp_ident`]-bounded, sentinel+detail refusal — the shape
/// `agent_call::refusal_reply` uses, adopted here for the two MACHINE-READABLE refusals this
/// tier gained from issues #1151/#1155 (`rate_limited`, `result_too_large`). Every OTHER refusal
/// this tier answers (an unrecognized `resource`, `origin_refused`, a resource fn's own
/// validation error) keeps its EXISTING shape unchanged — `error` carries the prose directly, no
/// `detail` — routed through [`bounded_result_reply`] instead, so no existing client parsing
/// THOSE breaks. `extra` merges additional fields (`retryAfterMs`, an identity arg) onto the
/// payload; pass `json!({})` for none. Re-measures the built reply and degrades to a minimal
/// envelope (mirrors `agent_call::REFUSAL_UNDELIVERABLE_DETAIL` verbatim) if it still does not
/// fit — "measured, not assumed" for the same reason that fn's own doc gives.
fn sentinel_refusal_reply(
    req_id: &str,
    resource: &str,
    error: &'static str,
    detail: String,
    extra: Value,
) -> String {
    let mut payload = json!({
        "ok": false,
        "resource": super::agent_call::clamp_ident(resource),
        "error": error,
        "detail": detail,
    });
    if let (Value::Object(base), Value::Object(more)) = (&mut payload, &extra) {
        for (k, v) in more {
            base.insert(k.clone(), v.clone());
        }
    }
    let reply = json!({
        "type": super::msg::AGENT_RESULT,
        "reqId": super::agent_call::clamp_ident(req_id),
        "payload": payload,
    })
    .to_string();
    if reply.len() <= super::MAX_FRAME_BYTES {
        return reply;
    }
    json!({
        "type": super::msg::AGENT_RESULT,
        "reqId": "",
        "payload": {
            "ok": false,
            "resource": "",
            "error": error,
            "detail": super::agent_call::REFUSAL_UNDELIVERABLE_DETAIL,
        },
    })
    .to_string()
}

/// [`agent_result_reply`], with `resource`/`reqId` pre-clamped and the built reply re-measured
/// against [`super::MAX_FRAME_BYTES`] (issue #1151) — the SAME two properties
/// `agent_call::refusal_reply` guarantees, one wire type over, applied to EVERY reply this tier
/// builds (the success path included, mirroring `agent_call::handle_agent_call`'s single
/// `enforce_frame_cap` call site): an oversized reply of any kind — a legitimately huge `job`/
/// `best-matches` payload, or (after clamping, effectively unreachable) a refusal that still
/// somehow didn't fit — is substituted with a [`sentinel_refusal_reply`] `result_too_large`
/// refusal rather than closing the socket with nothing (the exact #1135 failure mode this mirrors
/// from the generic tier).
fn bounded_result_reply(req_id: &str, resource: &str, outcome: AppResult<Value>) -> String {
    let reply = agent_result_reply(
        super::agent_call::clamp_ident(req_id),
        super::agent_call::clamp_ident(resource),
        outcome,
    );
    if reply.len() <= super::MAX_FRAME_BYTES {
        return reply;
    }
    sentinel_refusal_reply(
        req_id,
        resource,
        super::agent_call::ERR_RESULT_TOO_LARGE,
        format!(
            "the reply ({} B) exceeds the bridge's own frame cap and was discarded rather than \
             truncated — narrow the request (a smaller `limit`, a `found-jobs` page) if this \
             resource takes one",
            reply.len()
        ),
        json!({}),
    )
}

// `pub(super)` — reused verbatim by `agent_call`'s own throttle refusal
// (Phase 2, ADR-038 §2) so the two tiers report identical wording for the
// identical shared-bucket cause, never a second hand-typed copy.
pub(super) const THROTTLED_MESSAGE: &str = "Too many requests — try again shortly.";

/// The caller-supplied argument that names WHICH request a throttle refusal belongs to, beyond
/// `resource` alone (issue #1155 — a throttled `job` lookup used to echo only
/// `"resource":"job"`, never which of several in-flight urls was refused). `job` keys on `url`,
/// `found-jobs` on `autopilotId`; every other resource takes no per-request identifier. Clamped
/// like every other echoed identifier here — caller-supplied, bounded only by the incoming frame.
fn identity_arg<'a>(resource: &str, payload: &'a Value) -> Option<(&'static str, &'a str)> {
    let field = match resource {
        RES_JOB => "url",
        RES_FOUND_JOBS => "autopilotId",
        _ => return None,
    };
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(|v| (field, super::agent_call::clamp_ident(v)))
}

/// The read tier's own `rate_limited` refusal (issue #1155) — the SAME sentinel+detail shape,
/// SAME `retryAfterMs`, as `agent_call::throttled_reply`'s: `retry_after_ms` is computed by the
/// ONE caller (`mod.rs`) from the shared `AgentQueryThrottle` bucket right after the failed
/// acquire, never invented here. Adds the refused request's identity — `resource` plus, where the
/// resource takes one, [`identity_arg`] — so a caller juggling several in-flight lookups can tell
/// WHICH one was blocked (the gap issue #1155 reports: three throttled `job` lookups previously
/// looked identical).
pub(super) fn throttled_reply(req_id: &str, payload: &Value, retry_after_ms: u64) -> String {
    let resource = resource_name(payload);
    let mut extra = json!({ "retryAfterMs": retry_after_ms });
    if let Some((field, value)) = identity_arg(resource, payload) {
        extra[field] = json!(value);
    }
    sentinel_refusal_reply(
        req_id,
        resource,
        super::agent_call::ERR_RATE_LIMITED,
        THROTTLED_MESSAGE.to_string(),
        extra,
    )
}

/// Fixed sentinel — `msg::AGENT_QUERY`'s doc; never dynamic content (matches
/// every other refusal on this surface).
const CLI_ONLY_MESSAGE: &str = "agent.query is only available to the ajh-tauri agent CLI";

/// Reply for an `agent.query` arriving over a connection whose handshake
/// `Origin` wasn't `auth::AGENT_CLI_ORIGIN` (finding #5, security review) —
/// same `agent.result` envelope shape as every other outcome on this
/// surface, so a caller that DID legitimately reach this (there is none
/// today; see `msg::AGENT_QUERY`'s doc) parses it identically to any other
/// refusal. Routed through [`bounded_result_reply`] (issue #1151) rather than [`agent_result_reply`]
/// directly — this path writes straight to the socket (see `mod.rs`'s dispatch match), so nothing
/// else in this crate bounds what it echoes.
pub(super) fn origin_refused_reply(req_id: &str, payload: &Value) -> String {
    bounded_result_reply(
        req_id,
        resource_name(payload),
        Err(AppError::Validation(CLI_ONLY_MESSAGE.to_string())),
    )
}

/// Answer an authenticated, throttle-admitted `agent.query`. Never panics —
/// every resource fn degrades to `Err` on a missing/unexpected state (see
/// `list_autopilots`), and this match's fallback arm covers any resource name
/// [`RESOURCES`] doesn't recognize. Routed through [`bounded_result_reply`] (issue #1151):
/// identifiers are clamped and the reply is frame-capped, substituting `result_too_large` for an
/// oversized SUCCESS payload the same way `agent_call::handle_agent_call` already does for the
/// generic tier.
pub(super) async fn handle_agent_query(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let resource = resource_name(payload).to_string();
    let outcome = match resource.as_str() {
        RES_BEST_MATCHES => best_matches_resource(app, payload).await,
        RES_JOB => job_resource(app, payload),
        RES_PROFILE => profile_resource(app),
        RES_AUTOMATIONS => automations_resource(app),
        RES_FOUND_JOBS => found_jobs::found_jobs_resource(app, payload),
        RES_SCHEMA => Ok(schema_value()),
        // `other` is clamped here too (issue #1151, AC-3) — `bounded_result_reply` below only
        // clamps the envelope's `reqId`/`resource`, not a copy embedded in THIS message, so an
        // unclamped `other` could still blow the frame cap and get swallowed by the
        // `result_too_large` fallback, hiding the real cause (an unknown resource) behind the
        // wrong one (a reply too large).
        other => Err(AppError::Validation(format!(
            "unknown agent resource '{}'",
            super::agent_call::clamp_ident(other)
        ))),
    };
    bounded_result_reply(req_id, &resource, outcome)
}

#[cfg(test)]
mod tests;
