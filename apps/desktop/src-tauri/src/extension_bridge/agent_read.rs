//! `agent.query` → `agent.result` — the read-only agent/CLI surface (issue
//! #1084, PR 1). Six resources, one dispatch table ([`RESOURCES`]):
//! `best-matches` (optional `limit`/`cursor`/`query`, issue #1146 P11),
//! `job` (`url` required), `profile`, `automations`, `schema`, `found-jobs`
//! (issue #1115 — optional `autopilotId`/`limit`/`cursor` plus the
//! `minScore`/`country`/`remote`/`applied`/`query` filters, issues
//! #1167/#1168). `url` is the CROSS-RESOURCE KEY for `job` — not an id (a
//! `best-matches` row's own `key` is a cluster id, never echoed here);
//! `found-jobs` instead keys its cursor off `autopilotId` (or a fixed
//! all-autopilots sentinel when omitted) since it must survive across
//! autopilots that legitimately share a posting.
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
        "Strongest jobs across every autopilot, ranked. Optional `limit` (default 20, max 50), \
         `cursor` (repeat with the returned `nextCursor` until it is `null` to reach every row \
         past the first page), and `query` (case-insensitive substring over title or company). \
         `query` filters the already-capped, ranked top-N candidate list this tool computes \
         (NOT the full stored corpus) — a posting outside that cap reads as absent even when it \
         is still in storage; use `found-jobs`' own `query` to search every stored posting.",
    ),
    (
        RES_JOB,
        "Full detail for one posting, matched by its posting `url` ONLY — never by title or \
         company (use `found-jobs`' own `query` filter for that). `url` required.",
    ),
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
        "Paginated traversal of the stored found-jobs list (issue #1115). Every reply carries \
         `total` — the filtered row count THIS call matches, so a count never requires a full \
         traversal. `autopilotId` is optional (issue #1168): given, scopes to one autopilot; \
         omitted, spans every autopilot (deduped by posting identity) — the one call that \
         answers \"is this role already in my list?\" (`found-jobs {query: \"…\"}`). Optional \
         `limit`/`cursor` — repeat with the returned `nextCursor` until it is `null`; a cursor \
         is opaque and only valid for the same `autopilotId` scope AND the same filter \
         arguments that issued it. Optional \
         server-side filters `minScore`, `country` (substring match against location), `remote` \
         (bool), `applied` (bool) and `query` (substring over title/company). Rows are compact \
         (no `description`) unless `includeDescription: true` is set.",
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
const AGENT_BEST_MATCHES_REFILL_SECS: f64 = 30.0;

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

/// Fixed detail attached to [`JOB_NOT_FOUND_MESSAGE`] (issue #1166) — names
/// where a caller that still misses can read the url the app actually
/// stored, rather than being left at a dead end.
const JOB_NOT_FOUND_DETAIL: &str =
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
fn resolve_job(
    records: &[crate::autopilot::Autopilot],
    caller_identity: Option<(&'static str, String)>,
    normalized_url: &str,
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
/// `assistantModel`/`assistantBaseUrl`/`foundJobs`/`lastRunSummaries`/
/// `totalApplied` — the first four forbidden outright, the next two out of
/// scope for a status listing (`best-matches` and `job` already cover
/// found-jobs detail), and `totalApplied` dropped (issue #1171): the field
/// is dead on the source struct too (`docs/ARCHITECTURE_STATUS.md`'s own
/// "Drop dead `totalApplied` counter" row) — nothing in this codebase ever
/// writes it past its zero default, so exposing it here promised a real
/// applied-count that never existed.
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

/// Issue #1167/#1146 P11 — reuses `extension_bridge::paging::clamp_limit`, the
/// same shared primitive `found_jobs` uses, rather than a hand-rolled copy: the
/// hand-rolled version this replaced let `limit: 0` through as `0` instead of
/// falling back to [`DEFAULT_BEST_MATCHES_LIMIT`] (`Value::as_u64` reads `0` as
/// `Some(0)`, so `.unwrap_or` never fired) — a zero-row page whose `nextCursor`
/// never advances, hanging any paging loop built on it forever.
fn clamp_best_matches_limit(payload: &Value) -> usize {
    crate::extension_bridge::paging::clamp_limit(
        payload,
        DEFAULT_BEST_MATCHES_LIMIT,
        MAX_BEST_MATCHES_LIMIT,
    )
}

/// A `cursor` that isn't a nextCursor SHAPE at all — mirrors
/// `found_jobs::MALFORMED_CURSOR_MESSAGE`'s own wording for the identical
/// case, one hop over.
const BEST_MATCHES_MALFORMED_CURSOR_MESSAGE: &str =
    "cursor must be a nextCursor returned by a best-matches page — a bare offset is not one; \
     restart from `cursor: null`";

/// A well-formed `<issuer>:<offset>` cursor issued under a DIFFERENT `query`
/// (round 2 fix, B3-r1-F4 — `best-matches`' row set now depends on `query`
/// too, issue #1168, so a bare offset let a cursor replayed under a
/// DIFFERENT query silently page a different filtered list at a stale
/// offset, skipping rows rather than refusing). Mirrors
/// `found_jobs::WRONG_AUTOPILOT_CURSOR_MESSAGE`'s own split from the
/// malformed case: this one is "you are paging the wrong list", recoverable
/// by paging that same query.
const BEST_MATCHES_WRONG_QUERY_CURSOR_MESSAGE: &str =
    "cursor was issued for a different `query` — page that same query with it, or restart from \
     `cursor: null`";

/// Fold `query`'s already-normalized (lowercased/trimmed) value into the
/// cursor's issuer half — mirrors `found_jobs::found_jobs_cursor_issuer`'s
/// identical reasoning one resource over. [`crate::extension_bridge::paging::fingerprint`]
/// rather than the raw query text: `query` is caller-typed and could itself
/// contain `:`, and a fingerprint sidesteps needing to prove it never
/// collides with the issuer's own delimiter.
fn best_matches_cursor_issuer(query: Option<&str>) -> String {
    crate::extension_bridge::paging::fingerprint(&[query.unwrap_or("")])
}

/// Parse `payload`'s `cursor` against `issuer` (see
/// [`best_matches_cursor_issuer`]) — mirrors
/// `found_jobs::parse_found_jobs_cursor`'s own shape-then-issuer contract
/// and never-echo discipline, one resource over (round 2 fix, B3-r1-F4:
/// `best-matches` used to accept a bare numeric offset via
/// `extension_bridge::paging::parse_offset_cursor`, which carried no
/// evidence of which `query` produced it).
fn parse_best_matches_cursor(payload: &Value, issuer: &str) -> AppResult<usize> {
    let malformed = || AppError::Validation(BEST_MATCHES_MALFORMED_CURSOR_MESSAGE.to_string());
    match payload.get("cursor") {
        None | Some(Value::Null) => Ok(0),
        Some(Value::String(raw)) => {
            match raw
                .rsplit_once(':')
                .and_then(|(iss, off)| Some((iss, off.parse::<usize>().ok()?)))
            {
                Some((iss, off)) if iss == issuer => Ok(off),
                Some(_) => Err(AppError::Validation(
                    BEST_MATCHES_WRONG_QUERY_CURSOR_MESSAGE.to_string(),
                )),
                None => Err(malformed()),
            }
        }
        Some(_) => Err(malformed()),
    }
}

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
fn resolve_best_matches(rows: &[Value], offset: usize, limit: usize, query: Option<&str>) -> Value {
    let mut matches: Vec<AgentBestMatch> = rows
        .iter()
        .filter_map(|row| serde_json::from_value(row.clone()).ok())
        .collect();
    if let Some(q) = query {
        matches
            .retain(|m| m.title.to_lowercase().contains(q) || m.company.to_lowercase().contains(q));
    }
    let total = matches.len();
    let page: Vec<AgentBestMatch> = matches.into_iter().skip(offset).take(limit).collect();
    let returned = page.len();
    let next_offset = offset + returned;
    let cursor_issuer = best_matches_cursor_issuer(query);
    let next_cursor = if next_offset < total {
        Some(format!("{cursor_issuer}:{next_offset}"))
    } else {
        None
    };
    let mut value = json!({
        "matches": page,
        "total": total,
        "returned": returned,
        "nextCursor": next_cursor,
    });
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
    // Round 2 fix (B3-r2-F1) — used to read `query` with
    // `.and_then(Value::as_str)`, the exact silent-drop combinator
    // `found_jobs::trimmed_lowercase_filter` was hardened away from for the
    // identical key one resource over: a non-string (or, since B3-r2-F2,
    // present-but-blank) `query` collapsed to `None` here — indistinguishable
    // from omitted — and the caller got the unfiltered ranked list back with
    // a `total` it read as the filtered count. Reuses that SAME fallible
    // parse rather than a second copy, so the two can't drift apart again.
    let query = found_jobs::trimmed_lowercase_filter(payload, "query")?;
    let cursor_issuer = best_matches_cursor_issuer(query.as_deref());
    let offset = parse_best_matches_cursor(payload, &cursor_issuer)?;
    let raw = crate::commands::autopilot::autopilot_best_matches(app.clone()).await;
    let rows = raw
        .get("matches")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(resolve_best_matches(&rows, offset, limit, query.as_deref()))
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

/// The CALLER side of [`resolve_job`]'s identity compare (issue #1166) — the
/// identity counterpart to [`job_lookup_key`]'s normalized-string caller key,
/// run over the SAME unreserved-decoded input so a percent-escaped LinkedIn
/// slug still extracts the same id [`resolve_job`]'s stored-side extraction
/// computes. `None` for a board with no stable id space (or an unparseable
/// url) — [`resolve_job`] falls back to the normalized-string compare then.
fn job_caller_identity(raw_url: &str) -> Option<(&'static str, String)> {
    let decoded = crate::applications::decode_unreserved(raw_url);
    crate::scraping::scrape_url::job_identity(&decoded)
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
    let caller_identity = job_caller_identity(raw_url);
    let records = list_autopilots(app)?;
    resolve_job(&records, caller_identity, &normalized)
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

/// `(resource, fixed error sentinel) -> fixed detail`, for the handful of
/// refusals whose caller needs a next step rather than a bare sentinel
/// (issue #1166). Both sides of the match are compile-time constants, so
/// this can never echo caller-supplied content into `detail`.
fn error_detail(resource: &str, error: &str) -> Option<&'static str> {
    match (resource, error) {
        (RES_JOB, JOB_NOT_FOUND_MESSAGE) => Some(JOB_NOT_FOUND_DETAIL),
        _ => None,
    }
}

fn agent_result_reply(req_id: &str, resource: &str, outcome: AppResult<Value>) -> String {
    let payload = match outcome {
        Ok(data) => json!({ "ok": true, "resource": resource, "data": data }),
        // Wire-error discipline: `AppError`'s `Display` here is always a fixed
        // sentinel or an echo of the CALLER'S OWN `resource`/`url` input
        // (never path/PII content) — mirrors `advance_authenticated`'s
        // "unknown message type" reply. `detail` (issue #1166) is looked up
        // off the SAME fixed sentinel — never dynamic content either — and
        // omitted entirely when there is none, same shape as every other
        // resource's success-only payload.
        Err(e) => {
            let error = e.to_string();
            let mut payload = json!({ "ok": false, "resource": resource, "error": error });
            if let Some(detail) = error_detail(resource, &error) {
                payload["detail"] = json!(detail);
            }
            payload
        }
    };
    json!({
        "type": super::msg::AGENT_RESULT,
        "reqId": req_id,
        "payload": payload,
    })
    .to_string()
}

// `pub(super)` — reused verbatim by `agent_call`'s own throttle refusal
// (Phase 2, ADR-038 §2) so the two tiers report identical wording for the
// identical shared-bucket cause, never a second hand-typed copy.
pub(super) const THROTTLED_MESSAGE: &str = "Too many requests — try again shortly.";

pub(super) fn throttled_reply(req_id: &str, resource: &str) -> String {
    agent_result_reply(
        req_id,
        resource,
        Err(AppError::RateLimited(THROTTLED_MESSAGE.to_string())),
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
/// refusal.
pub(super) fn origin_refused_reply(req_id: &str, payload: &Value) -> String {
    agent_result_reply(
        req_id,
        resource_name(payload),
        Err(AppError::Validation(CLI_ONLY_MESSAGE.to_string())),
    )
}

/// Answer an authenticated, throttle-admitted `agent.query`. Never panics —
/// every resource fn degrades to `Err` on a missing/unexpected state (see
/// `list_autopilots`), and this match's fallback arm covers any resource name
/// [`RESOURCES`] doesn't recognize.
pub(super) async fn handle_agent_query(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let resource = resource_name(payload).to_string();
    let outcome = match resource.as_str() {
        RES_BEST_MATCHES => best_matches_resource(app, payload).await,
        RES_JOB => job_resource(app, payload),
        RES_PROFILE => profile_resource(app),
        RES_AUTOMATIONS => automations_resource(app),
        RES_FOUND_JOBS => found_jobs::found_jobs_resource(app, payload),
        RES_SCHEMA => Ok(schema_value()),
        other => Err(AppError::Validation(format!(
            "unknown agent resource '{other}'"
        ))),
    };
    agent_result_reply(req_id, &resource, outcome)
}

#[cfg(test)]
mod tests;
