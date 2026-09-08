//! `found-jobs` resource (issue #1115) — split out of `agent_read`'s own
//! module under R8's hard LOC cap (`docs/architecture-rules.md`). This is a
//! private implementation detail of `agent_read` — nothing here is visible
//! outside `extension_bridge`, and the only items that reach past
//! `agent_read` itself are the two `limit` constants `agent_cli::mcp` derives
//! its tool schema from (issue #1129). See that module's own doc for the
//! resource-table picture this fits into. Reaches into `super::` for the
//! shared allowlist plumbing (`project_value`, `fence_posting_display_fields`,
//! `list_autopilots`) rather than duplicating any of it — a child module can
//! see its parent's private items, so no visibility widening was needed for
//! that half; only the three helper fns this module's own tests borrow from
//! `agent_read::tests` needed `pub(super)` (see their own doc there).
//!
//! ## Compact rows + server-side filters (issue #1167)
//! A row is compact by default — `title`/`company`/`location`/`score`/
//! `scoreProvisional`/`url`/`foundAt`/`applied`/`isAgency`/`autopilotId`/
//! `autopilotName`, no `description` — because the previous shape (every
//! optional job field plus a 2,000-char description preview on every row)
//! put an ordinary page over what a real MCP client will accept in-band
//! (issue #1167's own measured payload sizes). `description` is opt-in via
//! `includeDescription: true`, still fenced at
//! [`FOUND_JOBS_DESCRIPTION_PREVIEW_CAP`]. Five server-side filters
//! (`minScore`/`country`/`remote`/`applied`/`query`) apply BEFORE paging, so
//! `total` always means "rows this call's filters actually match", never the
//! whole unfiltered store.
//!
//! ## Spanning every autopilot (issue #1168)
//! `autopilotId` is now OPTIONAL. Omitted, the traversal spans every
//! autopilot the store holds, in store order, each list in its own stored
//! order — the one call that answers "is this role already in my list?"
//! (`found-jobs {query: "…"}`) without a per-autopilot fan-out. Rows sharing
//! the same [`canonical_job_key`](crate::scraping::boards::common::canonical_job_key)
//! across two autopilots collapse to the FIRST occurrence in that order
//! that also PASSES this call's own filters (round 2 fix, B3-r1-F1 — dedup
//! used to run before filtering, so a posting that failed a filter under the
//! first autopilot to hold it was dropped even when a later autopilot's copy
//! of the SAME posting would have passed) — the identical identity B2
//! already uses to collapse a run's own duplicates (`autopilot::merge_found_jobs`'s
//! `merge_key`), reused here rather than a second notion of "same job". The
//! cursor is `<issuer>:<offset>`; `issuer` is now itself
//! `<autopilotId or __all__>|<filter fingerprint>` — see
//! [`found_jobs_cursor_issuer`] (round 2 fix, B3-r1-F4 — the plain
//! `<autopilotId or __all__>` issuer (issue #1130) let a cursor replayed
//! under DIFFERENT filter arguments page a different filtered list at a
//! stale offset); either way a cursor is only valid for a later call with
//! the SAME scope AND the SAME filters.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::autopilot::{Autopilot, FoundJob};
use crate::error::{AppError, AppResult};
use crate::extension_bridge::paging;
use crate::scraping::boards::common::canonical_job_key;
use crate::scraping::engine::location_filter::REMOTE_MARKERS;

use super::{fence_posting_display_fields, list_autopilots, project_value};

/// `found-jobs` resource's per-row COMPACT payload — a SMALLER allowlist than
/// `agent_read::AgentJob` over the same `autopilot::FoundJob` source.
/// Deliberately excludes `board`/`salaryMin`/`salaryMax`/`salaryCurrency`/
/// `scoreSource`/`postedAt`/`trust`/`clusterMembers` (issue #1167 — a caller
/// that needs the full detail for ONE job already has `job`, keyed by this
/// same `url`) on top of everything `AgentJob` already excludes
/// (`assistantNotes`, forbidden; `clusterId`/`clusterCanonical`, internal).
/// `scoreProvisional` stays IN — see [`FoundJobSlice`]'s own doc for why.
/// `applied`/`autopilotId`/`autopilotName` are NOT part of this struct's own
/// serde round trip — [`project_found_job_row`] injects them afterward, since
/// none of the three is a plain passthrough of the stored `FoundJob` (applied
/// is derived at read time off [`crate::commands::autopilot::applied_job_urls`],
/// never the always-stale stored bit; the autopilot fields belong to the
/// PARENT `Autopilot`, not the job). `description` is likewise excluded from
/// this struct's round trip and injected separately, only when the caller
/// asked for it — see [`project_found_job_row`].
///
/// `score_provisional` (B3-r1-F5) is a REQUIRED passthrough, not excluded
/// like the rest of `AgentBestMatch`'s trust detail: `found-jobs` is the one
/// resource that now FILTERS by `score` (`minScore`, issue #1167's headline
/// case), and a score computed from a title-only or aggregator-snippet blob
/// is flagged provisional precisely so a caller does not treat it as fully
/// trusted (`build_found_job`'s own doc — LinkedIn plus TheMuse, Comeet,
/// Breezy, BambooHR, Pinpoint, and Rippling all produce title-only rows).
/// Stripping that flag off the one surface that ranks by the number it
/// qualifies would silence the exact warning a `minScore`-filtered caller
/// most needs. `score_source` stays excluded — the provisional flag alone is
/// the actionable "don't trust this" signal; the finer-grained enum is
/// still available via `job` for a caller that needs it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FoundJobSlice {
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

/// `description`'s fence cap for `found-jobs`, distinct from
/// `crate::prompt_fence::JOB_CAP` (used by the single-job `job` resource and
/// by `best-matches`' title/company/location fields) — a caller that needs
/// the full posting text already has `job`, keyed by this same row's `url`.
///
/// **2,000 chars** (up from an original 500 — pre-PR review round 2, HIGH:
/// 500 chars is mostly boilerplate and not enough to actually
/// qualify/dismiss a posting, defeating this resource's whole stated
/// purpose, and — since [`PAGE_BYTE_BUDGET`] is now what actually enforces
/// the transport cap, not a per-field size assumption — there is no longer
/// a reason to starve every row for that cap's sake). `description` is now
/// opt-in (issue #1167), so this cap only ever applies to a caller that asked
/// for it via `includeDescription: true`.
const FOUND_JOBS_DESCRIPTION_PREVIEW_CAP: usize = 2_000;

/// Server-side default/cap for `found-jobs`' `limit` — a CEILING on how much
/// work one call does (project + fence up to this many rows before
/// trimming), never the actual transport-size guarantee. That guarantee is
/// [`PAGE_BYTE_BUDGET`] (below), enforced by [`trim_page_to_budget`] against
/// the REAL serialized bytes of whatever rows actually came back.
///
/// `pub(in crate::extension_bridge)` (issue #1129) — `agent_cli::mcp` derives
/// the `found-jobs` tool schema's `limit` description from these two numbers
/// instead of retyping them, which is how the advertised 50/100 drifted from
/// the enforced 25/50 in the first place. Visible to the whole bridge, not
/// merely to `agent_read`, because that consumer is a sibling subtree; the
/// module declaration itself is widened to match (see `agent_read`'s
/// `pub(super) mod found_jobs;`).
pub(in crate::extension_bridge) const DEFAULT_FOUND_JOBS_LIMIT: usize = 25;
pub(in crate::extension_bridge) const MAX_FOUND_JOBS_LIMIT: usize = 50;

/// The REAL per-response safety net (pre-PR review round 2, HIGH — a
/// row-count limit cannot bound a page's byte size because a legitimate,
/// non-adversarial posting's title/company/location can each independently
/// reach `crate::prompt_fence::JOB_CAP` = 8,000 chars). [`trim_page_to_budget`]
/// checks the ACTUAL serialized bytes of the candidate page and drops rows
/// from the end — content-independent and exact, unlike trusting any
/// per-row size assumption. Since a compact row (issue #1167) no longer
/// carries a mandatory description, trimming now fires far less often than
/// it did against the old always-2,000-char-description shape — but a
/// caller that opts into `includeDescription` can still reach it, so the
/// guard stays.
///
/// Target: half of `agent_cli::mcp::MCP_RESULT_MAX_BYTES` (256 KiB = 262,144
/// B), leaving real margin for the MCP `content[]`/`isError` wrapper this
/// payload rides inside on the MCP transport — [`trim_page_to_budget`]'s
/// `base_cost` parameter (see [`resolve_found_jobs`]'s call site) accounts
/// for the REST of this resource's own envelope, so this budget is the FULL
/// response, not merely the `jobs` array.
const PAGE_BYTE_BUDGET: usize = 150_000;

/// Cap on `autopilotName` before it enters the response envelope or a row
/// (CodeRabbit finding, PR #1117 review — an autopilot's name is user-typed
/// and unbounded). 200 chars is generous for the short single-line name the
/// CreationWizard collects, while making the cap a CONCRETE bound rather
/// than "trust the UI never lets this grow".
const AUTOPILOT_NAME_FENCE_CAP: usize = 200;

/// Sentinel cursor issuer for a traversal spanning EVERY autopilot (issue
/// #1168 — `autopilotId` is optional). Never a value
/// [`Uuid::new_v4`](uuid::Uuid::new_v4) (the real id generator, see
/// `Autopilot::create`) can produce, so it can never collide with a real
/// autopilot id and be misread as a scoped cursor.
const ALL_AUTOPILOTS_CURSOR_ISSUER: &str = "__all__";

/// Fence `name` the same way every other display field on this resource
/// already is — same primitive, same `"job_posting"` tag as
/// [`fence_found_jobs_description`]/`agent_read::fence_posting_display_fields`,
/// even though an autopilot name is the CALLER'S OWN data rather than
/// scraped text: the primitive is exactly "cap length and neutralize any
/// embedded fence syntax," which is what this field needs regardless of
/// provenance, and one shared convention ("every text field on this
/// resource is fenced") is simpler than a per-field exception.
fn fence_autopilot_name(name: &str) -> String {
    crate::prompt_fence::fenced("job_posting", name, AUTOPILOT_NAME_FENCE_CAP)
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
fn trim_page_to_budget(candidates: Vec<Value>, base_cost: usize) -> Vec<Value> {
    paging::trim_to_byte_budget(candidates, base_cost, PAGE_BYTE_BUDGET)
}

/// Every envelope byte OTHER than the `jobs` array itself, measured (not
/// assumed) against the REAL fenced `autopilotId`/`autopilotName` a scoped
/// response carries — the fix for the gap the shared trim primitive's own
/// doc names (CodeRabbit, PR #1117 review round 3), and the `base_cost`
/// [`trim_page_to_budget`] subtracts from [`PAGE_BYTE_BUDGET`]. `None` for a
/// call spanning every autopilot (issue #1168), which carries neither
/// envelope-level field.
///
/// `nextCursor` isn't known when this runs (it depends on how many rows
/// survive trimming), so it is measured in the SAME `<issuer>:<offset>` shape
/// a real cursor has (issue #1130), with `total` standing in for the offset:
/// a real offset can never exceed `total`, so the estimate can only ever
/// OVER-count and thus only trim MORE aggressively than strictly required,
/// never less (the safe direction for a byte budget).
fn base_envelope_cost(cursor_issuer: &str, single: Option<(&str, &str)>, total: usize) -> usize {
    let mut base_envelope = json!({
        "jobs": [],
        "nextCursor": format!("{cursor_issuer}:{total}"),
        "total": total,
    });
    if let Some((id, name)) = single {
        base_envelope["autopilotId"] = json!(id);
        base_envelope["autopilotName"] = json!(name);
    }
    serde_json::to_string(&base_envelope)
        .map_or(usize::MAX, |s| s.len())
        .saturating_sub(2)
}

/// Fixed sentinel — mirrors `agent_read::JOB_NOT_FOUND_MESSAGE`'s "never
/// echo the caller's own id" discipline.
const AUTOPILOT_NOT_FOUND_MESSAGE: &str = "no autopilot found for this id";

/// A well-formed `<issuer>:<offset>` cursor issued by a DIFFERENT scope (a
/// different autopilot, or the all-autopilots traversal vs a scoped one, or
/// the SAME autopilot scope under DIFFERENT filter arguments — round 2 fix,
/// B3-r1-F4, since [`found_jobs_cursor_issuer`] now folds the active filters
/// into the issuer too) — the issue #1130 case, widened for #1168's optional
/// `autopilotId` and again for the filter fingerprint. Split from
/// [`MALFORMED_CURSOR_MESSAGE`] because the two have different recoveries:
/// this one is "you are paging the wrong list", where re-sending the same
/// cursor with the SAME `autopilotId` (present or omitted) AND the SAME
/// filters it was issued under works. Fixed sentinel — the caller's value is
/// never echoed back, same discipline as [`AUTOPILOT_NOT_FOUND_MESSAGE`].
const WRONG_AUTOPILOT_CURSOR_MESSAGE: &str =
    "cursor was issued for a different autopilotId scope or filter arguments — page that same \
     scope and filters with it, or restart this one from `cursor: null`";

/// A `cursor` that isn't a nextCursor SHAPE at all: a legacy bare offset, a
/// JSON number, or anything else unparseable. The recovery differs from
/// [`WRONG_AUTOPILOT_CURSOR_MESSAGE`]'s — there is no list this value pages,
/// so the only way forward is a fresh traversal. Fixed sentinel, same
/// never-echo discipline.
const MALFORMED_CURSOR_MESSAGE: &str =
    "cursor must be a nextCursor returned by a found-jobs page — a bare offset is not one; \
     restart from `cursor: null`";

/// Fence `description` at [`FOUND_JOBS_DESCRIPTION_PREVIEW_CAP`] — the
/// `found-jobs` twin of `agent_read::fence_description`, which uses the
/// larger single-job cap instead.
fn fence_found_jobs_description(value: &mut Value) {
    let Some(desc) = value.get("description").and_then(Value::as_str) else {
        return;
    };
    let fenced =
        crate::prompt_fence::fenced("job_posting", desc, FOUND_JOBS_DESCRIPTION_PREVIEW_CAP);
    value["description"] = json!(fenced);
}

/// Server-side filters for `found-jobs` (issue #1167). Every predicate here
/// is the app's OWN, already-established one — never a fresh matcher invented
/// for this surface:
/// - `remote` reuses the exact scrape-time
///   [`location_verdict`](crate::scraping::engine::location_filter::location_verdict)
///   two-branch check: `job.board_remote` (the board's own per-posting
///   classification) OR the
///   [`REMOTE_MARKERS`](crate::scraping::engine::location_filter::REMOTE_MARKERS)
///   list against `location` text — `location` text alone under-counts an
///   all-remote board that stores no location, or a jurisdiction string
///   ("USA Only") with no marker word (round-3 fix, H1).
/// - `country` is a case-insensitive substring match against `location` —
///   the SAME predicate the Jobs page's own free-text filter applies to a
///   posting's location (`(p.location ?? '').toLowerCase().includes(q)` in
///   `JobsPage`), not a structured country-code compare: `FoundJob` carries
///   no `countryCode` field, and `commands::match_resume::constraints`
///   documents that a bare country code "contributes nothing to the
///   matchable token" for this exact reason — only place text does.
/// - `query` mirrors that same JobsPage substring filter's title/company
///   half (its location half becomes the separate `country` filter above).
/// - `applied` reads the SAME derived-at-read-time set
///   [`crate::commands::autopilot::applied_job_urls`] produces for
///   `best-matches`/`autopilot_list`, never the stale stored bit.
// `pub(super)` — `agent_read::tests`' `no_resource_output_ever_carries_a_forbidden_key` and
// `automations_found_jobs_total_matches_found_jobs_own_total` build a `FoundJobsFilters` to call
// `resolve_found_jobs` directly (a sibling module, not a descendant of this one, needs the same
// widening `found_jobs::tests` gets automatically as a child).
#[derive(Debug)]
pub(super) struct FoundJobsFilters {
    min_score: Option<f64>,
    /// Lowercased.
    country: Option<String>,
    remote: Option<bool>,
    applied: Option<bool>,
    /// Lowercased.
    query: Option<String>,
    include_description: bool,
}

/// One shared refusal for every filter argument below that is PRESENT but
/// not readable as its declared shape (B3-r1-F3 — a wrong-typed value, e.g.
/// `{"minScore": "70"}` off the raw `agent.query` payload path, used to
/// vanish silently through `.and_then(Value::as_*)` returning `None` for a
/// mismatch exactly like it does for "absent") — and, for the two string
/// filters, also PRESENT-but-blank (B3-r2-F2, see
/// [`trimmed_lowercase_filter`]'s own doc). The caller got an UNFILTERED
/// page back with a `total` it read as filtered. Refusing here instead means
/// the filter this call asked for either applies or the call fails loudly —
/// never a third, silent option. Names the KEY, not the caller's value
/// (never echoed) — the key is this resource's own static schema, not
/// caller data.
fn unreadable_filter_message(key: &str) -> AppError {
    AppError::Validation(format!(
        "{key} was present but not usable as its declared type — remove it or fix its value"
    ))
}

/// `payload.get(key)`, refusing anything present that is neither absent/
/// `null` nor a non-blank JSON string. A PRESENT-but-blank/whitespace-only
/// string now refuses too (round 2 fix, B3-r2-F2 — it used to read as
/// "filter not set", silently widening the call to the entire corpus with a
/// `total` the caller reads as the filtered count; the canonical repro is a
/// shell caller forwarding an unset variable, e.g. `--query "$ROLE"` with
/// `ROLE` empty). There is no legitimate caller that types an explicitly
/// empty filter, so this now mirrors `parse_autopilot_id_arg`'s blank-must-
/// refuse rule even though `query`/`country` are additive filters, not
/// selectors — only the OMITTED key still means "no filter".
///
/// `pub(super)` (round 2 fix, B3-r2-F1) so `agent_read::best_matches_resource`
/// reuses this SAME fallible parse for its own `query` argument rather than
/// the bare `.and_then(Value::as_str)` combinator that let a wrong-typed or
/// blank `query` collapse silently to "absent" on that resource too.
pub(super) fn trimmed_lowercase_filter(payload: &Value, key: &str) -> AppResult<Option<String>> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Err(unreadable_filter_message(key))
            } else {
                Ok(Some(trimmed.to_lowercase()))
            }
        }
        Some(_) => Err(unreadable_filter_message(key)),
    }
}

/// `payload.get(key)`, refusing anything present that is neither absent/
/// `null` nor a JSON boolean.
fn bool_filter(payload: &Value, key: &str) -> AppResult<Option<bool>> {
    match payload.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(unreadable_filter_message(key)),
    }
}

impl FoundJobsFilters {
    /// Fallible (B3-r1-F3) — a filter key that IS present must either parse
    /// as its declared shape or refuse the whole call; it can no longer
    /// silently collapse to "no filter" the way `.and_then(Value::as_*)`
    /// alone would for a wrong-typed value.
    ///
    /// `minScore`'s `is_finite()` guard is defense-in-depth, not the fix for
    /// the non-finite `--min-score` repro (`1e400`/`inf`/`nan`): RFC 8259
    /// has no `Infinity`/`NaN` token, so `json!(non_finite_f64)` collapses
    /// to `null` BEFORE this ever runs, and `None | Some(Value::Null) =>
    /// None` below already treats that the same as "absent" — the
    /// established, intentional convention for every filter/cursor here,
    /// not a bug. The load-bearing half of that fix is upstream, at the
    /// CLI's own `--min-score` parse (`agent_cli::parse_found_jobs`), which
    /// refuses the non-finite value before it is ever handed to `json!` —
    /// see that fn's own doc and
    /// `found_jobs::tests::found_jobs_filters_from_payload_treats_a_null_min_score_as_absent`.
    pub(super) fn from_payload(payload: &Value) -> AppResult<Self> {
        let min_score = match payload.get("minScore") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_f64()
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| unreadable_filter_message("minScore"))?,
            ),
        };
        Ok(Self {
            min_score,
            country: trimmed_lowercase_filter(payload, "country")?,
            remote: bool_filter(payload, "remote")?,
            applied: bool_filter(payload, "applied")?,
            query: trimmed_lowercase_filter(payload, "query")?,
            include_description: bool_filter(payload, "includeDescription")?.unwrap_or(false),
        })
    }
}

/// True when `job` survives every filter set in `filters`. `is_applied` is
/// passed in (precomputed once per job by [`candidate_jobs`]) rather than
/// recomputed here, so the SAME derivation backs both this filter and the
/// row's own `applied` field.
fn passes_filters(job: &FoundJob, filters: &FoundJobsFilters, is_applied: bool) -> bool {
    if let Some(min) = filters.min_score {
        match job.score {
            Some(s) if s >= min => {}
            _ => return false,
        }
    }
    if filters.country.is_some() || filters.remote.is_some() {
        let loc = job.location.as_deref().unwrap_or("").to_lowercase();
        if let Some(country) = &filters.country {
            if !loc.contains(country.as_str()) {
                return false;
            }
        }
        if let Some(want_remote) = filters.remote {
            // Mirrors `location_verdict`'s own two-branch remote check
            // (`board_remote` short-circuits first, THEN the marker scan) —
            // a `location` string alone under-counts an all-remote board
            // that stores no location (`location: None`) or a jurisdiction
            // string with no marker word ("USA Only"). Round-3 fix (H1):
            // `job.board_remote` used to be missing from this OR entirely,
            // so `--remote true` silently dropped those postings.
            let is_remote = job.board_remote || REMOTE_MARKERS.iter().any(|m| loc.contains(m));
            if is_remote != want_remote {
                return false;
            }
        }
    }
    if let Some(want_applied) = filters.applied {
        if is_applied != want_applied {
            return false;
        }
    }
    if let Some(q) = &filters.query {
        let title = job.title.to_lowercase();
        let company = job.company.to_lowercase();
        if !title.contains(q.as_str()) && !company.contains(q.as_str()) {
            return false;
        }
    }
    true
}

/// The ordered, filtered candidate list across every autopilot in `scoped` —
/// ready to be sliced `[offset, offset + limit)`. `dedupe_across_autopilots`
/// (issue #1168) additionally collapses rows sharing the same
/// [`canonical_job_key`] to their FIRST occurrence THAT ALSO PASSES this
/// call's filters (round 2 fix, B3-r1-F1 — filtering runs before dedup, not
/// after, so a copy that fails a filter never consumes the dedup slot a
/// later, passing copy needed) — needed ONLY for a spanning traversal
/// (`autopilot_id: None`), where the same posting can legitimately surface
/// in more than one autopilot's own list, each scored against that
/// autopilot's own resume. Scoped to
/// exactly one autopilot, `false`: that list is already deduped at merge
/// time (`autopilot::merge_found_jobs`), and `automations`' own
/// `foundJobsTotal` promises `total` here equals that list's plain
/// `found_jobs.len()` (pinned by
/// `agent_read::tests::automations_found_jobs_total_matches_found_jobs_own_total`)
/// — re-deduping would silently break that promise the moment a caller's
/// stored data isn't ALREADY deduped for some other reason (a legacy/
/// migrated record, a hand-built test fixture), so the single-autopilot path
/// stays a byte-for-byte passthrough of the stored list's own count.
fn candidate_jobs<'a>(
    scoped: &[&'a Autopilot],
    filters: &FoundJobsFilters,
    applied_urls: &HashSet<String>,
    dedupe_across_autopilots: bool,
) -> Vec<(&'a Autopilot, &'a FoundJob, bool)> {
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
            let is_applied =
                applied_urls.contains(&crate::applications::normalize_job_url(&job.url));
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

/// Project one row: [`FoundJobSlice`]'s allowlist round trip, plus the three
/// fields that round trip can't carry (see that struct's own doc) —
/// `applied` (precomputed), `autopilotId`/`autopilotName` (the PARENT
/// record's, fenced), and `description` (only when `include_description`).
fn project_found_job_row(
    job: &FoundJob,
    autopilot: &Autopilot,
    include_description: bool,
    is_applied: bool,
) -> Option<Value> {
    let mut value = project_value::<_, FoundJobSlice>(job)?;
    if include_description {
        if let Some(desc) = &job.description {
            value["description"] = json!(desc);
            fence_found_jobs_description(&mut value);
        }
    }
    fence_posting_display_fields(&mut value);
    value["applied"] = json!(is_applied);
    value["autopilotId"] = json!(autopilot.id);
    value["autopilotName"] = json!(fence_autopilot_name(&autopilot.name));
    Some(value)
}

/// Fold `autopilot_id`'s scope (or [`ALL_AUTOPILOTS_CURSOR_ISSUER`] spanning
/// every autopilot) AND every filter argument that changes WHICH rows a
/// traversal contains into the cursor's issuer half (round 2 fix, B3-r1-F4).
/// `include_description` is deliberately excluded — it changes a row's
/// CONTENT, never which rows survive or their order, so replaying a cursor
/// under a different `includeDescription` is harmless and must stay valid.
/// [`paging::fingerprint`] rather than a literal join of the filter values:
/// `country`/`query` are caller-typed strings that could themselves contain
/// `:` or `|`, and a fingerprint sidesteps needing to prove they can never
/// collide with the issuer's own delimiters.
fn found_jobs_cursor_issuer(autopilot_id: Option<&str>, filters: &FoundJobsFilters) -> String {
    let scope = autopilot_id.unwrap_or(ALL_AUTOPILOTS_CURSOR_ISSUER);
    let fp = paging::fingerprint(&[
        &filters.min_score.map(|n| n.to_string()).unwrap_or_default(),
        filters.country.as_deref().unwrap_or(""),
        &filters.remote.map(|b| b.to_string()).unwrap_or_default(),
        &filters.applied.map(|b| b.to_string()).unwrap_or_default(),
        filters.query.as_deref().unwrap_or(""),
    ]);
    format!("{scope}|{fp}")
}

/// Pure core of `found-jobs`: resolve the requested scope (one autopilot, or
/// every autopilot when `autopilot_id` is `None` — issue #1168), build the
/// filtered/deduped candidate list (issue #1167), slice it at
/// `[offset, offset + limit)`, project + fence each surviving row, then
/// [`trim_page_to_budget`] the result before returning it.
///
/// `offset` is a plain index into the candidate list THIS CALL'S filters
/// produce — stable across calls only as long as NONE of three inputs
/// change between them: the underlying stored order, the filter arguments,
/// and (round 2 fix, B3-r2-F6) `applied_urls` — a fresh, live re-derivation
/// on every call (see [`found_jobs_resource`]'s
/// `commands::autopilot::applied_job_urls(app)`), not a stored bit. That
/// third input breaks the guarantee the ORIGINAL, pre-#1167 doc here made:
/// back when the only drift source was a `record_run` merge PREPENDING new
/// jobs, a stale offset could only ever produce a DUPLICATE (nothing is
/// ever removed from a stored `found_jobs` list), never a skip. `applied`
/// (and, on the spanning path, cross-autopilot dedup) can REMOVE a row from
/// the middle of the candidate list mid-traversal — a job applied to
/// between two calls drops out under `applied: false`, shifting every LATER
/// index down by one, so the next page at the stale offset silently skips
/// exactly one row instead of repeating it. `total` moving between calls
/// (in EITHER direction, not just growing) is the caller-visible signal;
/// the recovery is unchanged — restart from `cursor: null` — but a
/// SHRINKING `total` is the one that can hide a missed row rather than
/// merely repeat one.
///
/// Directly unit-testable with hand-built `Autopilot` records, no
/// `AppHandle` — same pure/impure split as `agent_read::resolve_job`/
/// `agent_read::resolve_best_matches`. `pub(super)` because `agent_read`'s
/// own `no_resource_output_ever_carries_a_forbidden_key` test calls this
/// directly to sweep every resource's output in one place.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_found_jobs(
    records: &[Autopilot],
    autopilot_id: Option<&str>,
    filters: &FoundJobsFilters,
    applied_urls: &HashSet<String>,
    offset: usize,
    limit: usize,
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

    let page_values: Vec<Value> = candidates
        .iter()
        .skip(offset)
        .take(limit)
        .filter_map(|(ap, job, is_applied)| {
            project_found_job_row(job, ap, filters.include_description, *is_applied)
        })
        .collect();

    let cursor_issuer = found_jobs_cursor_issuer(autopilot_id, filters);
    let single = match (autopilot_id, scoped.as_slice()) {
        (Some(_), [ap]) => Some(*ap),
        _ => None,
    };
    let autopilot_name_fenced = single.map(|ap| fence_autopilot_name(&ap.name));

    let base_cost = base_envelope_cost(
        &cursor_issuer,
        single
            .zip(autopilot_name_fenced.as_deref())
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
    if let (Some(ap), Some(name)) = (single, autopilot_name_fenced) {
        envelope["autopilotId"] = json!(ap.id);
        envelope["autopilotName"] = json!(name);
    }
    Ok(envelope)
}

/// Parse `payload`'s `cursor` against `cursor_issuer` (the requested
/// `autopilotId`, or [`ALL_AUTOPILOTS_CURSOR_ISSUER`] when spanning every
/// autopilot) — absent (or explicit `null`) means "start at 0"; anything
/// else that isn't a `<issuer>:<offset>` cursor THIS call's own scope issued
/// is a caller error (never silently reset to page 1, which would look like
/// forward progress while actually restarting the traversal). Matches on
/// the `Value` variant directly (HIGH fix, pre-PR review round 2) rather
/// than `.and_then(Value::as_str)`: that combinator returns `None` for a
/// JSON NUMBER cursor too, not just for an absent one, so `{"cursor": 100}`
/// used to collapse silently to `Ok(0)` instead of being read as offset 100
/// or rejected — exactly the failure mode this function's own contract
/// promises never happens.
///
/// TWO fixed refusal texts, one sentinel kind (MEDIUM fix, review round 4):
/// [`WRONG_AUTOPILOT_CURSOR_MESSAGE`] when a real cursor is replayed against
/// the wrong scope — recoverable by paging that same scope — and
/// [`MALFORMED_CURSOR_MESSAGE`] for a legacy bare offset or any other
/// non-cursor, whose only recovery is a fresh traversal. Neither ever echoes
/// the value it refused. `rsplit_once` so an id that ever contains `:`
/// still round-trips.
fn parse_found_jobs_cursor(payload: &Value, cursor_issuer: &str) -> AppResult<usize> {
    let malformed = || AppError::Validation(MALFORMED_CURSOR_MESSAGE.to_string());
    match payload.get("cursor") {
        None | Some(Value::Null) => Ok(0),
        // SHAPE first, issuer second: only a value that really is
        // `<issuer>:<offset>` can have a meaningfully WRONG issuer.
        Some(Value::String(raw)) => {
            match raw
                .rsplit_once(':')
                .and_then(|(issuer, offset)| Some((issuer, offset.parse::<usize>().ok()?)))
            {
                Some((issuer, offset)) if issuer == cursor_issuer => Ok(offset),
                Some(_) => Err(AppError::Validation(
                    WRONG_AUTOPILOT_CURSOR_MESSAGE.to_string(),
                )),
                None => Err(malformed()),
            }
        }
        Some(_) => Err(malformed()),
    }
}

/// A present-but-unusable `autopilotId` (blank/whitespace-only, or shaped
/// like a CLI flag) must error rather than silently widen the scope to every
/// autopilot (B3-r1-F2 — `agent-cli-standards`: an empty selector must never
/// mean "all"; this is a SELECTOR, unlike the additive filters
/// [`trimmed_lowercase_filter`] covers). Absent (or explicit `null`) is the
/// deliberate issue #1168 case and stays `None`. The `--`-prefix check
/// mirrors `agent_cli::mcp::tool_argv`'s own guard on the SAME field (round
/// 2 fix — that layer forwards this value as a bare CLI positional, where a
/// flag-shaped id would otherwise be misread as the flag itself rather than
/// refused); harmless but redundant defense-in-depth here, since this path
/// never builds argv.
const BLANK_AUTOPILOT_ID_MESSAGE: &str =
    "autopilotId must be a non-empty id, not blank or flag-shaped — omit the key entirely to \
     span every autopilot";

fn parse_autopilot_id_arg(payload: &Value) -> AppResult<Option<String>> {
    match payload.get("autopilotId") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                Err(AppError::Validation(BLANK_AUTOPILOT_ID_MESSAGE.to_string()))
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        Some(_) => Err(AppError::Validation(BLANK_AUTOPILOT_ID_MESSAGE.to_string())),
    }
}

/// `commands::autopilot::applied_job_urls`'s own doc: a missing
/// `ApplicationStore` (an explicitly NON-FATAL boot path — `lib.rs`'s setup
/// leaves it unmanaged rather than failing) yields an EMPTY set, the same
/// shape as "the user has applied to nothing". That collapse is harmless for
/// `enrich_applied`'s cosmetic badge, but the `applied` filter this fn adds
/// (issue #1167) cannot tell the two apart: `applied: true` would silently
/// answer `total: 0` for every autopilot, and `applied: false` would
/// silently return the WHOLE corpus, including postings already applied to
/// — the unsafe direction for a filter issue #1168 exists specifically to
/// prevent a duplicate application. Refuse instead, but ONLY when the
/// `applied` filter is actually requested — the row-level `applied` badge
/// (always emitted) keeps `enrich_applied`'s existing best-effort semantics,
/// out of scope here. `store_present` is a plain `bool`, not an `AppHandle`
/// — this crate has no `tauri::test` mock-app harness (see
/// `commands::autopilot::tests::every_record_mutation_goes_through_mutate_record`'s
/// own doc) — so the refusal itself stays unit-testable without one.
const APPLIED_FILTER_UNAVAILABLE_MESSAGE: &str =
    "the applications store is unavailable, so the `applied` filter cannot be answered — omit \
     `applied` to read the corpus without that filter";

fn check_applied_filter_available(
    store_present: bool,
    filters: &FoundJobsFilters,
) -> AppResult<()> {
    if filters.applied.is_some() && !store_present {
        Err(AppError::Validation(
            APPLIED_FILTER_UNAVAILABLE_MESSAGE.to_string(),
        ))
    } else {
        Ok(())
    }
}

/// `pub(super)` — dispatched from `agent_read::handle_agent_query`.
pub(super) fn found_jobs_resource(app: &AppHandle, payload: &Value) -> AppResult<Value> {
    let autopilot_id = parse_autopilot_id_arg(payload)?;
    let filters = FoundJobsFilters::from_payload(payload)?;
    check_applied_filter_available(
        app.try_state::<crate::applications::ApplicationStore>()
            .is_some(),
        &filters,
    )?;
    let cursor_issuer = found_jobs_cursor_issuer(autopilot_id.as_deref(), &filters);
    let offset = parse_found_jobs_cursor(payload, &cursor_issuer)?;
    let limit = clamp_found_jobs_limit(payload);
    let records = list_autopilots(app)?;
    let applied_urls = crate::commands::autopilot::applied_job_urls(app);
    resolve_found_jobs(
        &records,
        autopilot_id.as_deref(),
        &filters,
        &applied_urls,
        offset,
        limit,
    )
}

#[cfg(test)]
mod tests;
