//! Agent-layer list paging — the `{items,total,nextCursor}` envelope `PAGINATED_LIST_COMMANDS`
//! rows get, and the `limit`/`cursor` args this layer takes off `input` before dispatch.

use serde_json::{json, Value};

use crate::extension_bridge::paging;

use super::super::Refusal;

/// Commands whose reply is an unbounded, growth-only, already-newest-first ARRAY that NO command
/// argument can narrow — both take `(app: AppHandle)` and nothing else, backing a
/// `SELECT … ORDER BY … DESC` with no `LIMIT` (issue #1136: 1.7 MB of `ai_generations` and 440 KB
/// of `applications` on an ordinary account, permanently over the MCP server's 256 KiB result cap
/// with no parameter a caller could add to succeed). Paged HERE rather than in `commands/**`,
/// whose wire shape the renderer's service hooks depend on.
///
/// Audited by hand against each command's real signature and query: `applications_list` →
/// `ApplicationStore::list`, `ai_generations_list` → `AiGenerationStore::list`, `documents_list` →
/// `DocumentStore::list` (round-3 fix: it returns every document's FULL `text` with no argument to
/// narrow a `result_too_large` refusal — this is that narrowing path).
///
/// Adding a row here CHANGES that command's reply shape (a bare array becomes
/// [`paginate_list_reply`]'s envelope), so it is an audited list, never a shape-sniffing heuristic
/// that could silently reshape a future bounded-by-construction array. Visible to the whole
/// `extension_bridge` tree — `agent_cli::mcp`'s `commands` tool marks these rows so a caller can
/// discover the paging instead of inferring it from a surprise envelope.
pub(in crate::extension_bridge) const PAGINATED_LIST_COMMANDS: &[&str] =
    &["applications_list", "ai_generations_list", "documents_list"];

/// What the `commands` tool prints on a [`PAGINATED_LIST_COMMANDS`] row. Lives HERE, next to the
/// behaviour it describes, so it cannot drift from the list it describes. Names no default/cap
/// number — those live on [`DEFAULT_LIST_PAGE_LIMIT`]/[`MAX_LIST_PAGE_LIMIT`], a copy here would be
/// a second source of truth.
///
/// The pacing numbers ARE spelled out, since this text is the only thing an LLM consumer ever
/// sees and "burst, then refill" without figures is unactionable — a COPY of
/// `agent_read::AGENT_CHEAP_BURST`/`AGENT_CHEAP_REFILL_SECS` (private there, so `agent_read`'s own
/// test asserts this string still contains their `format!`-derived values, failing a test rather
/// than shipping a wrong number to the one reader who cannot check it).
///
/// The last sentence states the offset cursor's known weakness up front, same accepted trade-off
/// `found_jobs::resolve_found_jobs` documents at length.
pub(in crate::extension_bridge) const PAGINATED_LIST_NOTE: &str =
    "returns a paged envelope {items,total,nextCursor} instead of a bare array (the raw list is \
     unbounded and exceeds the result cap). Pass input.limit (clamped server-side) and \
     input.cursor (a prior reply's nextCursor, verbatim; omit for the first page); repeat until \
     nextCursor is null. `total` is the FULL row count, unaffected by paging — it is what an \
     Effect::Irreversible row whose proof is this list's length wants. Pages draw on the \
     shared cheap throttle bucket nearly every agent call uses (burst 10, one token back every \
     1 s), so pace a traversal at roughly one page per second instead of looping as fast as \
     replies arrive; a rate_limited refusal means wait, not retry at once. The cursor is a \
     plain offset into the list as it stands right now, so a row added or removed between two \
     pages can make that boundary repeat or skip a row (accepted, same as the found-jobs \
     resource) — a `total` that changed between calls is the signal, and restarting with no \
     cursor is the fix if that matters.";

/// Server-side default/cap for a [`PAGINATED_LIST_COMMANDS`] `limit` — a CEILING on rows per page,
/// never the transport-size guarantee ([`LIST_PAGE_BYTE_BUDGET`], enforced against the REAL
/// serialized bytes): one `AiGenerationRecord` carries a full résumé+cover letter+job ad while an
/// `Application` row is a fraction of that, so no row count alone can bound bytes for both.
pub(in crate::extension_bridge::agent_call) const DEFAULT_LIST_PAGE_LIMIT: usize = 20;
pub(in crate::extension_bridge::agent_call) const MAX_LIST_PAGE_LIMIT: usize = 100;

/// The REAL per-response bound for a paged list reply. Same 150,000 B as
/// `agent_read::found_jobs::PAGE_BYTE_BUDGET` and for the same reason: this
/// payload rides inside the MCP server's `content[]`/`isError` wrapper under
/// its 256 KiB `MCP_RESULT_MAX_BYTES` cap, so half of that leaves real margin
/// for the wrapper. Measured AFTER [`fence_scraped_fields`] has run, on the
/// bytes actually about to go on the wire — fencing expands every scraped
/// field it touches, so a budget checked before it would be measuring a
/// payload that no longer exists by the time it ships.
pub(in crate::extension_bridge::agent_call) const LIST_PAGE_BYTE_BUDGET: usize = 150_000;

/// Take the agent-layer paging arguments OFF `input` for a [`PAGINATED_LIST_COMMANDS`] target,
/// returning `(offset, limit)`; `None` for every other command, whose `input` is left untouched.
///
/// `limit`/`cursor` are REMOVED rather than read in place — they belong to this layer, not the
/// command, and a future one declaring its own `limit` must not receive this layer's copy. Clamp
/// vs refuse follows `found-jobs`' same rule on the shared `extension_bridge::paging` primitives: a
/// junk `limit` clamps to the default, while a junk `cursor` REFUSES — silently resetting to 0
/// looks like forward progress while actually restarting the traversal, an infinite paging loop.
pub(in crate::extension_bridge::agent_call) fn take_list_page_args(
    command: &str,
    input: &mut Value,
) -> Result<Option<(usize, usize)>, Refusal> {
    if !PAGINATED_LIST_COMMANDS.contains(&command) {
        return Ok(None);
    }
    let offset = paging::parse_offset_cursor(input).ok_or(Refusal::InvalidCursor)?;
    let limit = paging::clamp_limit(input, DEFAULT_LIST_PAGE_LIMIT, MAX_LIST_PAGE_LIMIT);
    if let Some(map) = input.as_object_mut() {
        map.remove("limit");
        map.remove("cursor");
    }
    Ok(Some((offset, limit)))
}

/// Slice an already-fenced array reply into one page and wrap it in the `{items,total,nextCursor}`
/// envelope. Pure — the whole of #1136's logic is unit-testable without an `AppHandle`.
///
/// Runs AFTER [`fence_scraped_fields`], left unconditional over the WHOLE reply on purpose:
/// narrowing its walk to "only the rows about to return" would make coverage depend on a paging
/// decision, and that full-array walk already happened on every one of these calls regardless.
///
/// A non-array reply is returned verbatim (no envelope). `total` is the FULL row count, not the
/// page's — what tells a caller the traversal is still moving, and what an `Irreversible` row's
/// `ProofSource::Count` is really after; `proof::resolve` dispatches directly, never through
/// [`dispatch_direct`], so it still resolves against the complete, unpaged array either way.
pub(in crate::extension_bridge::agent_call) fn paginate_list_reply(
    data: Value,
    offset: usize,
    limit: usize,
) -> Value {
    let Value::Array(rows) = data else {
        return data;
    };
    let total = rows.len();
    let candidates: Vec<Value> = rows.into_iter().skip(offset).take(limit).collect();

    // `base_cost` = every envelope byte OTHER than the `items` array itself,
    // measured rather than assumed (same derivation as
    // `agent_read::found_jobs::resolve_found_jobs`'s own call site).
    // `nextCursor` isn't known yet — it depends on how many rows survive the
    // trim just below — so it is measured as a digit string the length of
    // `total`, an upper bound (a real offset can never exceed `total`), which
    // can only over-count and so only ever trim MORE than strictly required.
    let base_envelope = json!({ "items": [], "total": total, "nextCursor": total.to_string() });
    let base_cost = serde_json::to_string(&base_envelope)
        .map_or(usize::MAX, |s| s.len())
        .saturating_sub(2); // the placeholder `[]`'s own two bytes

    let page = paging::trim_to_byte_budget(candidates, base_cost, LIST_PAGE_BYTE_BUDGET);
    let next_offset = offset + page.len();
    let next_cursor = (next_offset < total).then(|| next_offset.to_string());
    json!({ "items": page, "total": total, "nextCursor": next_cursor })
}
