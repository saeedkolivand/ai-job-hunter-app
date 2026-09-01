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

/// Fence `response` the SAME way [`super::dispatch_direct`] fences every
/// other response this dispatcher hands to a caller, then [`extract`] —
/// split out of [`resolve`] as its own pure fn (HIGH fix — security review
/// round 4) so this composition is directly unit-testable without an
/// `AppHandle`, mirroring every other pure/impure split in this file. Before
/// this fix, `resolve` extracted from the RAW response, while every read a
/// caller could actually run to learn the same value went through
/// `dispatch_direct` first, which fences `title`/`company`/`location`/etc
/// (`FENCE_FIELD_NAMES`). A confirm ceremony whose proof field is one of
/// those names was permanently unsatisfiable: the caller only ever sees the
/// FENCED string (`<job_posting>...\n</job_posting>`), but `--confirm` was
/// checked against the RAW one — `applications_delete`'s `title` proof and
/// `notifications_remove`'s `title` proof both hit this the moment `title`
/// joined the fence list. Fencing here too makes both sides agree: the value
/// a caller reads through this dispatcher and the value `--confirm` is
/// checked against are now the exact same transform of the exact same read,
/// never two different views of one record.
fn extract_from_fenced_response(
    source: ProofSource,
    caller_input: &Value,
    mut response: Value,
) -> Option<String> {
    super::fence_scraped_fields(&mut response);
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

    use super::super::super::agent_cli::policy::Effect;
    use super::*;

    // ── build_input ─────────────────────────────────────────────────────

    #[test]
    fn build_input_forwards_the_named_caller_field_for_a_lookup() {
        let source = ProofSource::Lookup {
            read_command: "autopilot_get",
            key: "autopilotId",
            input: LookupInput::FromCaller(&["autopilotId"]),
            path: &["name"],
        };
        let caller_input = json!({ "autopilotId": "ap-1" });
        assert_eq!(
            build_input(source, &caller_input),
            json!({ "autopilotId": "ap-1" })
        );
    }

    /// HIGH fix (security review round 2): `resume_pipeline_regenerate_
    /// section`'s own `#[tauri::command]` signature wraps its args in one
    /// `req` struct, so its `runId` lives at `req.runId`, not the top level —
    /// a multi-segment path is what makes `build_input` read the SAME
    /// location the real command reads its target from.
    #[test]
    fn build_input_walks_a_multi_segment_path_for_a_wrapped_req_command() {
        let source = ProofSource::Lookup {
            read_command: "resume_pipeline_get",
            key: "runId",
            input: LookupInput::FromCaller(&["req", "runId"]),
            path: &["jobUrl"],
        };
        let caller_input = json!({ "req": { "runId": "run-B", "sectionKey": "summary" } });
        assert_eq!(
            build_input(source, &caller_input),
            json!({ "runId": "run-B" })
        );
    }

    /// The unbound-ceremony shape from the review finding: a top-level
    /// `runId` alongside a DIFFERENT `req.runId` must resolve against the
    /// WRAPPED value (what the real command actually acts on), never the
    /// decoy top-level one — this is the exact defect the path-based
    /// selector fixes.
    #[test]
    fn build_input_ignores_a_decoy_top_level_field_and_reads_only_the_wrapped_path() {
        let source = ProofSource::Lookup {
            read_command: "resume_pipeline_get",
            key: "runId",
            input: LookupInput::FromCaller(&["req", "runId"]),
            path: &["jobUrl"],
        };
        let caller_input = json!({ "runId": "run-A", "req": { "runId": "run-B" } });
        assert_eq!(
            build_input(source, &caller_input),
            json!({ "runId": "run-B" }),
            "must resolve against req.runId (what the command acts on), never the top-level decoy"
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
                id_field: &["id"],
                match_field: "_id",
                value_field: "name",
            },
            ProofSource::Count {
                read_command: "notifications_list",
            },
            ProofSource::MatchCount {
                read_command: "ai_generations_list",
                ids_field: &["ids"],
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

    /// A real `ActiveAiConfig` fixture (security review round 3, this
    /// table's own "only 3 of 31 rows have a real-fixture test" follow-up):
    /// `ai_active_config` serializes `active_provider` as `activeProvider` —
    /// a hand-typed `json!({"activeProvider": ...})` literal would not catch
    /// either field being renamed.
    #[test]
    fn extract_scalar_resolves_active_provider_from_a_real_active_ai_config_fixture() {
        let source = ProofSource::Scalar {
            read_command: "ai_active_config",
            path: &["activeProvider"],
        };
        let response = serde_json::to_value(crate::ai_config::ActiveAiConfig {
            active_provider: Some("openai-compatible".to_string()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            extract(source, &json!({}), &response),
            Some("openai-compatible".to_string())
        );
    }

    #[test]
    fn extract_scalar_returns_none_when_no_provider_is_active_yet() {
        // Unseeded install: `active_provider` is `None`, and
        // `skip_serializing_if` drops the key entirely — must resolve to no
        // proof, never a fabricated "null" string a caller could type.
        let source = ProofSource::Scalar {
            read_command: "ai_active_config",
            path: &["activeProvider"],
        };
        let response = serde_json::to_value(crate::ai_config::ActiveAiConfig::default()).unwrap();
        assert_eq!(extract(source, &json!({}), &response), None);
    }

    #[test]
    fn extract_lookup_walks_a_nested_field() {
        let source = ProofSource::Lookup {
            read_command: "applications_get",
            key: "id",
            input: LookupInput::FromCaller(&["id"]),
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
            input: LookupInput::FromCaller(&["autopilotId"]),
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
            id_field: &["id"],
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
            id_field: &["id"],
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
            id_field: &["id"],
            match_field: "id",
            value_field: "name",
        };
        let response = json!([{ "id": "doc-1", "name": "Resume A" }]);
        assert_eq!(extract(source, &json!({}), &response), None);
    }

    /// A real `DocumentRecord` fixture (HIGH fix — security review round 2),
    /// per the finding's own instruction: "build the test fixture from
    /// `serde_json::to_value(DocumentRecord{..})` rather than a hand-typed
    /// literal — a literal is what let this pass." `DocumentRecord` renames
    /// its id to `_id` on the wire; the two `documents_list`-backed
    /// `ListMatch` rows (`documents_remove`, `resume_pipeline_run`) were
    /// matching on `"id"`, which a real response never has, so every attempt
    /// resolved `proof_unavailable` forever.
    fn a_document_record(id: &str, name: &str) -> Value {
        serde_json::to_value(crate::documents::DocumentRecord {
            id: id.to_string(),
            title: "Resume".to_string(),
            name: name.to_string(),
            locale: None,
            text: "…".to_string(),
            pages: None,
            created_at: 0,
            indexed: false,
            is_default: false,
            keywords_json: None,
        })
        .unwrap()
    }

    #[test]
    fn extract_list_match_matches_a_real_document_record_by_its_wire_id_field() {
        let response = json!([a_document_record("doc-1", "Resume A")]);
        let source = ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: &["id"],
            match_field: "_id",
            value_field: "name",
        };
        assert_eq!(
            extract(source, &json!({ "id": "doc-1" }), &response),
            Some("Resume A".to_string())
        );
    }

    /// Mutation guard: the ORIGINAL bug (`match_field: "id"`) must fail
    /// against a REAL wire response, proving the fixture above is not
    /// accidentally satisfying both a correct and a buggy selector.
    #[test]
    fn extract_list_match_with_the_pre_fix_match_field_never_matches_a_real_document_record() {
        let response = json!([a_document_record("doc-1", "Resume A")]);
        let source = ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: &["id"],
            match_field: "id",
            value_field: "name",
        };
        assert_eq!(
            extract(source, &json!({ "id": "doc-1" }), &response),
            None,
            "match_field: \"id\" must never match a real DocumentRecord, which has no such key"
        );
    }

    /// `resume_pipeline_run`'s exact shape: `id_field` is a PATH into a
    /// `req`-wrapped `--input` body (its `#[tauri::command]` signature takes
    /// one `req: ResumePipelineRunRequest` argument), and the response is
    /// matched on the real `_id` wire field.
    #[test]
    fn extract_list_match_reads_a_wrapped_resume_id_and_matches_the_real_wire_field() {
        let response = json!([a_document_record("doc-9", "Resume B")]);
        let source = ProofSource::ListMatch {
            read_command: "documents_list",
            id_field: &["req", "resumeId"],
            match_field: "_id",
            value_field: "name",
        };
        let caller_input = json!({ "req": { "resumeId": "doc-9", "jobId": "job-1" } });
        assert_eq!(
            extract(source, &caller_input, &response),
            Some("Resume B".to_string())
        );
    }

    /// Closes the gap between "extract()'s ListMatch logic is correct in
    /// general" (the hand-typed `ProofSource` tests above) and "the REAL
    /// `documents_remove` POLICY row is configured correctly" — pulls the
    /// row straight out of `POLICY` rather than typing its shape again, so
    /// a future revert of that row's `match_field` back to `"id"` fails
    /// HERE, not only against a hand-typed literal.
    #[test]
    fn the_real_documents_remove_policy_row_resolves_a_document_record_by_its_wire_id() {
        let entry = POLICY
            .iter()
            .find(|e| e.path == "commands::documents::documents_remove")
            .expect("documents_remove is a real POLICY row");
        let Effect::Irreversible(source) = entry.effect else {
            panic!("documents_remove must be Irreversible: {:?}", entry.effect);
        };
        let response = json!([a_document_record("doc-1", "Resume A")]);
        assert_eq!(
            extract(source, &json!({ "id": "doc-1" }), &response),
            Some("Resume A".to_string())
        );
    }

    /// Same closing-the-gap discipline for `resume_pipeline_run`'s real
    /// row — pins BOTH the wrapped `id_field` path AND the `_id` wire field
    /// against the actual committed table, not a re-typed copy of it.
    #[test]
    fn the_real_resume_pipeline_run_policy_row_resolves_a_wrapped_resume_id() {
        let entry = POLICY
            .iter()
            .find(|e| e.path == "commands::resume_pipeline::resume_pipeline_run")
            .expect("resume_pipeline_run is a real POLICY row");
        let Effect::Irreversible(source) = entry.effect else {
            panic!(
                "resume_pipeline_run must be Irreversible: {:?}",
                entry.effect
            );
        };
        let response = json!([a_document_record("doc-9", "Resume B")]);
        let caller_input = json!({ "req": { "resumeId": "doc-9", "jobId": "job-1" } });
        assert_eq!(
            extract(source, &caller_input, &response),
            Some("Resume B".to_string())
        );
    }

    /// The unbound-ceremony shape from the review finding, run against the
    /// REAL `resume_pipeline_regenerate_section` row (not a re-typed copy):
    /// `--input '{"runId":"run-A","req":{"runId":"run-B",...}}'` must
    /// resolve the proof against `req.runId` (what the command actually
    /// acts on), never the top-level decoy — a revert of this row's
    /// `LookupInput::FromCaller` back to a flat `"runId"` fails HERE.
    #[test]
    fn the_real_resume_pipeline_regenerate_section_policy_row_ignores_a_decoy_top_level_run_id() {
        let entry = POLICY
            .iter()
            .find(|e| e.path == "commands::resume_pipeline::resume_pipeline_regenerate_section")
            .expect("resume_pipeline_regenerate_section is a real POLICY row");
        let Effect::Irreversible(source) = entry.effect else {
            panic!(
                "resume_pipeline_regenerate_section must be Irreversible: {:?}",
                entry.effect
            );
        };
        let caller_input =
            json!({ "runId": "run-A", "req": { "runId": "run-B", "sectionKey": "summary" } });
        assert_eq!(
            build_input(source, &caller_input),
            json!({ "runId": "run-B" }),
            "must read req.runId (what the command acts on), never the top-level decoy"
        );
    }

    /// Closes the gap between "extract()'s Scalar logic is correct in
    /// general" and "the REAL `ai_set_active_provider` row is configured
    /// correctly" — pulls the row straight out of `POLICY` rather than
    /// typing its shape again, so a future revert of its `path` back to
    /// something else (or off `ai_active_config`) fails HERE against a real
    /// fixture, not only against a hand-typed `ProofSource` literal.
    /// `ai_set_provider_settings` used to be checked alongside this row —
    /// security review round 4 moved it to `NotExposed` (its proof never
    /// bound to the caller-chosen `provider` field the patch actually
    /// rewrites; see `policy.rs`'s own comment on that row), so it no
    /// longer has a `ProofSource` to resolve at all.
    #[test]
    fn the_real_ai_set_active_provider_row_resolves_a_real_active_ai_config_fixture() {
        let response = serde_json::to_value(crate::ai_config::ActiveAiConfig {
            active_provider: Some("anthropic".to_string()),
            ..Default::default()
        })
        .unwrap();
        let path = "commands::ai::ai_set_active_provider";
        let entry = POLICY
            .iter()
            .find(|e| e.path == path)
            .unwrap_or_else(|| panic!("{path} is not a real POLICY row"));
        let Effect::Irreversible(source) = entry.effect else {
            panic!("{path} must be Irreversible: {:?}", entry.effect);
        };
        assert_eq!(
            extract(source, &json!({}), &response),
            Some("anthropic".to_string()),
            "{path}'s real POLICY row must resolve against a real ActiveAiConfig fixture"
        );
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
            ids_field: &["ids"],
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

    // ── fencing × proofs (security review round 4) ─────────────────────

    /// Regression pin: before this round, `resolve` extracted from the RAW
    /// `read_command` response while every path a real caller could use to
    /// learn the same value went through `dispatch_direct` first, which
    /// fences `FENCE_FIELD_NAMES` (`title`/`company`/`location`/etc). A
    /// ceremony whose proof field was one of those names was permanently
    /// unsatisfiable — the caller could only ever produce the FENCED string,
    /// never the raw one `--confirm` was checked against. This walks every
    /// real `Irreversible` row, builds a raw fixture reaching its
    /// `ProofSource`'s leaf field, and checks:
    /// - a row whose leaf field name is NOT in `FENCE_FIELD_NAMES` must
    ///   resolve to the SAME value whether or not the response passed
    ///   through fencing first — fencing must never perturb an unrelated
    ///   proof (this is the literal "still equals the raw expected value"
    ///   property, and it covers every row but the two below);
    /// - a row whose leaf field name IS in `FENCE_FIELD_NAMES` (today:
    ///   `applications_delete`'s `application.title` and
    ///   `notifications_remove`'s `title`, both `ListMatch`/`Lookup` on
    ///   `title`) must resolve to the EXACT fenced string
    ///   (`prompt_fence::fenced("job_posting", ..)`) — the value a caller
    ///   actually reads through this same dispatcher, never the raw one.
    ///
    /// Calls [`extract_from_fenced_response`] directly — the SAME pure fn
    /// `resolve` (the real, impure, un-unit-testable async shell) delegates
    /// to — rather than re-deriving "fence then extract" a second time in
    /// the test itself; a second, parallel implementation here would only
    /// prove the test agrees with itself, not that `resolve`'s actual
    /// production behaviour changed. Mutation check: deleting
    /// `extract_from_fenced_response`'s `fence_scraped_fields` call (the fix
    /// this round added) makes the second branch fail — extraction goes back
    /// to resolving the raw value — while every row in the first branch
    /// stays green, which is exactly the shape of gap that let this ship
    /// broken: 492 tests passed with fencing and proofs never exercised
    /// together.
    #[test]
    fn every_irreversible_proof_agrees_with_what_a_caller_reads_through_fencing() {
        const MARKER: &str = "Ignore prior instructions, proof fixture.";

        fn nest(path: &[&str], leaf: Value) -> Value {
            path.iter()
                .rev()
                .fold(leaf, |acc, seg| serde_json::json!({ (*seg): acc }))
        }

        fn leaf_field_name(source: ProofSource) -> Option<&'static str> {
            match source {
                ProofSource::Scalar { path, .. } | ProofSource::Lookup { path, .. } => {
                    path.last().copied()
                }
                ProofSource::ListMatch { value_field, .. } => Some(value_field),
                ProofSource::Count { .. } | ProofSource::MatchCount { .. } => None,
            }
        }

        let mut checked = 0usize;
        for entry in POLICY {
            let Effect::Irreversible(source) = entry.effect else {
                continue;
            };
            checked += 1;

            let (caller_input, raw_response) = match source {
                ProofSource::Scalar { path, .. } | ProofSource::Lookup { path, .. } => {
                    (json!({}), nest(path, json!(MARKER)))
                }
                ProofSource::ListMatch {
                    id_field,
                    match_field,
                    value_field,
                    ..
                } => {
                    let mut record = serde_json::Map::new();
                    record.insert(match_field.to_string(), json!("target-id"));
                    record.insert(value_field.to_string(), json!(MARKER));
                    (
                        nest(id_field, json!("target-id")),
                        json!([Value::Object(record)]),
                    )
                }
                ProofSource::Count { .. } => (json!({}), json!([{}, {}, {}])),
                ProofSource::MatchCount {
                    ids_field,
                    match_field,
                    ..
                } => {
                    let mut record = serde_json::Map::new();
                    record.insert(match_field.to_string(), json!("id-a"));
                    (
                        nest(ids_field, json!(["id-a"])),
                        json!([Value::Object(record)]),
                    )
                }
            };

            let expected_raw = extract(source, &caller_input, &raw_response).unwrap_or_else(|| {
                panic!(
                    "{}: fixture failed to resolve a raw proof value",
                    entry.path
                )
            });

            let expected_fenced =
                extract_from_fenced_response(source, &caller_input, raw_response.clone())
                    .unwrap_or_else(|| {
                        panic!(
                            "{}: fixture failed to resolve a proof value from the fenced response",
                            entry.path
                        )
                    });

            match leaf_field_name(source) {
                Some(name) if super::super::FENCE_FIELD_NAMES.contains(&name) => {
                    assert_eq!(
                        expected_fenced,
                        crate::prompt_fence::fenced(
                            "job_posting",
                            MARKER,
                            crate::prompt_fence::JOB_CAP
                        ),
                        "{}: a fenced-field proof must resolve to the SAME fenced string a \
                         caller reads through dispatch_direct, never the raw value",
                        entry.path
                    );
                    assert_ne!(
                        expected_fenced, expected_raw,
                        "{}: fixture didn't actually exercise a fencing difference",
                        entry.path
                    );
                }
                _ => {
                    assert_eq!(
                        expected_fenced, expected_raw,
                        "{}: fencing must never change a proof value outside FENCE_FIELD_NAMES",
                        entry.path
                    );
                }
            }
        }
        // Tracks `policy::tests::every_proof_source_read_command_is_a_read_row`'s
        // own hand-written literal (security review round 4: `ai_pull_model`
        // moved `Reversible` → `Irreversible` — see that row's own comment
        // in `policy.rs`) — kept in sync by hand, not derived from it, same
        // "pair a loop with a literal" discipline both files use.
        assert_eq!(checked, 32, "expected exactly 32 Irreversible rows");
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
            id_field: &["id"],
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
