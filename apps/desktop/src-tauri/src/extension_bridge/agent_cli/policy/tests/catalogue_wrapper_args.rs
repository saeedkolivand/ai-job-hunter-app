//! The generator's two hand-written mirrors of how catalogue args are WRAPPED — the
//! unresolved `fields`/`location` keys and the resolved scalar args behind them.

/// Hand-written mirror of every catalogued arg whose `fields` is `Some(&[])` — a wrapper TYPE
/// this generator identified but could not resolve the field names of (MEDIUM — CLI review
/// round 1, issue #1158's "guess the wrapper" gap). `mcp.rs` surfaces these on the wire as
/// `"fields": null`, distinct from omitting the key, so a caller can at least tell "unknown
/// nested shape" apart from "no nested shape" — but nothing accounted for the CLASS itself. A
/// generator regression that silently reclassified a RESOLVED wrapper (e.g. a schema rename
/// dropping out of `schemas/index.ts`) as unresolved would leave every other test green while
/// quietly widening the set of commands a nested-key typo can sail past `agent_call::validate`
/// on. `(command, arg name)` pairs, pinned the same way [`EXPECTED_UNCATALOGUED`] is.
///
/// `ai_clear_stage_override`/`ai_set_stage_override`'s `stage` used to be pinned HERE
/// (A1-r1-AC-3 MEDIUM): `PipelineStage` is a scalar string-union alias
/// (`packages/shared/src/events/pipeline.ts`), not an object wrapper, so the generator's OWN
/// unresolved-named-type fallback was publishing it as `"fields": null` — the exact "this takes a
/// nested object" signal that field means. `gen-agent-catalogue.ts`'s `collectScalarTypeAliasNames`
/// now proves that shape and emits `fields: None` for both rows instead.
const EXPECTED_UNRESOLVED_WRAPPER_ARGS: &[(&str, &str)] = &[
    ("ai_set_provider_settings", "req"),
    ("autopilot_update", "req"),
    // The five below (A1-r1-AC-1 MEDIUM) used to fall through `findParamBinding` to
    // `fields: undefined` (a plain scalar) instead of this unresolved-wrapper shape: their `req`/
    // `prefs`/`filter` params are typed `unknown`, an inline object type literal, or
    // `Parameters<Fn>[0]` — none of those are a `TypeReferenceNode` the generator can look a name
    // up for, but every one IS a genuine object wrapper, not a scalar.
    ("job_preferences_set", "prefs"),
    ("resume_pipeline_run", "req"),
    ("scrape_list_interactions", "filter"),
    ("scrape_persist_job", "req"),
    ("scrape_remove_interaction", "req"),
    ("scrape_update_description", "req"),
    ("system_set_performance_mode", "config"),
];

#[test]
fn unresolved_wrapper_args_match_the_hand_written_list() {
    let mut actual: Vec<(&str, &str)> = Vec::new();
    for entry in super::super::super::catalogue::CATALOGUE.iter() {
        for arg in entry.args {
            if arg.fields.is_some_and(|f| f.is_empty()) {
                actual.push((entry.command, arg.name));
            }
        }
    }
    actual.sort_unstable();
    let mut expected = EXPECTED_UNRESOLVED_WRAPPER_ARGS.to_vec();
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "the set of catalogued args with an unresolved wrapper type (`fields: Some(&[])`, wire \
         `\"fields\": null`) drifted from this test's own hand-written list — if a NEW command \
         legitimately can't have its wrapper resolved, add it here deliberately; if one \
         DROPPED OUT (now resolved), remove it from this list"
    );
}

/// Hand-written pin for every catalogued arg with a RESOLVED, non-empty nested-field list
/// (`fields: Some(&["…"])`) — the sibling [`EXPECTED_UNRESOLVED_WRAPPER_ARGS`] only pinned the
/// empty class (A1-r1-AC-2 MEDIUM): nothing caught a generator regression that downgraded one of
/// THESE 32 rows to `fields: None` (a plain scalar — nested-key validation silently disabled for
/// that command) or to `fields: Some(&[])` (mis-labelled unresolved on the wire) — only
/// `applications_save_from_posting` had its own dedicated fixture assertion
/// (`validate/tests.rs`'s `resolved_fields` panic). Same shrink-only discipline as
/// [`EXPECTED_UNRESOLVED_WRAPPER_ARGS`]: a command dropping OUT of this list (its wrapper stopped
/// resolving) fails here; add a NEW resolved wrapper here deliberately, never let one through
/// silently.
const EXPECTED_RESOLVED_WRAPPER_ARGS: &[(&str, &str)] = &[
    ("ai_embed", "req"),
    ("ai_generate", "req"),
    ("ai_generations_save", "req"),
    ("ai_generations_update", "req"),
    ("ai_seed_active_config", "config"),
    ("applications_save_from_posting", "req"),
    ("applications_track", "req"),
    ("applications_update", "req"),
    ("autopilot_create", "req"),
    ("contact_profile_set", "profile"),
    ("dedup_mark_not_duplicate", "req"),
    ("discovery_search_companies", "req"),
    ("discovery_set_starred", "req"),
    ("documents_export_and_save", "request"),
    ("documents_export_document", "request"),
    ("documents_import", "req"),
    ("documents_recommend_template", "req"),
    ("documents_render_preview_images", "request"),
    ("generate_pipeline", "req"),
    ("help_search", "req"),
    ("match_resume", "req"),
    ("match_resume_text", "req"),
    ("privacy_set_crash_reporting", "settings"),
    ("referrals_upsert", "req"),
    ("resume_extract_text", "req"),
    ("resume_pipeline_regenerate_section", "req"),
    ("resume_pipeline_resolve_fabrication", "req"),
    ("resume_trim_suggestions", "req"),
    ("resume_validate_content", "req"),
    ("scrape_boards", "req"),
    ("scrape_hybrid_search", "req"),
    ("scrape_url", "req"),
];

#[test]
fn resolved_wrapper_args_match_the_hand_written_list() {
    let mut actual: Vec<(&str, &str)> = Vec::new();
    for entry in super::super::super::catalogue::CATALOGUE.iter() {
        for arg in entry.args {
            if arg.fields.is_some_and(|f| !f.is_empty()) {
                actual.push((entry.command, arg.name));
            }
        }
    }
    actual.sort_unstable();
    let mut expected = EXPECTED_RESOLVED_WRAPPER_ARGS.to_vec();
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "the set of catalogued args with a RESOLVED nested-field list drifted from this test's \
         own hand-written list — if one DROPPED OUT, a generator regression silently disabled \
         nested-key validation for that command (or mis-labelled it `fields: Some(&[])` on the \
         wire); if a NEW one legitimately resolved, add it here deliberately"
    );
}
