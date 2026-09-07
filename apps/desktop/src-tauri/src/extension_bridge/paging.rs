//! Offset/limit paging primitives shared by every agent-facing surface that
//! serves pages over an unbounded array THIS process already owns.
//!
//! Extracted from `agent_read::found_jobs` (issue #1115, the first surface to
//! need them) when `agent_call`'s generic tier needed the identical three
//! decisions for `applications_list`/`ai_generations_list` (issue #1136): how
//! a cursor is read, how a `limit` is clamped, and how a page is cut down to
//! a byte budget. Two hand-typed copies of "parse a cursor" is exactly the
//! shape this repo has already been bitten by — the `{"cursor": 100}`
//! silently-collapses-to-page-1 defect ([`parse_offset_cursor`]'s own doc)
//! was fixed once, in one of the copies, and a second copy would have kept
//! it alive on the other surface.
//!
//! Nothing here knows about any resource, envelope, or error type: the
//! per-surface constants (default/max limit, byte budget) and the mapping
//! from [`INVALID_CURSOR_MESSAGE`] onto that surface's own error/refusal type
//! stay with the caller. A plain offset was chosen over an opaque token for
//! the reasons `agent_read::found_jobs::resolve_found_jobs` documents at
//! length; this module inherits that decision rather than re-making it.

use serde_json::Value;

/// A caller-supplied `cursor` that isn't a plain non-negative integer. A
/// FIXED string that never echoes the offending value — both callers render
/// it into a reply an LLM reads, and neither has any use for the bad value
/// being repeated back at it.
pub(super) const INVALID_CURSOR_MESSAGE: &str = "cursor must be a non-negative integer offset";

/// Parse `payload`'s `cursor` — absent (or explicit `null`) means "start at
/// 0"; anything else that doesn't parse as a plain non-negative integer is a
/// caller error (never silently reset to page 1, which would look like
/// forward progress while actually restarting the traversal). Matches on
/// the `Value` variant directly (HIGH fix, PR #1117 pre-PR review round 2)
/// rather than `.and_then(Value::as_str)`: that combinator returns `None` for
/// a JSON NUMBER cursor too, not just for an absent one, so `{"cursor": 100}`
/// used to collapse silently to `Ok(0)` instead of being read as offset 100
/// or rejected — exactly the failure mode this function's own contract
/// promises never happens.
///
/// `Err` is [`INVALID_CURSOR_MESSAGE`] rather than a typed error so this
/// module stays free of any one surface's error vocabulary: `found-jobs`
/// maps it onto `AppError::Validation`, the generic dispatch tier onto its
/// own `Refusal`.
pub(super) fn parse_offset_cursor(payload: &Value) -> Result<usize, &'static str> {
    match payload.get("cursor") {
        None | Some(Value::Null) => Ok(0),
        Some(Value::String(raw)) => raw.parse::<usize>().map_err(|_| INVALID_CURSOR_MESSAGE),
        Some(_) => Err(INVALID_CURSOR_MESSAGE),
    }
}

/// Clamp `payload`'s `limit` into `[1, max]`, defaulting to `default`. A
/// zero, negative, non-numeric or absent `limit` all fall back to `default`
/// — never to "unbounded", which is the one interpretation that would defeat
/// the point of paging at all (`agent-cli-standards`: an empty variable must
/// never widen a selector).
///
/// A row-count limit is a ceiling on how much WORK one call does, never the
/// transport-size guarantee — that is [`trim_to_byte_budget`]'s job, against
/// the real serialized bytes. See `found_jobs::MAX_FOUND_JOBS_LIMIT`'s doc
/// for the review that proved a count alone insufficient.
pub(super) fn clamp_limit(payload: &Value, default: usize, max: usize) -> usize {
    payload
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .filter(|&n| n > 0)
        .unwrap_or(default)
        .min(max)
}

/// Drop rows from the end of `candidates` until `base_cost` PLUS the
/// serialized array of them fits `budget` — the transport-size guarantee is
/// the FULL response envelope, not just its row array (CodeRabbit finding,
/// PR #1117 review round 3: the array-only version left the sibling envelope
/// fields entirely uncounted, and some of them carry unbounded user text).
/// `base_cost` is the caller-measured byte size of every OTHER envelope field
/// combined (see each caller's own call site for how it's derived) — passed
/// in rather than measured here so this stays a pure "fit N pre-serialized
/// rows into a byte budget" primitive, not coupled to one envelope's shape.
///
/// Walks forward summing each row's OWN serialized length (plus a one-byte
/// array separator per row after the first) rather than re-serializing the
/// whole growing array on every step, so this is O(n) `to_string` calls
/// total, not O(n²). Always keeps at least one row when `candidates` is
/// non-empty — the forward-progress guarantee every cursor traversal built on
/// this depends on: a page that returned zero rows AND a `nextCursor` that
/// never advanced would hang the traversal forever. The guarantee is "at
/// least one row survives", not "the result is provably under `budget` no
/// matter how large one row or `base_cost` is".
pub(super) fn trim_to_byte_budget(
    candidates: Vec<Value>,
    base_cost: usize,
    budget: usize,
) -> Vec<Value> {
    let budget_for_rows = budget.saturating_sub(base_cost);
    let mut cumulative = 2; // the array's own "[" + "]"
    let mut kept = 0;
    for (i, row) in candidates.iter().enumerate() {
        let row_len = serde_json::to_string(row).map_or(usize::MAX, |s| s.len());
        let separator = usize::from(i > 0); // a comma between rows
        let next = cumulative + separator + row_len;
        if next > budget_for_rows && kept > 0 {
            break;
        }
        cumulative = next;
        kept = i + 1;
    }
    candidates.into_iter().take(kept).collect()
}
