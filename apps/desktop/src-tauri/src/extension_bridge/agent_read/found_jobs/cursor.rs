//! `found-jobs`' cursor issuing/parsing and the `autopilotId`/`applied`-filter-availability
//! guards — split out of `found_jobs.rs` under the R8 LOC cap.

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::extension_bridge::paging;

use super::filters::FoundJobsFilters;
use super::ALL_AUTOPILOTS_CURSOR_ISSUER;

/// Fixed sentinel — mirrors `agent_read::JOB_NOT_FOUND_MESSAGE`'s "never
/// echo the caller's own id" discipline.
pub(in crate::extension_bridge::agent_read) const AUTOPILOT_NOT_FOUND_MESSAGE: &str =
    "no autopilot found for this id";

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
pub(in crate::extension_bridge::agent_read) const WRONG_AUTOPILOT_CURSOR_MESSAGE: &str =
    "cursor was issued for a different autopilotId scope or filter arguments — page that same \
     scope and filters with it, or restart this one from `cursor: null`";

/// A `cursor` that isn't a nextCursor SHAPE at all: a legacy bare offset, a
/// JSON number, or anything else unparseable. The recovery differs from
/// [`WRONG_AUTOPILOT_CURSOR_MESSAGE`]'s — there is no list this value pages,
/// so the only way forward is a fresh traversal. Fixed sentinel, same
/// never-echo discipline.
pub(in crate::extension_bridge::agent_read) const MALFORMED_CURSOR_MESSAGE: &str =
    "cursor must be a nextCursor returned by a found-jobs page — a bare offset is not one; \
     restart from `cursor: null`";

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
pub(super) fn found_jobs_cursor_issuer(
    autopilot_id: Option<&str>,
    filters: &FoundJobsFilters,
) -> String {
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
pub(super) fn parse_found_jobs_cursor(payload: &Value, cursor_issuer: &str) -> AppResult<usize> {
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
pub(in crate::extension_bridge::agent_read) const BLANK_AUTOPILOT_ID_MESSAGE: &str =
    "autopilotId must be a non-empty id, not blank or flag-shaped — omit the key entirely to \
     span every autopilot";

pub(super) fn parse_autopilot_id_arg(payload: &Value) -> AppResult<Option<String>> {
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
/// is a separate concern, handled by [`resolve_found_jobs_for_store`]
/// omitting the key entirely (plus `appliedUnavailable: true` on the
/// envelope) rather than emitting a confident `false` (round-4 fix T3).
/// `store_present` is a plain `bool`, not an `AppHandle`
/// — this crate has no `tauri::test` mock-app harness (see
/// `commands::autopilot::tests::every_record_mutation_goes_through_mutate_record`'s
/// own doc) — so the refusal itself stays unit-testable without one.
pub(in crate::extension_bridge::agent_read) const APPLIED_FILTER_UNAVAILABLE_MESSAGE: &str =
    "the applications store is unavailable, so the `applied` filter cannot be answered — omit \
     `applied` to read the corpus without that filter";

pub(super) fn check_applied_filter_available(
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
