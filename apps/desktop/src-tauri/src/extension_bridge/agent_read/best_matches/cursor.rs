//! `best-matches`' limit clamp and cursor issuing/parsing — split out of `best_matches.rs` under
//! the R8 LOC cap.

use serde_json::Value;

use crate::error::AppResult;

/// Server-side default/cap for `best-matches`' `limit` — applied BEFORE
/// serialization (never trust an unbounded client-supplied number), well
/// under `MAX_FRAME_BYTES` even at the max.
///
/// `pub(in crate::extension_bridge)` — same reason `found_jobs`'
/// `DEFAULT_FOUND_JOBS_LIMIT`/`MAX_FOUND_JOBS_LIMIT` pair carries the identical
/// visibility: `agent_cli::mcp` (a sibling of `agent_read`, not a descendant) derives the
/// `best-matches` tool schema's advertised default/cap from THESE numbers, reached via the fully
/// qualified `agent_read::best_matches::{DEFAULT,MAX}_BEST_MATCHES_LIMIT`, rather than a
/// hand-typed copy that can silently drift out of sync.
///
/// `MAX_BEST_MATCHES_LIMIT` equals
/// `commands::autopilot::best_matches::BEST_MATCHES_CAP` (round 3 fix,
/// B3-r3-F2 — it used to be half that cap, so a full traversal took 2–5
/// calls, each one re-running the command's own real clustering pass with
/// no cache; the 30s-refill throttle bucket sized for exactly one call per
/// traversal turned that into 30–120s of forced stalls). Equal to the cap
/// means one max-limit page always reaches the whole reachable set in a
/// SINGLE call — see
/// `agent_read::tests::max_best_matches_limit_covers_the_full_capped_row_set_in_one_page`.
pub(in crate::extension_bridge) const DEFAULT_BEST_MATCHES_LIMIT: usize = 20;
pub(in crate::extension_bridge) const MAX_BEST_MATCHES_LIMIT: usize = 100;

/// Issue #1167/#1146 P11 — reuses `extension_bridge::paging::clamp_limit`, the
/// same shared primitive `found_jobs` uses, rather than a hand-rolled copy: the
/// hand-rolled version this replaced let `limit: 0` through as `0` instead of
/// falling back to [`DEFAULT_BEST_MATCHES_LIMIT`] (`Value::as_u64` reads `0` as
/// `Some(0)`, so `.unwrap_or` never fired) — a zero-row page whose `nextCursor`
/// never advances, hanging any paging loop built on it forever.
pub(in crate::extension_bridge::agent_read) fn clamp_best_matches_limit(payload: &Value) -> usize {
    crate::extension_bridge::paging::clamp_limit(
        payload,
        DEFAULT_BEST_MATCHES_LIMIT,
        MAX_BEST_MATCHES_LIMIT,
    )
}

/// A `cursor` that isn't a nextCursor SHAPE at all — mirrors
/// `found_jobs::MALFORMED_CURSOR_MESSAGE`'s own wording for the identical
/// case, one hop over.
pub(in crate::extension_bridge::agent_read) const BEST_MATCHES_MALFORMED_CURSOR_MESSAGE: &str =
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
pub(in crate::extension_bridge::agent_read) const BEST_MATCHES_WRONG_QUERY_CURSOR_MESSAGE: &str =
    "cursor was issued for a different `query` — page that same query with it, or restart from \
     `cursor: null`";

/// Fold `query`'s already-normalized (lowercased/trimmed) value into the
/// cursor's issuer half — mirrors `found_jobs::found_jobs_cursor_issuer`'s
/// identical reasoning one resource over. [`crate::extension_bridge::paging::fingerprint`]
/// rather than the raw query text: `query` is caller-typed and could itself
/// contain `:`, and a fingerprint sidesteps needing to prove it never
/// collides with the issuer's own delimiter.
pub(in crate::extension_bridge::agent_read) fn best_matches_cursor_issuer(
    query: Option<&str>,
) -> String {
    crate::extension_bridge::paging::fingerprint(&[query.unwrap_or("")])
}

/// Parse `payload`'s `cursor` against `issuer` (see
/// [`best_matches_cursor_issuer`]) — mirrors
/// `found_jobs::parse_found_jobs_cursor`'s own shape-then-issuer contract
/// and never-echo discipline, one resource over (round 2 fix, B3-r1-F4:
/// `best-matches` used to accept a bare numeric offset via
/// `extension_bridge::paging::parse_offset_cursor`, which carried no
/// evidence of which `query` produced it).
pub(in crate::extension_bridge::agent_read) fn parse_best_matches_cursor(
    payload: &Value,
    issuer: &str,
) -> AppResult<usize> {
    use crate::error::AppError;
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

/// The payload-only half of `best-matches`' argument parsing — `query`
/// (round 2 fix, B3-r2-F1 — MUST go through `found_jobs::trimmed_lowercase_filter`,
/// never a raw `.and_then(Value::as_str)`, which silently read a non-string
/// or present-but-blank `query` as absent and handed back the unfiltered
/// ranked list with a `total` the caller read as filtered) plus the cursor
/// offset it feeds. No `AppHandle` needed — unlike [`super::best_matches_resource`]
/// itself, which only adds the `commands::autopilot::autopilot_best_matches`
/// call this can't reach — so THIS delegation is directly unit-testable
/// (round 3 fix, B3-r3-F7: the previous guard tested
/// `found_jobs::trimmed_lowercase_filter` directly, which pinned nothing
/// about `best_matches_resource` actually calling it — reverting the call
/// site back to the old combinator left that guard green).
pub(in crate::extension_bridge::agent_read) fn parse_best_matches_args(
    payload: &Value,
) -> AppResult<(Option<String>, usize)> {
    let query = super::super::found_jobs::trimmed_lowercase_filter(payload, "query")?;
    let cursor_issuer = best_matches_cursor_issuer(query.as_deref());
    let offset = parse_best_matches_cursor(payload, &cursor_issuer)?;
    Ok((query, offset))
}
