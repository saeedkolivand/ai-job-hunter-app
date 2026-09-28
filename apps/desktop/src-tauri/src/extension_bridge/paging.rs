//! Offset/limit paging primitives shared by every agent-facing surface that
//! serves pages over an unbounded array THIS process already owns.
//!
//! Extracted from `agent_read::found_jobs` (issue #1115, the first surface to
//! need them) when `agent_call`'s generic tier needed the same decisions for
//! `applications_list`/`ai_generations_list` (issue #1136): how a `limit` is
//! clamped, how a page is cut down to a byte budget, and how a plain offset
//! cursor is read.
//!
//! **The clamp and the byte budget are the shared primitives; the cursor
//! parse is NOT.** Each surface owns its own cursor VOCABULARY — its error
//! type, its refusal wording, its grammar — and that freedom was exercised
//! immediately: `found-jobs` replaced its plain offset with an issuer-scoped
//! `<autopilotId>:<offset>` cursor (issue #1130) and its own two refusal
//! texts, so [`parse_offset_cursor`] now serves the generic dispatch tier
//! alone. What must never be re-decided per surface is the RULE this parse
//! encodes, and which `found-jobs` re-implements rather than drops: an
//! unreadable cursor refuses, it never silently resets to page 1 — the
//! `{"cursor": 100}` collapse-to-page-1 defect ([`parse_offset_cursor`]'s own
//! doc) was fixed once, in one of two hand-typed copies, and the other kept
//! it alive.
//!
//! Nothing here knows about any resource, envelope, or error type: the
//! per-surface constants (default/max limit, byte budget) and the mapping
//! from a rejected cursor onto that surface's own error/refusal type stay
//! with the caller — which is exactly why [`parse_offset_cursor`] returns an
//! `Option` and not a `Result` carrying one surface's message. A plain offset
//! was chosen over an opaque token for the reasons
//! `agent_read::found_jobs::resolve_found_jobs` documents at length; this
//! module inherits that decision rather than re-making it.

use serde_json::Value;

/// A caller-supplied `cursor` that isn't a plain non-negative integer. A
/// FIXED string that never echoes the offending value — it is rendered into a
/// reply an LLM reads, which has no use for the bad value being repeated back
/// at it. The same discipline `found-jobs` keeps for its own two refusal
/// texts, which are separate constants precisely because its grammar is.
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
/// Returns `None` for a rejected cursor rather than a `Result` carrying a
/// message: a `Result<_, &'static str>` is a stringly error that this crate's
/// own R6 check (`Result<_, String>` outside `error.rs`, a TEXTUAL arch test)
/// cannot see, and it forces one surface's wording onto every caller. The
/// caller attaches its own instead — the generic dispatch tier maps the
/// rejection onto `Refusal::InvalidCursor`, whose `detail` is
/// [`INVALID_CURSOR_MESSAGE`]. `Some(0)` for an
/// absent cursor is a real value and not a fallback for a bad one; the two
/// cases stay distinguishable, which is the whole contract above.
pub(super) fn parse_offset_cursor(payload: &Value) -> Option<usize> {
    match payload.get("cursor") {
        None | Some(Value::Null) => Some(0),
        Some(Value::String(raw)) => raw.parse::<usize>().ok(),
        Some(_) => None,
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

/// FNV-1a over `parts`, with a separator byte folded in between each part so
/// `["ab", "c"]` and `["a", "bc"]` never collide. Deterministic within one
/// run of the app — all a paging cursor's issuer half needs, since a cursor
/// is never persisted across a restart — unlike `std::hash::DefaultHasher`,
/// which the standard library explicitly does NOT guarantee stable even
/// across two `RandomState`s in the same process. Mirrors
/// `autopilot_scheduler::jitter_for`'s own reasoning for the same choice.
///
/// Shared by every surface that must fold its OWN filter/query arguments
/// into a paging cursor's issuer half (issue #1168 round 2, B3-r1-F4): a
/// cursor replayed under DIFFERENT filter arguments must hit the existing
/// wrong-scope refusal, never silently page a different filtered list at a
/// stale offset — the exact hazard issue #1130 closed one level up
/// (`<autopilotId>:<offset>`), reopened the moment `found-jobs` and
/// `best-matches` gained arguments that change which rows a traversal
/// contains without changing the cursor's own scope half.
pub(super) fn fingerprint(parts: &[&str]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash ^= 0xff; // separator between parts, so boundaries can't shift
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:x}")
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

#[cfg(test)]
mod tests;
