//! `resources/list`, `resources/templates/list` and `resources/read` — issue #1146 P4's MCP
//! resources mirroring the `profile`/`best-matches`/`job` tools byte-for-byte: each one builds
//! the SAME [`Verb`] its identically-named tool builds and is dispatched through the SAME
//! [`dispatch_payload`]/[`results::capped_result_text`] pair `tools/call` uses, so a resource can
//! never diverge from that tool's read path, projection or fencing — a test in `mcp::tests` pins
//! the two outputs byte-identical for the same input.
//!
//! R8 LOC-cap split (`docs/architecture-rules.md`), the same move `mcp/schemas.rs` already made:
//! this is the RESOURCE catalogue-and-classification unit, so the protocol loop, its queue and
//! its worker thread stay in `mcp.rs`, which reads back [`resources_list`],
//! [`resource_templates`], [`classify_resource_read`] and [`dispatched_resource_result`]/
//! [`resource_result`] through this module's own path.

use super::*;

pub(super) const URI_PROFILE: &str = "ajh://profile";
pub(super) const URI_BEST_MATCHES: &str = "ajh://best-matches";
const URI_JOB_PREFIX: &str = "ajh://job/";
const URI_JOB_TEMPLATE: &str = "ajh://job/{url}";

/// The two STATIC resources — `ajh://job/{url}` is a TEMPLATE (a real URI needs its `{url}`
/// filled in), so it is advertised separately via [`resource_templates`]
/// (`resources/templates/list`), the same split the MCP spec itself draws between concrete and
/// parameterized resources.
pub(super) fn resources_list() -> Vec<Value> {
    vec![
        json!({
            "uri": URI_PROFILE,
            "name": TOOL_PROFILE,
            "title": "My Profile",
            "description": "Same contact-field projection the `profile` tool returns.",
            "mimeType": "application/json",
        }),
        json!({
            "uri": URI_BEST_MATCHES,
            "name": TOOL_BEST_MATCHES,
            "title": "Best Matches",
            "description": "Same ranked, fenced first page the `best-matches` tool returns for \
                its default limit, with no cursor or query.",
            "mimeType": "application/json",
        }),
    ]
}

pub(super) fn resource_templates() -> Vec<Value> {
    vec![json!({
        "uriTemplate": URI_JOB_TEMPLATE,
        "name": TOOL_JOB,
        "title": "Job by URL",
        "description": "Same fenced posting lookup the `job` tool returns — {url} is the \
            posting's URL, percent-encoded.",
        "mimeType": "application/json",
    })]
}

/// `ajh://profile` / `ajh://best-matches` / `ajh://job/<percent-encoded url>` → the SAME [`Verb`]
/// the identically-named tool's own [`tool_argv`] builds for an empty/default `arguments` object
/// — never a second, hand-typed construction. `None` for anything else, including a job URI whose
/// tail fails to percent-decode OR decodes to an empty/whitespace-only string (T7, PR #1184
/// CodeRabbit review: `ajh://job/` — no tail at all — previously built `Verb::Job { url: "" }`
/// and paid a bridge round trip for a URL no `job` tool call would ever accept): none of these
/// name a real resource, so all answer the identical `resources/read` refusal this fn's caller
/// builds, before any bridge call.
fn resource_verb(uri: &str) -> Option<Verb> {
    match uri {
        URI_PROFILE => Some(Verb::Profile),
        URI_BEST_MATCHES => Some(Verb::BestMatches {
            limit: None,
            cursor: None,
            query: None,
        }),
        _ => {
            let encoded = uri.strip_prefix(URI_JOB_PREFIX)?;
            let url = urlencoding::decode(encoded).ok()?.into_owned();
            if url.trim().is_empty() {
                return None;
            }
            Some(Verb::Job { url })
        }
    }
}

/// What a `resources/read` request turns out to be, once classified — the resource mirror of
/// [`ToolCall`]. Only [`ResourceCall::Bridge`] costs a round trip; the `uri` travels with it so
/// the eventual reply can echo the exact string the client asked for.
pub(super) enum ResourceCall {
    Local(Result<Value, (i64, &'static str)>),
    Bridge(String, Verb),
}

/// `resources/read` on a `uri` this server does not recognize — MCP's own named error for this
/// case: the specification's `resources/read` example carries exactly this code
/// (`-32002`, `"Resource not found"`) for an unrecognized `uri`, distinct from the generic
/// `-32602` a malformed ARGUMENT SHAPE gets everywhere else on this server. The `uri` itself is
/// never echoed back into the error text — it is caller-supplied and this fixed message is
/// enough to act on.
const ERR_RESOURCE_NOT_FOUND: (i64, &str) = (-32002, "Resource not found");

/// Classifies one `resources/read` request — pure, like [`classify_tool_call`]: a missing/
/// non-string `uri` or one that matches no known resource is answered locally; only a real one
/// costs the bridge round trip.
pub(super) fn classify_resource_read(params: &Value) -> ResourceCall {
    let Some(uri) = params.get("uri").and_then(Value::as_str) else {
        return ResourceCall::Local(Err((-32602, "Invalid params")));
    };
    match resource_verb(uri) {
        Some(verb) => ResourceCall::Bridge(uri.to_string(), verb),
        None => ResourceCall::Local(Err(ERR_RESOURCE_NOT_FOUND)),
    }
}

/// One `resources/read` result: a single `contents[0]` entry naming its own `uri`, capped by the
/// SAME [`results::MCP_RESULT_MAX_BYTES`] [`results::tool_result`] enforces via
/// [`results::capped_result_text`] — a resource travels the identical worker queue as a
/// `tools/call` (its busy/shutting-down refusals included, see `mcp.rs`'s `PendingKind`), so it
/// must be bounded the same way rather than growing a second, unbounded egress path for the exact
/// data a tool call would have refused.
///
/// Returns `Err` when the cap fires (T8, PR #1184 CodeRabbit review): `tools/call` has an
/// `isError` field to mark `tool_result`'s own capped reply as a failure, but `resources/read`
/// has no such field — wrapping the `result_too_large` refusal in `contents[0]` unchanged would
/// have shipped a `200`/successful JSON-RPC result whose body happens to be a refusal, which a
/// client has no contractual way to distinguish from real posting/profile/match data. A genuine
/// JSON-RPC error is the only shape that tells the client this reply is not the data it asked
/// for. Every normal (uncapped) payload is unaffected — `Ok` with the exact same envelope as
/// before.
pub(super) fn resource_result(uri: &str, payload: Value) -> Result<Value, (i64, &'static str)> {
    let (text, _payload, code) = results::capped_result_text(payload, 0);
    if code != 0 {
        // `capped_result_text` only ever returns a non-zero code (always `2`) when it substituted
        // the oversized-result refusal for the payload it was given — the `0` passed in above is
        // otherwise returned unchanged.
        return Err((-32603, agent_call::ERR_RESULT_TOO_LARGE));
    }
    Ok(json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": text }] }))
}

/// The bridge-backed TAIL of a `resources/read` — the resource mirror of
/// [`dispatched_tool_result`], sharing [`results::dispatch_payload`] so the SAME payload (success
/// or the synthesized `ok:false` sentinel wrapper) backs both; only the envelope around it
/// differs.
pub(super) fn dispatched_resource_result(
    uri: &str,
    verb: &Verb,
    dispatch: &mut dyn FnMut(&Verb) -> Result<Value, &'static str>,
) -> Result<Value, (i64, &'static str)> {
    let (payload, _code) = results::dispatch_payload(verb, dispatch);
    resource_result(uri, payload)
}
