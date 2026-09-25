//! The `tools/call` path end to end: what a call frame turns out to be, everything about it
//! that can be decided WITHOUT the bridge, and the bridge-backed tail it becomes once it is.

use super::*;

/// What a `tools/call` frame turns out to be, once classified. The split exists because only
/// [`ToolCall::Bridge`] costs a bridge round trip: everything else is decided from this binary's
/// own bundled tables and is answered on the spot, never queued behind an in-flight dispatch (see
/// the module doc's concurrency guarantees).
pub(super) enum ToolCall {
    /// Answered with no wire traffic at all — `commands`, an unknown tool or bad params, a
    /// `parse_verb` usage error, or any [`local_call_refusal`].
    Local(Result<Value, (i64, &'static str)>),
    /// The one outcome that needs the app: dispatch this verb and wrap the reply.
    Bridge(Verb),
}

/// MCP RESERVES every `_`-prefixed key for the protocol itself (`_meta` is the one in use today,
/// and a client may attach it to ANY tool call's `arguments`), so the key-set gate below must skip
/// them: no tool schema declares `_meta`, and refusing it would refuse a spec-conformant call.
/// Matched on the PREFIX rather than an `_meta` literal, because the reservation is on the prefix
/// — and a plain typo (`limt`) carries no `_`, so it is still refused. Nothing downstream reads
/// these keys: [`tool_argv`] builds argv from named keys only, so a reserved key is inert, never
/// forwarded.
pub(super) fn is_reserved_argument_key(key: &str) -> bool {
    key.starts_with('_')
}

/// Everything about a `tools/call` that can be decided WITHOUT the bridge. Pure — no dispatch
/// closure in its signature at all, which is what makes "local tools never queue" a property of
/// the type rather than of a comment: [`serve`] can run this on its writer thread precisely
/// because there is nothing here to block on.
pub(super) fn classify_tool_call(params: &Value, server: &Server) -> ToolCall {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return ToolCall::Local(Err((-32602, "Invalid params")));
    };
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    // Bound ONCE here: a non-object `arguments` is a protocol error before anything reads it, so
    // the key-set gate below — the only reader — needs no second, unreachable `as_object()` arm.
    let Some(given) = arguments.as_object() else {
        return ToolCall::Local(Err((-32602, "Invalid params")));
    };
    let Some(tool) = server
        .tools
        .iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some(name))
    else {
        return ToolCall::Local(Err((-32602, "Unknown tool")));
    };
    // `additionalProperties:false` is advertised on every schema `schema_object` builds and, until
    // now, enforced by nothing (issue #1134): a typo'd OPTIONAL key (`limt`) was dropped in
    // silence and answered with that field's DEFAULT — a quietly wrong page, isError:false. Read
    // off this tool's OWN already-built `inputSchema.properties`, never a second hand-written key
    // list per tool (`tool_for`/`mcp_help_text`'s rule). A `usage` result, not `-32602`, so it
    // keeps the exitCode block every refusal carries; the offending key is caller-authored text
    // and so is never echoed — the detail names the DECLARED set instead.
    let declared: Vec<&str> = tool["inputSchema"]["properties"]
        .as_object()
        .map_or_else(Vec::new, |p| p.keys().map(String::as_str).collect());
    if given
        .keys()
        .any(|k| !is_reserved_argument_key(k) && !declared.contains(&k.as_str()))
    {
        let detail = if declared.is_empty() {
            "unknown argument (this tool accepts none)".to_string()
        } else {
            format!(
                "unknown argument (this tool accepts: {})",
                declared.join(", ")
            )
        };
        return ToolCall::Local(Ok(tool_result(usage_error_value(&detail), 2)));
    }

    if name == TOOL_COMMANDS {
        // MUST FIX — an `effect` outside the declared enum, or not even a STRING (`{"effect":5}`
        // skipped the old `and_then(Value::as_str)` gate entirely — review round 3, item 20),
        // used to fall through and match nothing, answering `{"commands":[]}` isError:false exit
        // 0. A PRESENT `effect` must be a valid string or this is a usage error; an ABSENT one
        // means "no filter" and is fine.
        if let Some(effect_value) = arguments.get("effect") {
            let valid = effect_value
                .as_str()
                .is_some_and(|s| EFFECT_FILTER_VALUES.contains(&s));
            if !valid {
                return ToolCall::Local(Ok(tool_result(
                    usage_error_value(
                        "effect must be one of read, reversible, irreversible, not_exposed",
                    ),
                    2,
                )));
            }
        }
        // Same reasoning as `effect` just above, for the SAME failure shape (issue #1163's
        // `namespace` filter): a typo'd namespace would otherwise match zero rows and answer
        // `{"commands":[]}` isError:false exit 0 — a refusal disguised as an empty success.
        // `namespace` has no small enum to advertise in the schema (unlike `effect`), so it is
        // checked against POLICY's own real namespace set rather than a hand-typed list.
        if let Some(namespace_value) = arguments.get("namespace") {
            let valid = namespace_value.as_str().is_some_and(|s| {
                POLICY
                    .iter()
                    .any(|entry| agent_call::split_path(entry.path).0 == s)
            });
            if !valid {
                return ToolCall::Local(Ok(tool_result(
                    usage_error_value("namespace does not match any real command's namespace"),
                    2,
                )));
            }
        }
        return ToolCall::Local(Ok(tool_result(commands_value(&arguments, server.tier), 0)));
    }

    // B3-r1-F2 — a PRESENT-but-blank `autopilotId` used to collapse to the
    // same argv [`tool_argv`] builds for an OMITTED one (`.filter(|s|
    // !s.is_empty())` before the push below), silently widening a
    // one-autopilot selector into a spanning traversal
    // (`agent-cli-standards`: an empty selector must never mean "all"). A
    // flag-shaped value (`"--include-description"`) was WORSE: forwarded as
    // the bare leading positional [`tool_argv`] builds, [`parse_found_jobs`]
    // reads it as a real flag rather than as an id, since it doesn't look
    // like one — turning on a filter the caller never asked for. Checked
    // HERE, before argv is built, rather than inside [`tool_argv`] (which
    // never validates anything itself, by its own documented contract) —
    // mirrors `found_jobs::parse_autopilot_id_arg`'s identical guard on the
    // SAME field one hop further in.
    if name == TOOL_FOUND_JOBS {
        if let Some(id) = arguments.get("autopilotId").filter(|v| !v.is_null()) {
            let usable = id
                .as_str()
                .is_some_and(|s| !s.trim().is_empty() && !s.trim().starts_with("--"));
            if !usable {
                return ToolCall::Local(Ok(tool_result(
                    usage_error_value(
                        "autopilotId must be a non-empty id, not blank or flag-shaped — omit \
                         the key entirely to span every autopilot",
                    ),
                    2,
                )));
            }
        }
        // Round 2 fix (B3-r2-F4) — `tool_argv`'s `includeDescription` arm used to read this
        // value with `.and_then(Value::as_bool)`, the exact silent-drop combinator this fn's own
        // doc says every optional argument avoids: a non-bool (`"true"`, `1`) vanished as
        // "absent" rather than reaching `parse_verb`, so the resource-level refusal for the
        // identical value one hop further in (`found_jobs::bool_filter`, via
        // `FoundJobsFilters::from_payload`) could never fire — the caller got compact rows with
        // no error and no signal that `description` was silently dropped. Checked HERE, before
        // argv is built, mirroring the `autopilotId` guard above on the SAME tool.
        if let Some(v) = arguments.get("includeDescription").filter(|v| !v.is_null()) {
            if v.as_bool().is_none() {
                return ToolCall::Local(Ok(tool_result(
                    usage_error_value("includeDescription must be a boolean"),
                    2,
                )));
            }
        }
    }

    let argv = tool_argv(name, &arguments);
    let verb = match parse_verb(&argv) {
        Ok(v) => v,
        Err(e) => return ToolCall::Local(Ok(tool_result(usage_error_value(&e.to_string()), 2))),
    };

    if let Some(refusal) = local_call_refusal(name, &verb, server.tier) {
        return ToolCall::Local(Ok(tool_result(refusal, 2)));
    }

    ToolCall::Bridge(verb)
}

/// The bridge-backed TAIL of a `tools/call` — the only part that touches the wire, and so the
/// only part [`serve`] hands to its worker thread. Split out of [`tool_call_result`] so the
/// dispatch closure appears in exactly one signature. Shares [`results::dispatch_payload`] with
/// [`resources::dispatched_resource_result`] (issue #1146 P4) — see that fn's own doc.
pub(super) fn dispatched_tool_result(
    verb: &Verb,
    dispatch: &mut dyn FnMut(&Verb) -> Result<Value, &'static str>,
) -> Value {
    let (payload, code) = results::dispatch_payload(verb, dispatch);
    tool_result(payload, code)
}
