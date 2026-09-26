//! Agent-layer list paging — the `{items,total,nextCursor}` envelope `PAGINATED_LIST_COMMANDS`
//! rows get, and the `limit`/`cursor` args this layer takes off `input` before dispatch.

use serde_json::{json, Value};

use crate::extension_bridge::paging;

use super::super::Refusal;

/// Commands whose reply is an unbounded, growth-only, already-newest-first
/// ARRAY that NO command argument can narrow — both take `(app: AppHandle)`
/// and nothing else, and both back a `SELECT … ORDER BY … DESC` with no
/// `LIMIT` (issue #1136: 1.7 MB of `ai_generations` and 440 KB of
/// `applications` on an ordinary account, i.e. permanently over the MCP
/// server's 256 KiB result cap with no parameter a caller could add to
/// succeed). Paged HERE rather than in `commands/**`, whose wire shape the
/// renderer's own service hooks depend on — the shared command bodies are
/// deliberately untouched by this fix.
///
/// Audited by hand, one row at a time, against the command's real signature
/// and its real query (mirrors `policy`'s own per-row audit discipline):
/// - `applications_list` → `applications::ApplicationStore::list`
/// - `ai_generations_list` → `ai_generations::AiGenerationStore::list`
/// - `documents_list` → `documents::DocumentStore::list` (round-3 fix,
///   `B1-r3-ACLI-5`: it takes no argument and returns every document's FULL
///   `text`, so a caller pointed at it by `INSTRUCTIONS`/the `profile` tool
///   description had no way to narrow a `result_too_large` refusal — this
///   is that narrowing path)
///
/// Adding a row here CHANGES that command's reply shape for every generic-tier
/// caller (a bare array becomes [`paginate_list_reply`]'s envelope), so it is
/// an audited list and not a heuristic like "any command whose reply is a big
/// array" — a shape-sniffing rule would silently reshape a future command
/// whose array is bounded by construction, and reshape it differently as the
/// user's data grew. Visible to the whole `extension_bridge` tree —
/// `agent_cli::mcp`'s `commands` tool marks these rows so a caller can
/// DISCOVER the paging instead of inferring it from a surprise envelope,
/// never a second hand-typed name list.
pub(in crate::extension_bridge) const PAGINATED_LIST_COMMANDS: &[&str] =
    &["applications_list", "ai_generations_list", "documents_list"];

/// What the `commands` tool prints on a [`PAGINATED_LIST_COMMANDS`] row.
/// Lives HERE, next to the behaviour it describes, so the description cannot
/// drift from the list it describes (`agent-cli-standards`: nothing
/// hand-maintained that can drift). Deliberately names no default/cap number
/// — those live on [`DEFAULT_LIST_PAGE_LIMIT`]/[`MAX_LIST_PAGE_LIMIT`] and a
/// copy here would be a second source of truth for them.
///
/// The pacing numbers ARE spelled out, unlike those two, because this text is
/// the only thing the consumer (an LLM that cannot read this source) ever
/// sees, and "burst, then refill" without figures is unactionable. They are a
/// COPY of `agent_read::AGENT_CHEAP_BURST`/`AGENT_CHEAP_REFILL_SECS`, which
/// remain the source of truth. Private there, so this copy cannot be derived
/// here without widening them; instead `agent_read`'s own test module (which
/// sees both) asserts this string still contains the `format!`-derived
/// "burst {N}" and "every {N} s" — so the drift fails a test rather than
/// shipping a wrong number to the one reader who cannot check it.
///
/// The last sentence states the offset cursor's known weakness rather than
/// leaving a caller to discover it: this is the same accepted trade-off
/// `agent_read::found_jobs::resolve_found_jobs` documents at length for
/// `found-jobs`, and a caller that is told can act on it.
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

/// Server-side default/cap for a [`PAGINATED_LIST_COMMANDS`] `limit` — a
/// CEILING on rows per page, never the transport-size guarantee. That
/// guarantee is [`LIST_PAGE_BYTE_BUDGET`], enforced by
/// `paging::trim_to_byte_budget` against the REAL serialized bytes, because a
/// row count cannot bound bytes here either: one `AiGenerationRecord` carries
/// a full résumé, a cover letter AND the whole scraped job ad, while an
/// `Application` row is a fraction of that, so no single count can be right
/// for both. The count is only the cheap first cut — for `ai_generations` the
/// budget will usually cut a page well below the default, and for the much
/// smaller `applications` rows the default is comfortably inside it.
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

/// Take the agent-layer paging arguments OFF `input` for a
/// [`PAGINATED_LIST_COMMANDS`] target, returning `(offset, limit)`; `None`
/// for every other command, whose `input` is left untouched.
///
/// `limit`/`cursor` are REMOVED rather than read in place because they belong
/// to this layer, not to the command: neither of these two commands declares
/// any argument at all, and a future one that declared its own `limit` must
/// not receive the paging layer's copy of it. Clamping vs refusing follows the
/// same RULE `found-jobs` does, on the shared `extension_bridge::paging`
/// primitives — the clamp and the byte budget are literally shared, while the
/// cursor GRAMMAR stays each surface's own to evolve (`found-jobs` may scope
/// its cursor to its issuing autopilot without this tier changing at all).
/// The rule: a junk `limit` clamps to the default (never to "unbounded"),
/// while a junk `cursor` REFUSES — silently resetting a cursor to 0 looks
/// like forward progress while actually restarting the traversal, which is how
/// a paging loop turns into an infinite one.
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

/// Slice an already-fenced array reply into one page and wrap it in the
/// `{items,total,nextCursor}` envelope. Pure — the whole of #1136's logic is
/// unit-testable without an `AppHandle`, the same pure/impure split
/// [`classify_response`] and `confirm_and_run` use.
///
/// Runs AFTER [`fence_scraped_fields`], which is left unconditional over the
/// WHOLE reply: fencing is the security property, and narrowing what it walks
/// to "only the rows we are about to return" would make its coverage depend
/// on a paging decision. The cost is unchanged from before this fix — that
/// full-array walk already happened on every one of these calls.
///
/// A non-array reply is returned verbatim (no envelope), so a command that
/// ever stopped returning an array degrades to today's behaviour instead of
/// producing `{items: <not an array>}`.
///
/// `total` is the FULL row count, not the page's — it is what tells a caller
/// the traversal is still moving, and it is the value an `Effect::Irreversible`
/// row whose `ProofSource::Count` reads this list is really after. The proof
/// ceremony itself is unaffected either way: `proof::resolve` dispatches
/// through [`invoke_command`] directly, never through [`dispatch_direct`], so
/// it still resolves against the complete, unpaged array.
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
