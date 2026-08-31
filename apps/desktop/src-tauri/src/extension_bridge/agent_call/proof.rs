//! ADR-038 §4, Phase 3 — resolving an [`Effect::Irreversible`] row's
//! `--confirm` value. Split out of `agent_call.rs` to keep that file under
//! R8's LOC cap (the same reason `documents/sql.rs`/`applications/reminders.rs`
//! exist) — this is real logic, not tests, so it earns its own file rather
//! than living in `agent_call/tests.rs`.
//!
//! Every fn here is split pure/impure: [`resolve`] is the ONLY one that
//! touches [`AppHandle`] — it dispatches `source.read_command()` through
//! [`super::invoke_command`], the SAME real command body every other row
//! already uses, never a second implementation of that command's logic.
//! [`extract`]/[`build_input`]/[`hint`] are pure `Value`-in,
//! `Value`/`String`-out — directly unit-testable with hand-built fixtures,
//! no live app, mirroring this crate's standing pure-core/impure-shell split
//! (`agent_read::resolve_job`/`job_resource`, `resolve_best_matches`/
//! `best_matches_resource`).

use serde_json::Value;
use tauri::AppHandle;

use super::super::agent_cli::policy::{LookupInput, ProofSource, POLICY};

/// The input body `source.read_command()` is invoked with — pure, so a
/// caller-controlled `caller_input` can never smuggle anything past this
/// beyond the ONE key a [`ProofSource::Lookup`] row explicitly forwards.
fn build_input(source: ProofSource, caller_input: &Value) -> Value {
    match source {
        ProofSource::Lookup { key, input, .. } => {
            let value = match input {
                LookupInput::Literal(v) => Value::String(v.to_string()),
                LookupInput::FromCaller(field) => {
                    caller_input.get(field).cloned().unwrap_or(Value::Null)
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
fn walk<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |v, key| v.get(key))
}

/// Render a resolved [`Value`] as the exact string a caller's `--confirm`
/// must match — only scalar shapes are ever a valid proof; an object/array/
/// null is a resolution failure (nothing to confirm against), never
/// stringified as `"null"`/`"{}"` (which would make an ABSENT record
/// satisfiable by typing a literal word).
fn stringify(value: &Value) -> Option<String> {
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
            let target = caller_input.get(id_field)?;
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
            let ids = caller_input.get(ids_field)?.as_array()?;
            let count = response
                .as_array()?
                .iter()
                .filter(|record| record.get(match_field).is_some_and(|id| ids.contains(id)))
                .count();
            Some(count.to_string())
        }
    }
}

/// The impure shell: dispatch `source.read_command()` for real, then
/// [`extract`]. `None` on anything that stops this from producing a usable
/// proof — the caller (`dispatch_irreversible`) turns that into
/// [`super::Refusal::ProofUnavailable`], never a panic and never a value
/// this fn invents.
pub(super) async fn resolve(
    app: &AppHandle,
    source: ProofSource,
    caller_input: &Value,
) -> Option<String> {
    let read_input = build_input(source, caller_input);
    let response = super::invoke_command(app, source.read_command(), read_input)
        .await
        .ok()?;
    extract(source, caller_input, &response)
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
        ProofSource::ListMatch { value_field, .. } => {
            format!("the matching record's own `{value_field}` field")
        }
        ProofSource::Count { .. } => "its own array length (how many records exist)".to_string(),
        ProofSource::MatchCount { .. } => {
            "the count of the targeted ids that actually exist".to_string()
        }
    };
    format!("read {target} and pass {field} as --confirm")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // ── build_input ─────────────────────────────────────────────────────

    #[test]
    fn build_input_forwards_the_named_caller_field_for_a_lookup() {
        let source = ProofSource::Lookup {
            read_command: "autopilot_get",
            key: "autopilotId",
            input: LookupInput::FromCaller("autopilotId"),
            path: &["name"],
        };
        let caller_input = json!({ "autopilotId": "ap-1" });
        assert_eq!(
            build_input(source, &caller_input),
            json!({ "autopilotId": "ap-1" })
        );
    }

    #[test]
    fn build_input_uses_a_literal_regardless_of_caller_input() {
        let source = ProofSource::Lookup {
            read_command: "boards_get_status",
            key: "boardId",
            input: LookupInput::Literal("linkedin"),
            path: &["connected"],
        };
        // Even a caller trying to steer the literal via its own input has no
        // effect — `Literal` never reads `caller_input` at all.
        let caller_input = json!({ "boardId": "attacker-controlled" });
        assert_eq!(
            build_input(source, &caller_input),
            json!({ "boardId": "linkedin" })
        );
    }

    #[test]
    fn build_input_is_empty_for_every_no_input_read_command() {
        for source in [
            ProofSource::Scalar {
                read_command: "system_get_version",
                path: &[],
            },
            ProofSource::ListMatch {
                read_command: "documents_list",
                id_field: "id",
                match_field: "id",
                value_field: "name",
            },
            ProofSource::Count {
                read_command: "notifications_list",
            },
            ProofSource::MatchCount {
                read_command: "ai_generations_list",
                ids_field: "ids",
                match_field: "id",
            },
        ] {
            assert_eq!(build_input(source, &json!({ "id": "x" })), json!({}));
        }
    }

    // ── extract ──────────────────────────────────────────────────────────

    #[test]
    fn extract_scalar_walks_a_nested_path() {
        let source = ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today", "inputTokens"],
        };
        let response = json!({ "today": { "inputTokens": 4200 } });
        assert_eq!(
            extract(source, &json!({}), &response),
            Some("4200".to_string())
        );
    }

    #[test]
    fn extract_scalar_with_empty_path_uses_the_bare_response() {
        let source = ProofSource::Scalar {
            read_command: "system_get_version",
            path: &[],
        };
        let response = json!("0.144.0");
        assert_eq!(
            extract(source, &json!({}), &response),
            Some("0.144.0".to_string())
        );
    }

    #[test]
    fn extract_lookup_walks_a_nested_field() {
        let source = ProofSource::Lookup {
            read_command: "applications_get",
            key: "id",
            input: LookupInput::FromCaller("id"),
            path: &["application", "title"],
        };
        let response = json!({ "application": { "title": "Staff Engineer" }, "events": [] });
        assert_eq!(
            extract(source, &json!({ "id": "app-1" }), &response),
            Some("Staff Engineer".to_string())
        );
    }

    #[test]
    fn extract_lookup_returns_none_for_a_null_response() {
        // `autopilot_get` returns `json!(None::<Autopilot>)` (bare `null`)
        // when the id doesn't exist — must not stringify as `"null"`.
        let source = ProofSource::Lookup {
            read_command: "autopilot_get",
            key: "autopilotId",
            input: LookupInput::FromCaller("autopilotId"),
            path: &["name"],
        };
        assert_eq!(
            extract(source, &json!({ "autopilotId": "gone" }), &Value::Null),
            None
        );
    }

    #[test]
    fn extract_list_match_finds_the_record_by_the_callers_own_id() {
        let source = ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: "id",
            match_field: "id",
            value_field: "name",
        };
        let response = json!([
            { "id": "doc-1", "name": "Resume A" },
            { "id": "doc-2", "name": "Resume B" },
        ]);
        assert_eq!(
            extract(source, &json!({ "id": "doc-2" }), &response),
            Some("Resume B".to_string())
        );
    }

    #[test]
    fn extract_list_match_returns_none_when_the_id_is_not_in_the_list() {
        let source = ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: "id",
            match_field: "id",
            value_field: "name",
        };
        let response = json!([{ "id": "doc-1", "name": "Resume A" }]);
        assert_eq!(
            extract(source, &json!({ "id": "doc-missing" }), &response),
            None
        );
    }

    #[test]
    fn extract_list_match_returns_none_when_the_callers_own_id_field_is_absent() {
        // An empty/omitted selector must never widen to "the first record" —
        // absent input resolves to no proof, not an accidental match.
        let source = ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: "id",
            match_field: "id",
            value_field: "name",
        };
        let response = json!([{ "id": "doc-1", "name": "Resume A" }]);
        assert_eq!(extract(source, &json!({}), &response), None);
    }

    #[test]
    fn extract_count_is_the_array_length() {
        let source = ProofSource::Count {
            read_command: "notifications_list",
        };
        let response = json!([{}, {}, {}]);
        assert_eq!(
            extract(source, &json!({}), &response),
            Some("3".to_string())
        );
    }

    #[test]
    fn extract_count_of_an_empty_list_is_zero() {
        let source = ProofSource::Count {
            read_command: "notifications_list",
        };
        assert_eq!(
            extract(source, &json!({}), &json!([])),
            Some("0".to_string())
        );
    }

    #[test]
    fn extract_match_count_counts_only_the_targeted_ids_that_exist() {
        let source = ProofSource::MatchCount {
            read_command: "ai_generations_list",
            ids_field: "ids",
            match_field: "id",
        };
        let response = json!([
            { "id": "g-1" },
            { "id": "g-2" },
            { "id": "g-3" },
        ]);
        // Two of three requested ids actually exist; the third is a typo/stale id.
        let caller_input = json!({ "ids": ["g-1", "g-3", "g-nonexistent"] });
        assert_eq!(
            extract(source, &caller_input, &response),
            Some("2".to_string())
        );
    }

    #[test]
    fn extract_never_stringifies_null_array_or_object_as_a_proof() {
        // Mutation-style guard: a resolver that fell back to `"null"` or
        // `"{}"` would let a caller satisfy the ceremony by typing that
        // literal word for a record that doesn't exist.
        let source = ProofSource::Scalar {
            read_command: "email_watch_status",
            path: &["address"],
        };
        assert_eq!(
            extract(source, &json!({}), &json!({ "address": Value::Null })),
            None
        );
        let source2 = ProofSource::Scalar {
            read_command: "ai_spend_summary",
            path: &["today"],
        };
        assert_eq!(
            extract(
                source2,
                &json!({}),
                &json!({ "today": { "inputTokens": 1 } })
            ),
            None,
            "an object must never stringify as a proof"
        );
    }

    // ── hint — never discloses a value, always names the read surface ─────

    #[test]
    fn hint_names_the_real_namespaced_read_command() {
        let source = ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: "id",
            match_field: "id",
            value_field: "name",
        };
        let text = hint(source);
        assert!(
            text.contains("agent call documents:documents_list"),
            "{text}"
        );
        assert!(text.contains("name"), "{text}");
    }

    #[test]
    fn hint_never_contains_a_digit_sequence_that_could_be_mistaken_for_a_resolved_value() {
        // Not a full proof of "never leaks the value" (that needs the
        // end-to-end run against a live app — see the manual verification
        // step), but a cheap regression guard: `hint` must be built ONLY
        // from `ProofSource`'s own `'static` field names, never from a
        // resolved `Value`.
        for source in [
            ProofSource::Scalar {
                read_command: "ai_spend_summary",
                path: &["today", "inputTokens"],
            },
            ProofSource::Count {
                read_command: "scrape_list_postings",
            },
        ] {
            let text = hint(source);
            assert!(
                !text.chars().any(|c| c.is_ascii_digit()),
                "hint leaked something numeric: {text}"
            );
        }
    }

    #[test]
    fn hint_falls_back_to_the_bare_command_name_if_somehow_unregistered() {
        // Defensive only — `every_proof_source_read_command_is_a_read_row`
        // (policy.rs) makes this unreachable for a real row, but `hint`
        // itself must still degrade gracefully rather than panic.
        let source = ProofSource::Scalar {
            read_command: "not_a_real_command",
            path: &[],
        };
        assert!(hint(source).contains("not_a_real_command"));
    }
}
