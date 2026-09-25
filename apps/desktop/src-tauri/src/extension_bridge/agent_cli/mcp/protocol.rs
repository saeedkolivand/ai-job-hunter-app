//! The JSON-RPC protocol layer both transports share: the version handshake, the launched
//! server's fixed state, reply framing, and the classifier that decides whether one parsed
//! message is dropped, answered locally, or queued for the bridge.

use super::*;

// ── Version negotiation (Claude Code's own hard list; never the 2026-07-28 era) ────────────────

pub(super) const SUPPORTED_VERSIONS: &[&str] = &[
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];
pub(super) const DEFAULT_VERSION: &str = "2025-11-25";

pub(super) fn initialize_result(params: &Value, instructions: &str) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let version = requested
        .filter(|v| SUPPORTED_VERSIONS.contains(v))
        .unwrap_or(DEFAULT_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {}, "resources": {}, "prompts": {} },
        "serverInfo": { "name": "ai-job-hunter", "version": env!("CARGO_PKG_VERSION") },
        "instructions": instructions,
    })
}

// ── The JSON-RPC loop ───────────────────────────────────────────────────

/// One launched server's fixed state: its `tools/list` answer, its (tier-dependent) `initialize`
/// instructions, and the [`Tier`] `commands`/`tool_call_result` both need. Built once in [`run`]
/// (or by a test) and threaded through the read loop instead of a growing positional parameter
/// list.
pub(super) struct Server {
    pub(super) tools: Vec<Value>,
    pub(super) instructions: String,
    pub(super) tier: Tier,
}

impl Server {
    /// Still takes the raw launch-flag pair (matches [`run`]'s own [`LaunchArgs`]) but resolves it
    /// to a [`Tier`] via [`Tier::from_flags`] exactly once, here — `tools`/`commands_value`/
    /// `build_instructions` never see the raw pair at all.
    pub(super) fn new(allow_reversible: bool, allow_irreversible: bool) -> Self {
        let tier = Tier::from_flags(allow_reversible, allow_irreversible);
        Self {
            tools: tools(tier),
            instructions: build_instructions(tier),
            tier,
        }
    }
}

pub(super) fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// One JSON-RPC reply frame around an already-computed outcome.
pub(super) fn reply_frame(id: Value, outcome: Result<Value, (i64, &'static str)>) -> Value {
    match outcome {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => rpc_error(id, code, message),
    }
}

/// What the main thread does with one input line. [`Routed::Drop`] means "no reply, ever" — a
/// notification (no `id` member), an explicit `id: null`, or any `notifications/*` method
/// regardless of `id`; a `tools/call` in that state never becomes a [`Routed::Call`] and so never
/// reaches the worker at all: nothing is listening for the result.
pub(super) enum Routed {
    Drop,
    /// Answerable without touching the bridge — written immediately, even mid-call. Every
    /// protocol method AND every [`ToolCall::Local`] outcome lands here.
    Reply(Value),
    /// A bridge-backed call, already classified and parsed: the ONLY thing that queues behind an
    /// earlier one (see the module doc). `kind` decides the reply SHAPE once dispatched — a
    /// `tools/call` and a `resources/read` share this one queue and worker (issue #1146 P4), so
    /// the busy/shutting-down refusals below need it too, not just the happy path.
    Call {
        id: Value,
        verb: Verb,
        kind: PendingKind,
    },
}

/// Which reply shape a queued bridge call is owed once dispatched, decided at classification
/// time — `tools/call` becomes a `CallToolResult` ([`dispatched_tool_result`]), `resources/read`
/// becomes a `contents` envelope naming its own `uri` ([`resources::dispatched_resource_result`]).
/// Threaded through the dispatch queue AND `in_flight` so the busy ([`TrySendError::Full`]) and
/// shutting-down (EOF drain) refusals answer in the SAME shape a successful dispatch would have,
/// never a tool-shaped refusal for a resource read or vice versa. [`handle_message`] (the HTTP
/// transport's shared per-request handler) switches on the SAME `kind` to build its own reply,
/// so a resource read answers identically over either wire.
#[derive(Debug, Clone)]
pub(super) enum PendingKind {
    Tool,
    Resource(String),
}

/// Route one already-read JSON-RPC line: parse, then hand off to [`route_value`]. Pure — never
/// dispatches. Split from [`route_value`] so the HTTP transport (`mcp::http`), which already
/// receives a parsed request body rather than a text line, can call the shared classifier
/// directly without re-serializing its body just to re-parse it here.
pub(super) fn route_line(line: &str, server: &Server) -> Routed {
    let parsed: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return Routed::Reply(rpc_error(Value::Null, -32700, "Parse error")),
    };
    route_value(parsed, server)
}

/// The classifier both transports share (issue #1173): given one already-parsed JSON-RPC
/// message, decide whether it is dropped (a notification, or `id: null`), answerable locally, or
/// needs the bridge. Identical to what `route_line` did inline before this split — no behaviour
/// change, only a parse/classify split so a caller that already holds a [`Value`] (the HTTP
/// transport's request body) skips the string round-trip.
pub(super) fn route_value(parsed: Value, server: &Server) -> Routed {
    let Some(obj) = parsed.as_object() else {
        return Routed::Reply(rpc_error(Value::Null, -32600, "Invalid Request"));
    };
    let id = obj.get("id").cloned().unwrap_or(Value::Null);
    if id.is_null() {
        return Routed::Drop;
    }
    let method = obj.get("method").and_then(Value::as_str);
    let params = obj.get("params").cloned().unwrap_or_else(|| json!({}));

    let outcome: Result<Value, (i64, &'static str)> = match method {
        None => Err((-32600, "Invalid Request")),
        Some(m) if m.starts_with("notifications/") => return Routed::Drop,
        Some("initialize") => Ok(initialize_result(&params, &server.instructions)),
        Some("ping") => Ok(json!({})),
        Some("tools/list") => Ok(json!({ "tools": server.tools })),
        // Classified HERE, on the writer thread: only a target that really needs the app is
        // handed to the worker; a local tool, a usage error and every local refusal are
        // answered like any other immediate method (module doc).
        Some("tools/call") => match classify_tool_call(&params, server) {
            ToolCall::Local(outcome) => outcome,
            ToolCall::Bridge(verb) => {
                return Routed::Call {
                    id,
                    verb,
                    kind: PendingKind::Tool,
                }
            }
        },
        // `resources/list`/`resources/templates/list` are pure catalogue reads, answered exactly
        // like `tools/list` — no bridge call, no `Tier` gate (issue #1146 P4: every resource here
        // mirrors a curated Read tool, so there is nothing to gate along the `Effect` boundary).
        Some("resources/list") => Ok(json!({ "resources": resources::resources_list() })),
        Some("resources/templates/list") => {
            Ok(json!({ "resourceTemplates": resources::resource_templates() }))
        }
        Some("resources/read") => match resources::classify_resource_read(&params) {
            resources::ResourceCall::Local(outcome) => outcome,
            resources::ResourceCall::Bridge(uri, verb) => {
                return Routed::Call {
                    id,
                    verb,
                    kind: PendingKind::Resource(uri),
                }
            }
        },
        // `prompts/*` never touches the bridge (see `mcp/prompts.rs`'s own doc): both are
        // answered locally, the same as `commands`.
        Some("prompts/list") => Ok(json!({ "prompts": prompts::prompts_list() })),
        Some("prompts/get") => prompts::prompts_get(&params),
        // Everything else — `server/discover` included — is a plain "Method not found", the
        // legacy-fallback signal the 2025-11-25 spec itself defines (see the module doc).
        Some(_) => Err((-32601, "Method not found")),
    };

    Routed::Reply(reply_frame(id, outcome))
}

/// The one per-request handler both transports call (issue #1173): classify an already-parsed
/// JSON-RPC message via [`route_value`] and, for the one outcome that needs the app, dispatch it
/// and wrap the reply — `None` for a dropped notification, `Some(frame)` for everything else. The
/// stdio [`serve`] loop does NOT call this directly: its three-thread split (module doc) exists so
/// a bridge-backed call can be classified without blocking the writer and dispatched without
/// blocking a `ping` behind it, so it composes the same two calls (`route_value` on the writer
/// thread, [`dispatched_tool_result`]/[`resources::dispatched_resource_result`] on the worker)
/// across that boundary instead of in one frame. The stateless HTTP transport (`mcp::http`) has no
/// writer thread to protect and answers one request at a time, so it calls this directly and
/// synchronously — same classifier, same tiers, same throttle (a fresh [`super::query`] bridge
/// round trip per call either way), same result cap, byte-for-byte the same [`tool_result`]/
/// refusal shapes. `kind` (issue #1146 P4) picks the same reply shape [`serve`]'s worker picks for
/// the identical queued call, so a `resources/read` answers identically over either wire, not just
/// `tools/call`. Never used by [`route_line`]/[`route_value`] themselves, which stay pure and
/// dispatch-free.
pub(super) fn handle_message(
    parsed: Value,
    server: &Server,
    dispatch: &mut dyn FnMut(&Verb) -> Result<Value, &'static str>,
) -> Option<Value> {
    match route_value(parsed, server) {
        Routed::Drop => None,
        Routed::Reply(frame) => Some(frame),
        Routed::Call { id, verb, kind } => {
            // `dispatched_resource_result` (T8, PR #1184) can itself be `Err` — an oversized
            // reply capped by `results::capped_result_text` — so both arms are unified as a
            // `Result` here rather than always wrapping in `Ok`, letting `reply_frame` write a
            // real JSON-RPC error for that case exactly as it does for any other refusal.
            let result = match &kind {
                PendingKind::Tool => Ok(dispatched_tool_result(&verb, dispatch)),
                PendingKind::Resource(uri) => {
                    resources::dispatched_resource_result(uri, &verb, dispatch)
                }
            };
            Some(reply_frame(id, result))
        }
    }
}
