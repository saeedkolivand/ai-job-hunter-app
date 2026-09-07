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
mod tests {
    use serde_json::json;

    use super::*;

    // ── clamp_limit ───────────────────────────────────────────────────────

    /// Both bounds AND every "no usable limit" shape in one place. The
    /// zero/negative/string/absent cases all land on `default` and NEVER on
    /// "unbounded" — the one interpretation that would defeat paging
    /// entirely (`agent-cli-standards`: an empty variable must never widen a
    /// selector).
    #[test]
    fn clamp_limit_defaults_every_unusable_shape_and_caps_at_the_max() {
        for payload in [
            json!({}),
            json!({ "limit": 0 }),
            json!({ "limit": -5 }),
            json!({ "limit": "20" }),
            json!({ "limit": null }),
        ] {
            assert_eq!(
                clamp_limit(&payload, 20, 100),
                20,
                "must fall back to the default, never to unbounded: {payload}"
            );
        }
        assert_eq!(clamp_limit(&json!({ "limit": 7 }), 20, 100), 7);
        assert_eq!(clamp_limit(&json!({ "limit": 100 }), 20, 100), 100);
        assert_eq!(
            clamp_limit(&json!({ "limit": 10_000 }), 20, 100),
            100,
            "an over-max limit is capped, not honoured"
        );
    }

    // ── parse_offset_cursor ──────────────────────────────────────────

    #[test]
    fn parse_offset_cursor_accepts_an_absent_null_or_digit_string_cursor() {
        assert_eq!(parse_offset_cursor(&json!({})), Some(0));
        assert_eq!(parse_offset_cursor(&json!({ "cursor": null })), Some(0));
        assert_eq!(parse_offset_cursor(&json!({ "cursor": "0" })), Some(0));
        assert_eq!(parse_offset_cursor(&json!({ "cursor": "40" })), Some(40));
    }

    /// The rejection side, INCLUDING the JSON-number case that used to
    /// collapse silently to page 1 (this fn's own doc). A rejected cursor is
    /// `None`, never `Some(0)` — restarting a traversal while looking like
    /// forward progress is how a paging loop turns into an infinite one.
    #[test]
    fn parse_offset_cursor_rejects_anything_that_is_not_a_non_negative_integer_string() {
        for payload in [
            json!({ "cursor": 100 }),
            json!({ "cursor": -1 }),
            json!({ "cursor": "-1" }),
            json!({ "cursor": "12.5" }),
            json!({ "cursor": "abc" }),
            json!({ "cursor": "" }),
            json!({ "cursor": true }),
            json!({ "cursor": ["40"] }),
            json!({ "cursor": { "offset": 40 } }),
        ] {
            assert_eq!(
                parse_offset_cursor(&payload),
                None,
                "must refuse rather than silently restart at 0: {payload}"
            );
        }
    }

    // ── trim_to_byte_budget ──────────────────────────────────────────

    /// The forward-progress guarantee: a single row larger than the WHOLE
    /// budget still survives, because a page of zero rows whose `nextCursor`
    /// never advanced would hang every traversal built on this forever. The
    /// second half pins that this is the only case where the budget is
    /// exceeded — rows past the first are still dropped.
    #[test]
    fn trim_to_byte_budget_keeps_at_least_one_row_and_drops_the_rest() {
        let huge = json!({ "text": "x".repeat(500) });
        let trimmed = trim_to_byte_budget(vec![huge.clone(), huge.clone(), huge], 0, 100);
        assert_eq!(
            trimmed.len(),
            1,
            "exactly one row survives an impossible budget"
        );

        // An empty input stays empty — "at least one" is never "invent one".
        assert!(trim_to_byte_budget(Vec::new(), 0, 100).is_empty());

        // Under budget: nothing is dropped.
        let small = vec![json!({ "id": 1 }), json!({ "id": 2 })];
        assert_eq!(trim_to_byte_budget(small.clone(), 0, 10_000), small);

        // `base_cost` really is subtracted from the same budget — the same
        // rows fit with no envelope cost and stop fitting with a large one.
        let rows: Vec<Value> = (0..20).map(|i| json!({ "id": i })).collect();
        let with_no_base = trim_to_byte_budget(rows.clone(), 0, 200).len();
        let with_big_base = trim_to_byte_budget(rows, 190, 200).len();
        assert!(
            with_big_base < with_no_base,
            "a larger base_cost must leave less room for rows ({with_big_base} !< {with_no_base})"
        );
    }
}
