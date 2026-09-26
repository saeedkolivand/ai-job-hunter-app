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

/// A well-formed `<issuer>:<offset>` cursor issued by a DIFFERENT scope — a different autopilot,
/// all-autopilots vs scoped, or the SAME scope under DIFFERENT filters (round 2 fix, B3-r1-F4,
/// since [`found_jobs_cursor_issuer`] folds filters into the issuer too). Split from
/// [`MALFORMED_CURSOR_MESSAGE`]: this one is "you are paging the wrong list", recoverable by
/// re-sending the same cursor with the SAME scope AND filters. Fixed sentinel, never echoes the
/// caller's value.
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

/// Fold `autopilot_id`'s scope (or [`ALL_AUTOPILOTS_CURSOR_ISSUER`]) AND every filter argument
/// that changes WHICH rows a traversal contains into the cursor's issuer half (round 2 fix,
/// B3-r1-F4). `include_description` is excluded — it changes a row's CONTENT, never which rows
/// survive, so replaying under a different value stays valid. [`paging::fingerprint`] rather than
/// a literal join: `country`/`query` are caller-typed and could contain `:`/`|` themselves.
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

/// Parse `payload`'s `cursor` against `cursor_issuer` — absent (or explicit `null`) means "start
/// at 0"; anything else that isn't a `<issuer>:<offset>` cursor THIS call's scope issued is a
/// caller error (never silently reset to page 1, which would look like progress while restarting
/// the traversal). Matches on the `Value` variant directly (HIGH fix, round 2) rather than
/// `.and_then(Value::as_str)`, which returns `None` for a JSON NUMBER too — `{"cursor": 100}` used
/// to collapse silently to `Ok(0)`.
///
/// TWO fixed refusal texts, one sentinel (MEDIUM fix, round 4): [`WRONG_AUTOPILOT_CURSOR_MESSAGE`]
/// for the wrong scope (recoverable by paging that same scope) and [`MALFORMED_CURSOR_MESSAGE`]
/// for a legacy bare offset (only recovery: a fresh traversal). Neither echoes the refused value.
/// `rsplit_once` so an id containing `:` still round-trips.
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

/// A present-but-unusable `autopilotId` (blank/whitespace-only, or shaped like a CLI flag) must
/// error rather than silently widen the scope to every autopilot (B3-r1-F2 — `agent-cli-standards`:
/// an empty selector must never mean "all"). Absent (or explicit `null`) is the deliberate issue
/// #1168 case and stays `None`. The `--`-prefix check mirrors `agent_cli::mcp::tool_argv`'s own
/// guard on the SAME field — harmless but redundant defense-in-depth here, since this path never
/// builds argv.
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

/// A missing `ApplicationStore` (a NON-FATAL boot path) yields an EMPTY set, the same shape as
/// "the user has applied to nothing" — harmless for `enrich_applied`'s cosmetic badge, but the
/// `applied` filter this fn adds (issue #1167) can't tell the two apart: `applied: true` would
/// silently answer `total: 0`, and `applied: false` would silently return postings already applied
/// to — the unsafe direction issue #1168 exists to prevent. Refuse instead, but ONLY when the
/// `applied` filter is actually requested. `store_present` is a plain `bool`, not an `AppHandle` —
/// this crate has no `tauri::test` mock-app harness — so the refusal stays unit-testable without one.
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
