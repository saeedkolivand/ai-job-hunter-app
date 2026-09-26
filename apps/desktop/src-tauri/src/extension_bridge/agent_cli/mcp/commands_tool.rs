//! The LOCAL `commands` tool — the one tool that answers with this binary's own bundled
//! tables and never touches the wire. It is the only tool that exists so an agent can learn
//! what it may call, and what confirming a call would require.

use super::*;

/// `commands`' own `effect` filter values — builds its `inputSchema` enum AND validates an
/// incoming call (MUST FIX — previously nothing validated this at all, so a typo'd filter matched
/// zero rows and answered `{"commands":[]}` with `isError:false` exit 0: a refusal disguised as an
/// empty success).
pub(super) const EFFECT_FILTER_VALUES: &[&str] =
    &["read", "reversible", "irreversible", "not_exposed"];

// ── `commands` (local — no bridge call) ────────────────────────────────

/// `command`'s row in the generated [`CATALOGUE`], or `None` when it is absent (an `invoke()` call
/// the generator could not parse with confidence, or one with no call site at all — its own module
/// doc). `commands` marks that absence with `args: null` (issue #1163) rather than an empty list,
/// which would otherwise be indistinguishable from "this command genuinely takes no arguments".
pub(super) fn catalogue_lookup(command: &str) -> Option<&'static CatalogueEntry> {
    CATALOGUE.iter().find(|entry| entry.command == command)
}

pub(super) fn commands_value(arguments: &Value, tier: Tier) -> Value {
    let effect_filter = arguments.get("effect").and_then(Value::as_str);
    let namespace_filter = arguments.get("namespace").and_then(Value::as_str);
    let rows: Vec<Value> = POLICY
        .iter()
        .filter_map(|entry| {
            let (namespace, command) = agent_call::split_path(entry.path);
            if namespace_filter.is_some_and(|n| n != namespace) {
                return None;
            }
            let effect_name = match entry.effect {
                Effect::Read => "read",
                Effect::Reversible => "reversible",
                Effect::Irreversible(_) => "irreversible",
                Effect::NotExposed(_) => "not_exposed",
            };
            if effect_filter.is_some_and(|f| f != effect_name) {
                return None;
            }
            let mut row =
                json!({ "namespace": namespace, "command": command, "effect": effect_name });
            match catalogue_lookup(command) {
                Some(catalogued) => {
                    if !catalogued.description.is_empty() {
                        row["description"] = json!(catalogued.description);
                    }
                    row["args"] = json!(catalogued
                        .args
                        .iter()
                        .map(|arg| {
                            let mut value = json!({ "name": arg.name, "required": arg.required });
                            // `None` (scalar arg) omits the key entirely — unchanged. `Some(&[])`
                            // (a wrapper type this generator could not resolve — see
                            // `CatalogueArg::fields`'s own doc) is surfaced as an explicit
                            // `null`, distinct from omission, so a caller can tell "known to
                            // take no nested fields" apart from "unknown nested shape" (MEDIUM —
                            // CLI review).
                            match arg.fields {
                                None => {}
                                Some([]) => {
                                    value["fields"] = Value::Null;
                                }
                                Some(fields) => {
                                    value["fields"] = json!(fields);
                                }
                            }
                            value
                        })
                        .collect::<Vec<_>>());
                }
                // `args: null`, never an absent key or an empty array — see this fn's own doc.
                None => row["args"] = Value::Null,
            }
            // A paged row's reply is an ENVELOPE, not the bare array its name suggests
            // (issue #1136). Both the list and the note come from `agent_call`, so this
            // row cannot drift from the behaviour `dispatch_direct` actually applies.
            if agent_call::reshape::PAGINATED_LIST_COMMANDS.contains(&command) {
                row["returns"] = json!(agent_call::reshape::PAGINATED_LIST_NOTE);
            }
            // Same discovery precedent as the paging note just above, for the
            // OTHER reply reshape a generic-tier caller cannot otherwise learn
            // about (round-1 review, issue #1180): plain `call-read` never sees
            // an MCP tool description.
            if command == agent_call::reshape::CONTACT_PROFILE_GET_COMMAND {
                row["returns"] = json!(agent_call::reshape::CONTACT_PROFILE_GET_PROJECTION_NOTE);
            }
            let gate_open = tier_exposes(tier, &entry.effect);
            match tool_for(&entry.effect) {
                Some(tool) if gate_open => row["tool"] = json!(tool),
                Some(_) => row["unavailable"] = json!(unavailable_reason(&entry.effect)),
                None => {}
            }
            match entry.effect {
                Effect::Irreversible(source) => {
                    if let Some(pf) = proof_from(source) {
                        row["proofFrom"] = json!(pf);
                    }
                    // The field a confirm ceremony will require (issue #1160: "what would
                    // deleting this require?" answerable without dispatching) — derived, never
                    // hand-typed, the same as `proofFrom`/`hint`'s own `field` clause.
                    if let Some(field) = agent_call::proof_field_for(source) {
                        row["proofField"] = json!(field);
                    }
                    // What an ABSENT `proofField` means for this row (CLI review round 2 —
                    // MEDIUM): `"count"` — pass the array length / `total`; `"response_value"` —
                    // pass the whole response value; `"field"` — a field IS named above. Carried
                    // on every Irreversible row, not just the ones with a named field, so a
                    // caller never has to dispatch the destructive command just to discover which
                    // shape its own refusal would have described.
                    row["proofKind"] = json!(agent_call::proof_kind_for(source));
                    if let ProofSource::Lookup { key, input, .. } = source {
                        row["proofInput"] = json!(key);
                        // A `Literal` input's VALUE (e.g. `privacy_sign_out_all`'s `boardId` =
                        // `"linkedin"`) is not secret and is otherwise the one thing this
                        // ceremony can't complete from `commands` alone; a `FromCaller` value is
                        // the caller's own input and deliberately never echoed here.
                        if let LookupInput::Literal(value) = input {
                            row["proofInputValue"] = json!(value);
                        }
                    }
                }
                Effect::NotExposed(reason) => row["reason"] = json!(reason),
                _ => {}
            }
            Some(row)
        })
        .collect();
    json!({ "commands": rows })
}
