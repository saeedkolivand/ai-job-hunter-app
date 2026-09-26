//! `agent.query` → `agent.result` — the read-only agent/CLI surface (issue #1084, PR 1). One
//! dispatch table ([`RESOURCES`]): `best-matches` (optional `limit`/`cursor`/`query`), `job` (`url`
//! required), `profile`, `automations`, `schema`, `found-jobs` (issue #1115 — optional
//! `autopilotId`/`limit`/`cursor` plus `minScore`/`country`/`remote`/`applied`/`query` filters),
//! `documents` (PR2), `prep` (PR4). `url` is the CROSS-RESOURCE KEY for `job`, not an id;
//! `found-jobs` keys its cursor off `autopilotId` (or an all-autopilots sentinel) since it must
//! survive across autopilots that legitimately share a posting.
//!
//! ## Allowlist projections, absent by construction
//! Every payload below is built by [`project`]: round-trip the SOURCE value through JSON into an
//! allowlist struct. `serde`'s default "ignore unknown fields" `Deserialize` behavior means any
//! field the target struct doesn't declare is silently dropped — so `resume_text`/`cover_letter`/
//! `assistant_notes`/`assistant_provider`/`assistant_model`/`assistant_base_url` (and a cluster's
//! opaque `key`) are absent BY CONSTRUCTION. `profile` is the one exception — it reuses
//! `super::resolve_profile` VERBATIM, so there is exactly one profile projection in this crate.
//!
//! ## Untrusted text still crosses one boundary: [`prompt_fence`](crate::prompt_fence)
//! An allowlist projection stops a FORBIDDEN FIELD; it says nothing about an allowlisted field
//! carrying raw scraped text. `job`'s `description` is [`fence_description`]d the same way
//! `answer_assist::build_user_message` fences the identical string before it reaches a model.
//!
//! ## Ungated, but not un-gated where it matters
//! Every agent verb is ungated by explicit owner decision — no new opt-in file.
//! [`AgentQueryThrottle`] is the DoS bound, not a consent gate. `profile` still refuses when
//! autofill is OFF, riding `profile.get`'s OWN pre-existing consent gate rather than adding a
//! second one.
//!
//! ## Throttle, not a compute cap
//! `best-matches` calls the already-public `autopilot_best_matches` unmodified rather than
//! re-wrapping its private blocking fn — see [`AgentQueryThrottle`]'s doc for why that leaves the
//! compute itself un-truncated and how the throttle compensates.

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

/// `documents` resource (PR2 — documents into ATS) — see its own module doc. Plain `mod` (no
/// visibility needed beyond this module; unlike `found_jobs`, nothing outside `agent_read` reads
/// into it).
mod documents;

/// `prep` resource (PR4 — Prep tab) — see its own module doc. Plain `mod`, same reasoning as
/// `documents` above.
mod prep;

/// `best-matches` resource (PR4 — R8 relief, same LOC-cap reasoning `found_jobs` above carries) —
/// see its own module doc. `pub(super)` (same as `found_jobs`) purely so `agent_cli::mcp` can
/// reach `best_matches::{DEFAULT,MAX}_BEST_MATCHES_LIMIT` (both `pub(in crate::extension_bridge)`)
/// to derive the `best-matches` tool schema's advertised limits, the same way it already does for
/// `found_jobs`'s pair. Every other caller (`agent_read` itself, this module's own
/// `#[cfg(test)] mod tests;`, `found_jobs`) reaches in via the qualified `best_matches::name` path
/// — the same convention `found_jobs`'s own items use everywhere in this crate; no blanket
/// re-export.
pub(super) mod best_matches;

// ── Resource table (schema's single source of truth) ───────────────────────

const RES_BEST_MATCHES: &str = "best-matches";
const RES_JOB: &str = "job";
const RES_PROFILE: &str = "profile";
const RES_AUTOMATIONS: &str = "automations";
const RES_SCHEMA: &str = "schema";
const RES_FOUND_JOBS: &str = "found-jobs";
const RES_DOCUMENTS: &str = "documents";
const RES_PREP: &str = "prep";

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
        "Strongest jobs across every autopilot, ranked. Optional `limit` (default 20, max 100), \
         `cursor` (repeat with the returned `nextCursor` until it is `null` to reach every row \
         past the first page; a cursor is opaque and only valid for the same `query` — present \
         or omitted — that issued it), and `query` (case-insensitive substring over title or \
         company). `query` filters the already-capped, ranked top-N candidate list this tool \
         computes (NOT the full stored corpus) — a posting outside that cap reads as absent even \
         when it is still in storage; use `found-jobs`' own `query` to search every stored \
         posting. `total` is the size of this capped ranked list, not the number of qualifying \
         postings in storage — use `found-jobs` for a true corpus count.",
    ),
    (
        RES_JOB,
        "Full detail for one posting, matched by its posting `url` ONLY — never by title or \
         company (use `found-jobs`' own `query` filter for that). `url` required. `applied` is \
         OMITTED (never a confident `false`) when the applications store is unreadable; the \
         reply then carries `appliedUnavailable: true`.",
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
         (no `description`) unless `includeDescription: true` is set. Each row's `applied` is \
         OMITTED (never a confident `false`) when the applications store is unreadable; the \
         reply then carries `appliedUnavailable: true` and the `applied` filter is refused.",
    ),
    (
        RES_DOCUMENTS,
        "Document-picker candidates for one job posting (PR2 — documents into ATS): `url` \
         required. Reports whether a saved generation exists for it — résumé/cover-letter TEXT \
         PRESENCE only, never the text itself — plus the base résumés on file, newest first \
         (capped). Fetch the actual bytes with the dedicated `document.export` bridge verb, not \
         through this resource.",
    ),
    (
        RES_PREP,
        "The Prep tab's content for one job posting (PR4): `url` required. Returns the ACTUAL \
         TEXT of any existing company brief, interview questions, and salary answer for this job \
         (zero-cost read of the user's own data) — `null` when nothing exists yet; `truncated: \
         true` means a generous cap was hit. To GENERATE a missing brief/salary answer, use \
         `answer.assist`'s optional `topic` field (billable, its own opt-in).",
    ),
];

/// A placeholder-envelope's serialized length minus its empty-array placeholder's own two bytes
/// (`[]`) — shared by `found_jobs`/`best_matches`'s otherwise-identical base-cost measurement:
/// each builds its own envelope shape with a `total`-sized stand-in for a not-yet-known cursor
/// string (a real offset can never exceed `total`, so this can only over-count and thus only trim
/// MORE than strictly required, never less), then hands it here to measure.
fn envelope_cost_estimate(base_envelope: &Value) -> usize {
    serde_json::to_string(base_envelope)
        .map_or(usize::MAX, |s| s.len())
        .saturating_sub(2)
}

fn schema_value() -> Value {
    json!({
        "resources": RESOURCES
            .iter()
            .map(|(name, description)| json!({ "name": name, "description": description }))
            .collect::<Vec<_>>(),
    })
}

mod automations;
mod job;
mod reply;
mod throttle;

use job::AgentTrust;
pub(super) use reply::{
    extension_capped_reply, extension_gate_reply, origin_refused_reply, resource_name,
    throttled_reply, THROTTLED_MESSAGE,
};
pub(super) use throttle::AgentQueryThrottle;
#[cfg(test)]
pub(super) use throttle::AGENT_BEST_MATCHES_REFILL_SECS;

/// Shared `AutopilotStore` read for the `job`/`automations` resources —
/// `try_state` (never the panicking `.state()`), degrading to a config error
/// rather than a panic inside a frame handler (this crate builds with
/// `panic = "abort"` in release).
fn list_autopilots(app: &AppHandle) -> AppResult<Vec<crate::autopilot::Autopilot>> {
    app.try_state::<std::sync::Arc<parking_lot::Mutex<crate::autopilot::AutopilotStore>>>()
        .map(|s| s.lock().list())
        .ok_or_else(|| AppError::Config("autopilot store unavailable".to_string()))
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

/// Answer an authenticated, throttle-admitted `agent.query`. Never panics —
/// every resource fn degrades to `Err` on a missing/unexpected state (see
/// `list_autopilots`), and this match's fallback arm covers any resource name
/// [`RESOURCES`] doesn't recognize. Routed through [`reply::bounded_result_reply`] (issue #1151):
/// identifiers are clamped and the reply is frame-capped, substituting `result_too_large` for an
/// oversized SUCCESS payload the same way `agent_call::handle_agent_call` already does for the
/// generic tier.
pub(super) async fn handle_agent_query(app: &AppHandle, req_id: &str, payload: &Value) -> String {
    let resource = reply::resource_name(payload).to_string();
    let outcome = match resource.as_str() {
        RES_BEST_MATCHES => best_matches::best_matches_resource(app, payload).await,
        RES_JOB => job::job_resource(app, payload),
        RES_PROFILE => profile_resource(app),
        RES_AUTOMATIONS => automations::automations_resource(app),
        RES_FOUND_JOBS => found_jobs::found_jobs_resource(app, payload),
        RES_DOCUMENTS => documents::documents_resource(app, payload),
        RES_PREP => prep::prep_resource(app, payload),
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
    reply::bounded_result_reply(req_id, &resource, outcome)
}

#[cfg(test)]
mod tests;
