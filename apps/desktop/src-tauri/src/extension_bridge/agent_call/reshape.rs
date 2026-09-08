//! Agent-layer payload reshaping — everything the generic tier does to a
//! payload that the renderer's own `commands/**` wire shape must not see, and
//! nothing else. Outbound: [`reshape_reply`] owns the ONE order the fence,
//! page and byte-encode steps run in; inbound:
//! [`unfence_named_fields_recursive`] is that fence's mirror, stripping a
//! wrapper a caller echoed back before any command body sees it. Both are
//! pure, and both are applied at the single `super::dispatch_direct`
//! chokepoint.
//!
//! R8 LOC-cap split (`docs/architecture-rules.md`), the same move `agent_read`
//! made for `found_jobs` and `agent_cli::mcp` for `instructions`: this is the
//! RESHAPING unit, so nothing about policy, refusal vocabulary, dispatch or
//! the frame-size ceiling travelled with it — those stay in `agent_call.rs`,
//! which names in one `use` the three items it calls. That naming runs ONE
//! way: this module reaches back with a blanket `use super::*` rather than an
//! item list, so nothing here records which parent items it leans on.
//! `agent_cli::mcp` and `agent_read` reach the few items they read through
//! this module's own path, the same shape
//! `agent_read::found_jobs` uses. The audited consts
//! here are in the SAME hand-audited style as `super::FENCE_FIELD_NAMES`, for
//! the same reason that const exists: the shared `commands/**` bodies are the
//! RENDERER's wire shape and must stay byte-for-byte identical, so anything
//! only an agent needs is done here, on the way out, and nowhere else.
//!
//! The fencing TABLES and the outbound fence walk itself stay in the parent:
//! they are a safety property of that chokepoint read from BOTH directions
//! (`super::fence_scraped_fields` on the way out, the mirror here on the way
//! in), and a child module sees its parent's private items, so nothing needed
//! widening for that half.

use serde_json::{json, Value};

use crate::extension_bridge::paging;

use super::*;

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
pub(super) const DEFAULT_LIST_PAGE_LIMIT: usize = 20;
pub(super) const MAX_LIST_PAGE_LIMIT: usize = 100;

/// The REAL per-response bound for a paged list reply. Same 150,000 B as
/// `agent_read::found_jobs::PAGE_BYTE_BUDGET` and for the same reason: this
/// payload rides inside the MCP server's `content[]`/`isError` wrapper under
/// its 256 KiB `MCP_RESULT_MAX_BYTES` cap, so half of that leaves real margin
/// for the wrapper. Measured AFTER [`fence_scraped_fields`] has run, on the
/// bytes actually about to go on the wire — fencing expands every scraped
/// field it touches, so a budget checked before it would be measuring a
/// payload that no longer exists by the time it ships.
pub(super) const LIST_PAGE_BYTE_BUDGET: usize = 150_000;

/// `(command, field)` pairs whose value is a RAW BYTE ARRAY that `serde_json`
/// renders as ~3.2–4× its own size in decimal digits and commas (issue
/// #1138: an ordinary one-page résumé exported to PDF came back at 259,841 B,
/// 99.1% of the MCP result cap, and a two-page one exceeded it — with no
/// `limit`/`cursor` to narrow and no other exposed export path). Re-encoded
/// base64 (~1.33×) HERE, never on the struct: `ExportResult.data` is the
/// RENDERER's own wire shape (`data: number[]`, consumed by the export
/// service hooks through `AppClient`), and `#[serde(with = …)]` on that field
/// would change it for them too.
///
/// Audited by hand against the struct each pair actually serializes from:
/// - `documents_export_document` → `export::types::ExportResult.data:
///   Vec<u8>` (camelCase-renamed struct; `data` is already its wire key).
///
/// `documents_render_preview_images` is the other payload the MCP cap's own
/// comment names, and it is deliberately NOT here: `PreviewResult.pages` is
/// `Vec<String>` of SVG source, already text, and base64ing it would make it
/// bigger and unreadable.
pub(super) const BASE64_BYTE_FIELDS: &[(&str, &str)] = &[("documents_export_document", "data")];

/// Suffix appended to a [`BASE64_BYTE_FIELDS`] field name to form the sibling
/// key that DECLARES the encoding (`data` → `dataEncoding`). Derived from the
/// field name rather than listed per pair so a second entry cannot forget it.
pub(super) const ENCODING_KEY_SUFFIX: &str = "Encoding";
pub(super) const BASE64_ENCODING: &str = "base64";

/// Re-encode every [`BASE64_BYTE_FIELDS`] array-of-bytes on `command`'s reply
/// as a base64 STRING, and add the sibling `<field>Encoding: "base64"` key
/// that says so. A payload that describes its own encoding survives a caller
/// that never read the server `instructions` or the tool description — the
/// reason this is a wire key and not documentation.
///
/// Top-level only, and by exact `(command, field)` pair — the opposite of
/// [`fence_named_fields_recursive`]'s unconditional recursive walk, on
/// purpose: fencing is a SAFETY property that must cover a field wherever it
/// appears, while this is a lossy-looking representation change that must
/// only ever hit the one field whose type was audited. A recursive
/// "any array of small integers is bytes" rule would eventually rewrite a
/// legitimate array of scores or ids into gibberish.
///
/// A non-array value (or a `data` that is not entirely bytes) is left exactly
/// as it was and gets NO marker key — the marker is only ever added on a
/// value this actually re-encoded, so the two can never disagree.
///
/// Visible to the whole `extension_bridge` tree for ONE reason: the test that
/// proves this actually solves #1138 has to compare against
/// `agent_cli::mcp::MCP_RESULT_MAX_BYTES`, the
/// cap it exists to get under, and that constant is private to the `mcp`
/// module — so the test lives THERE, beside the cap, rather than here beside
/// a hand-copied literal of it that could silently drift (same
/// cross-module-test reasoning as [`gate`]'s own `pub(super)`).
pub(in crate::extension_bridge) fn base64_byte_fields(command: &str, data: &mut Value) {
    use base64::Engine;
    let Some(map) = data.as_object_mut() else {
        return;
    };
    for (cmd, field) in BASE64_BYTE_FIELDS {
        if *cmd != command {
            continue;
        }
        let Some(Value::Array(items)) = map.get(*field) else {
            continue;
        };
        let bytes: Option<Vec<u8>> = items
            .iter()
            .map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
            .collect();
        let Some(bytes) = bytes else { continue };
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        map.insert((*field).to_string(), json!(encoded));
        map.insert(
            format!("{field}{ENCODING_KEY_SUFFIX}"),
            json!(BASE64_ENCODING),
        );
    }
}

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
pub(super) fn take_list_page_args(
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
pub(super) fn paginate_list_reply(data: Value, offset: usize, limit: usize) -> Value {
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

/// Reverses [`fence_named_fields_recursive`]'s wrapper on every INCOMING
/// `--input` value under a [`FENCE_FIELD_NAMES`] key, before ANY dispatched
/// command's real body ever sees it (security review round 4 — the
/// centralised fix: `commands::scrape::scrape_persist_job`'s own
/// `unfence_job_field` was a hand-added per-call-site strip, and every OTHER
/// freely-dispatchable WRITE command accepting one of these SAME field
/// names had none — three rounds of "add it at the call site" is what
/// produced that gap). A caller that reads a job through a fenced surface
/// (`scrape_list_postings`, `autopilot_list`, `ai_generations_list`, …) and
/// echoes a value straight back into a write would otherwise persist the
/// literal `<job_posting>…</job_posting>` markup into the user's own data —
/// this closes it for every CURRENT and FUTURE writer at the one chokepoint
/// every real dispatch already funnels through ([`dispatch_direct`], called
/// directly for `Read`/`Reversible` and at the tail of
/// [`dispatch_irreversible_confirmed`] for a confirmed `Irreversible`), not
/// one call site at a time. A no-op for the normal case — a clean value
/// that was never fenced — by [`crate::prompt_fence::strip_fence_wrapper`]'s
/// own contract (an exact prefix/suffix match, unchanged otherwise).
/// `commands::scrape::scrape_persist_job`'s own call-site strip is left in
/// place as defense-in-depth at the actual store-write boundary (that
/// command is also reachable from the renderer's normal `invoke()`, not
/// only through this dispatcher) rather than removed.
///
/// Mirrors [`fence_named_fields_recursive`]'s `ApplicationAnswer` shape rule
/// too (`answers_save` is a real writer of that exact shape), but NOT its
/// [`JOB_RECORD_ANCHOR_FIELDS`] exemption: nothing is written back into a
/// job's `result`, and a strip is a no-op on a value that was never fenced,
/// so the incoming walk stays deliberately unconditional.
pub(super) fn unfence_named_fields_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for field in FENCE_FIELD_NAMES {
                if let Some(s) = map.get(*field).and_then(Value::as_str) {
                    let stripped = crate::prompt_fence::strip_fence_wrapper("job_posting", s);
                    map.insert((*field).to_string(), json!(stripped));
                    continue;
                }
                if let Some(Value::Array(items)) = map.get_mut(*field) {
                    for item in items.iter_mut() {
                        if let Value::String(s) = item {
                            *s = crate::prompt_fence::strip_fence_wrapper("job_posting", s);
                        }
                    }
                }
            }
            // The mirror of [`fence_named_fields_recursive`]'s shape guard:
            // an `ApplicationAnswer`'s `question` goes out fenced, so a
            // caller echoing that record back into a write (`answers_save`)
            // must not persist the markup. Same predicate, same field — see
            // [`APPLICATION_ANSWER_ANCHOR_FIELDS`].
            if is_application_answer_shaped(map) {
                if let Some(question) = map
                    .get(APPLICATION_ANSWER_QUESTION_FIELD)
                    .and_then(Value::as_str)
                {
                    let stripped =
                        crate::prompt_fence::strip_fence_wrapper("job_posting", question);
                    map.insert(
                        APPLICATION_ANSWER_QUESTION_FIELD.to_string(),
                        json!(stripped),
                    );
                }
            }
            for v in map.values_mut() {
                unfence_named_fields_recursive(v);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                unfence_named_fields_recursive(item);
            }
        }
        _ => {}
    }
}

/// Commands whose reply is a BARE JSON string carrying the same untrusted
/// document text [`super::FENCE_FIELD_NAMES`]'s `"text"` entry fences when it
/// arrives wrapped in an object (`documents_list`'s rows). `documents_get_text`
/// declares `-> AppResult<String>`, so its reply is a scalar at the ROOT of
/// the tree — [`fence_named_fields_recursive`]'s name-keyed walk only fences a
/// STRING VALUE reached under a named key, so a root with no key at all falls
/// through its `_ => {}` arm untouched (issue #1170's follow-up,
/// `B1-r1-ACLI-R5-7`). `system_get_version` also returns a bare string, but
/// that value is this app's OWN version, never user-authored text, so this
/// list is scoped to the one command whose scalar reply is untrusted.
const SCALAR_FENCE_COMMANDS: &[&str] = &["documents_get_text"];

/// Fences `data` in place when `command` is on [`SCALAR_FENCE_COMMANDS`] and
/// the reply is actually a bare string — a rename to an object reply (nothing
/// requires `AppResult<String>` to stay that shape) simply stops matching
/// here rather than double-fencing, since [`fence_scraped_fields`]'s
/// name-keyed walk would then cover it instead.
fn fence_scalar_reply(command: &str, data: &mut Value) {
    if let Value::String(s) = data {
        if SCALAR_FENCE_COMMANDS.contains(&command) {
            *data = json!(crate::prompt_fence::fenced(
                "job_posting",
                s,
                crate::prompt_fence::JOB_CAP
            ));
        }
    }
}

/// Every reshape a dispatched reply gets before it goes on the wire, in the
/// ONE order they are allowed to run in. Pure — no `AppHandle`, no I/O — so
/// the ordering itself is testable, which is the reason it is a fn at all
/// (as three statements inline, nothing failed when they were reordered).
///
/// 1. **Fence first.** [`fence_scraped_fields`] (plus [`fence_scalar_reply`]
///    for the one bare-string reply it structurally cannot reach) is the
///    security property and is unconditional over the WHOLE reply; narrowing
///    it to "only the rows we are about to return" would make its coverage
///    depend on a paging decision.
/// 2. **Then page.** [`paginate_list_reply`]'s byte budget must measure the
///    FENCED bytes that will really ship: fencing rewrites every field it
///    touches (`crate::prompt_fence::JOB_CAP` truncates a long one, the
///    wrapper adds to a short one), so a budget applied first would be
///    measuring a payload that no longer exists by the time it ships. This is
///    the step the test mutation-checks: reorder 1 and 2 and the page's row
///    count changes.
/// 3. **Then base64.** [`base64_byte_fields`] must see the raw `Vec<u8>`
///    array rather than something a later step rewrote, and it writes a
///    TOP-LEVEL key — after paging, "top level" means the paged envelope. No
///    command is in both [`PAGINATED_LIST_COMMANDS`] and
///    [`BASE64_BYTE_FIELDS`] today, so no payload can currently observe 2-vs-3
///    ordering; that disjointness is itself asserted in the tests, so the day
///    it stops holding, the guard fires instead of the ordering silently
///    starting to matter unnoticed.
pub(super) fn reshape_reply(
    command: &str,
    mut data: Value,
    page_args: Option<(usize, usize)>,
) -> Value {
    fence_scalar_reply(command, &mut data);
    fence_scraped_fields(&mut data);
    if let Some((offset, limit)) = page_args {
        data = paginate_list_reply(data, offset, limit);
    }
    base64_byte_fields(command, &mut data);
    data
}
