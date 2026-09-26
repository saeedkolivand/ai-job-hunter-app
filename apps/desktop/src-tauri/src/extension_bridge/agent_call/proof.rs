//! ADR-038 §4, Phase 3 — resolving an [`Effect::Irreversible`] row's `--confirm` value. Split out
//! of `agent_call.rs` under R8's LOC cap.
//!
//! Split pure/impure: [`resolve`] is the ONLY fn touching [`AppHandle`] — it dispatches
//! `source.read_command()` through [`super::invoke_command`], the SAME real command body every
//! other row uses. [`extract`]/[`build_input`]/[`hint`] are pure `Value`-in, `Value`/`String`-out —
//! directly unit-testable with hand-built fixtures, mirroring this crate's standing
//! pure-core/impure-shell split (`agent_read::resolve_job`/`job_resource`).

use serde_json::Value;
use tauri::AppHandle;

use super::super::agent_cli::policy::{LookupInput, ProofSource, POLICY};

/// The input body `source.read_command()` is invoked with — pure, so a
/// caller-controlled `caller_input` can never smuggle anything past this
/// beyond the ONE key a [`ProofSource::Lookup`] row explicitly forwards.
/// `LookupInput::FromCaller` is a PATH into `caller_input` (walked via
/// [`walk`], the SAME fn a response path uses below) rather than a flat
/// top-level field — see `LookupInput::FromCaller`'s own doc for why a flat
/// field silently read the wrong location for a command whose
/// `#[tauri::command]` signature wraps its args in one `req` struct.
fn build_input(source: ProofSource, caller_input: &Value) -> Value {
    match source {
        ProofSource::Lookup { key, input, .. } => {
            let value = match input {
                LookupInput::Literal(v) => Value::String(v.to_string()),
                LookupInput::FromCaller(path) => {
                    walk(caller_input, path).cloned().unwrap_or(Value::Null)
                }
            };
            serde_json::json!({ key: value })
        }
        ProofSource::Scalar { .. }
        | ProofSource::ListMatch { .. }
        | ProofSource::Count { .. }
        | ProofSource::MatchCount { .. } => serde_json::json!({}),
    }
}

/// Walk `path` (a sequence of object keys) into `value`; an empty `path`
/// returns `value` itself (e.g. `system_get_version`'s bare string response).
/// ONE walker for BOTH directions this module reads a path from (HIGH fix —
/// security review round 2): a `ProofSource`'s own response `path`
/// ([`ProofSource::Scalar`]/[`ProofSource::Lookup`]) AND a
/// [`LookupInput::FromCaller`]/`ProofSource::ListMatch`'s `id_field`/
/// `ProofSource::MatchCount`'s `ids_field` path into the CALLER's `--input`
/// — never two copies of the same walk that could silently diverge.
pub(super) fn walk<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |v, key| v.get(key))
}

/// Render a resolved [`Value`] as the exact string a caller's `--confirm`
/// must match — only scalar shapes are ever a valid proof; an object/array/
/// null is a resolution failure (nothing to confirm against), never
/// stringified as `"null"`/`"{}"` (which would make an ABSENT record
/// satisfiable by typing a literal word).
pub(super) fn stringify(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// The pure extraction core: given the ALREADY-FETCHED `read_command`
/// response and the irreversible command's own `caller_input`, compute the
/// expected proof string — or `None` if the target record cannot be
/// resolved (e.g. deleting a document whose id no longer exists, or a
/// caller who omitted the id field the ceremony needs to look it up).
pub(super) fn extract(
    source: ProofSource,
    caller_input: &Value,
    response: &Value,
) -> Option<String> {
    match source {
        ProofSource::Scalar { path, .. } | ProofSource::Lookup { path, .. } => {
            stringify(walk(response, path)?)
        }
        ProofSource::ListMatch {
            id_field,
            match_field,
            value_field,
            ..
        } => {
            let target = walk(caller_input, id_field)?;
            let record = response
                .as_array()?
                .iter()
                .find(|record| record.get(match_field) == Some(target))?;
            stringify(record.get(value_field)?)
        }
        ProofSource::Count { .. } => Some(response.as_array()?.len().to_string()),
        ProofSource::MatchCount {
            ids_field,
            match_field,
            ..
        } => {
            let ids = walk(caller_input, ids_field)?.as_array()?;
            let count = response
                .as_array()?
                .iter()
                .filter(|record| record.get(match_field).is_some_and(|id| ids.contains(id)))
                .count();
            Some(count.to_string())
        }
    }
}

/// Fence `response` the SAME way [`super::dispatch_direct`] fences every other response this
/// dispatcher hands to a caller, then [`extract`] (HIGH fix, security review round 4): otherwise a
/// proof bound to a fenced field (`FENCE_FIELD_NAMES`) is permanently unsatisfiable, since a caller
/// can only ever read the FENCED value back — hit `applications_delete`/`notifications_remove`'s
/// `title` proof the moment `title` joined the fence list.
///
/// Calls [`super::reshape::reshape_pre_fence`] then [`super::reshape::fence_reply`] — the SAME
/// composition [`super::reshape::reshape_reply`] runs up to (not including) paging/base64, never a
/// hand-rolled subset — so a FUTURE fenced/pre-fenced field is covered here automatically.
fn extract_from_fenced_response(
    source: ProofSource,
    caller_input: &Value,
    mut response: Value,
) -> Option<String> {
    let command = source.read_command();
    super::reshape::reshape_pre_fence(command, &mut response);
    super::reshape::fence_reply(command, &mut response);
    extract(source, caller_input, &response)
}

/// The impure shell: dispatch `source.read_command()` for real, then
/// [`extract_from_fenced_response`]. `None` on anything that stops this from
/// producing a usable proof — the caller (`dispatch_irreversible`) turns
/// that into [`super::Refusal::ProofUnavailable`], never a panic and never a
/// value this fn invents.
pub(super) async fn resolve(
    app: &AppHandle,
    source: ProofSource,
    caller_input: &Value,
) -> Option<String> {
    let read_input = build_input(source, caller_input);
    let outcome = super::invoke_command(app, source.read_command(), read_input)
        .await
        .ok()?;
    let response = match outcome {
        super::InvokeOutcome::Success(v) => v,
        // The read this proof depends on itself hit a Tauri-level error
        // (HIGH fix — security review: the old fold-into-Ok behaviour meant
        // this used to treat that error VALUE as the resolved proof) — no
        // proof value exists to extract; degrade to `None` like any other
        // resolution failure, never a panic and never a value invented here.
        super::InvokeOutcome::CommandErr(_) => return None,
    };
    extract_from_fenced_response(source, caller_input, response)
}

/// The proof value's own field NAME/PATH (never the value) — `commands`' `proofField` row (issue
/// #1160): "what would deleting this require?" answerable without dispatching. Joined with `.` for
/// a multi-segment `Scalar`/`Lookup` path (e.g. `"application.title"`) — the full path a caller
/// would read a response at, unlike [`hint`]'s own per-variant match, which only needs the LAST
/// segment (to check membership in `super::FENCE_FIELD_NAMES`). `None` for `Count`/`MatchCount`
/// (the proof is a DERIVED number, not a field on the read response — nothing to name) and for a
/// `Scalar`/`Lookup` with an EMPTY path (the proof is the bare response value itself, e.g.
/// `system_get_version`'s — same "its own response value" case [`hint`] spells out in prose).
/// `pub(super)` — re-exported by [`super::proof_field_for`] for `agent_cli::mcp` (this module
/// itself stays private to `agent_call`; see that fn's own doc for why).
pub(super) fn proof_field(source: ProofSource) -> Option<String> {
    match source {
        ProofSource::Scalar { path, .. } | ProofSource::Lookup { path, .. } => {
            (!path.is_empty()).then(|| path.join("."))
        }
        ProofSource::ListMatch { value_field, .. } => Some(value_field.to_string()),
        ProofSource::Count { .. } | ProofSource::MatchCount { .. } => None,
    }
}

/// What [`proof_field`]'s `None` means for THIS `source` — the discriminator CLI review round 2
/// (MEDIUM) asked for: an absent `proofField` used to conflate two unrelated causes (`Count`/
/// `MatchCount`, where the proof is a DERIVED NUMBER with no field to name at all, and a `Scalar`/
/// `Lookup` with an empty path, where the proof IS the response value, just unnamed) into one
/// "nothing to go on" shape — exactly the `CatalogueArg::fields` null-vs-omitted collapse this
/// same table already went out of its way to avoid. Always returns a value (never `None` itself)
/// so `commands` can carry it on EVERY `Irreversible` row, not only the 26 with a named field.
pub(super) fn proof_kind(source: ProofSource) -> &'static str {
    match source {
        ProofSource::Scalar { path, .. } | ProofSource::Lookup { path, .. } if path.is_empty() => {
            "response_value"
        }
        ProofSource::Scalar { .. } | ProofSource::Lookup { .. } | ProofSource::ListMatch { .. } => {
            "field"
        }
        ProofSource::Count { .. } | ProofSource::MatchCount { .. } => "count",
    }
}

/// The `ConfirmationRequired` refusal's own detail text — names WHICH read
/// surface and field the proof comes from, NEVER the value (ADR-038 §4's
/// entire point). The namespace prefix is derived from [`POLICY`] itself via
/// [`super::split_path`] (never a second hand-typed mapping), so it can
/// never drift from the table that actually backs it.
pub(super) fn hint(source: ProofSource) -> String {
    let bare = source.read_command();
    let target = POLICY
        .iter()
        .find_map(|entry| {
            let (ns, cmd) = super::split_path(entry.path);
            (cmd == bare).then(|| format!("`agent call {ns}:{cmd}`"))
        })
        .unwrap_or_else(|| format!("`{bare}`"));
    let field = match source {
        ProofSource::Scalar { path, .. } | ProofSource::Lookup { path, .. } => {
            if path.is_empty() {
                "its own response value".to_string()
            } else {
                format!("its own `{}` field", path.join("."))
            }
        }
        // These three may read a paginated list command (issue #1136 wrapped such replies in
        // `{items,total,nextCursor}`); worded generically so it stays correct whether or not the
        // read command is paged, rather than a second per-command list to drift from the first.
        ProofSource::ListMatch { value_field, .. } => format!(
            "the matching record's own `{value_field}` field (paging with `cursor` if it is \
             not on the first page)"
        ),
        ProofSource::Count { .. } => {
            "its own array length: how many records exist (a paged reply reports this as \
             `total`)"
                .to_string()
        }
        ProofSource::MatchCount { .. } => {
            "the count of the targeted ids that actually exist (paging with `cursor` if they \
             are not all on the first page)"
                .to_string()
        }
    };
    format!("read {target} and pass {field} as --confirm")
}

mod grace_window;

pub(super) use grace_window::{
    accepted, grace_window_key, refresh_from_read, remember, SnapshotOutcome,
};
#[cfg(test)]
pub(super) use grace_window::{
    accepted_at, remember_at, GRACE_WINDOW_KEY_TEST_LOCK, GRACE_WINDOW_PATH,
    GRACE_WINDOW_READ_COMMAND, PROOF_SNAPSHOT_TTL,
};

#[cfg(test)]
mod tests;
