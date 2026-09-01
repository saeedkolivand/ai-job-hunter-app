use std::collections::HashSet;

use serde_json::json;

use super::*;

// ── eligible_subset ──────────────────────────────────────────────────────────

fn item(id: &str, title: &str) -> Value {
    json!({ "id": id, "title": title, "company": "Acme", "description": "text" })
}

#[test]
fn eligible_subset_with_no_allowlist_returns_everything() {
    let items = vec![item("a", "A"), item("b", "B")];
    let rows = eligible_subset(&items, None);
    assert_eq!(rows.len(), 2);
}

#[test]
fn eligible_subset_with_empty_allowlist_returns_everything() {
    // The empty case is treated as "no filter", per the wire contract's own
    // "Absent/empty ranks the whole live cache" doc.
    let items = vec![item("a", "A"), item("b", "B")];
    let rows = eligible_subset(&items, Some(&[]));
    assert_eq!(rows.len(), 2);
}

#[test]
fn eligible_subset_filters_to_the_allowlist() {
    let items = vec![item("a", "A"), item("b", "B"), item("c", "C")];
    let rows = eligible_subset(&items, Some(&["b".to_string()]));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "b");
}

#[test]
fn eligible_subset_ignores_an_id_absent_from_the_live_cache() {
    // A renderer-supplied allowlist id that names a posting the cache does
    // NOT have (stale UI state, or a hostile caller probing for a cleared
    // corpus) must never be trusted into existence.
    let items = vec![item("a", "A")];
    let rows = eligible_subset(
        &items,
        Some(&["a".to_string(), "does-not-exist".to_string()]),
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "a");
}

#[test]
fn eligible_subset_is_empty_when_every_allowlisted_id_is_absent_from_the_cache() {
    // The direct precursor to `run_search`'s `corpus_size == 0` branch: a
    // NON-empty allowlist that names nothing the live cache still has must
    // degrade to an empty corpus — never silently fall back to "no filter"
    // (that fallback is reserved for an absent/empty `eligible_ids`, per
    // `eligible_subset_with_empty_allowlist_returns_everything` above).
    let items = vec![item("a", "A"), item("b", "B")];
    let rows = eligible_subset(
        &items,
        Some(&[
            "does-not-exist-1".to_string(),
            "does-not-exist-2".to_string(),
        ]),
    );
    assert!(
        rows.is_empty(),
        "an allowlist matching nothing in the cache must produce an empty corpus, not the whole cache"
    );
}

#[test]
fn to_posting_row_requires_a_string_id() {
    assert!(to_posting_row(&json!({"title": "no id here"})).is_none());
    assert!(
        to_posting_row(&json!({"id": 123})).is_none(),
        "a non-string id must not coerce"
    );
}

#[test]
fn to_posting_row_defaults_missing_optional_fields_to_empty() {
    let row = to_posting_row(&json!({"id": "x"})).expect("id alone must still parse");
    assert_eq!(row.title, "");
    assert_eq!(row.description, "");
}

// ── merge_rerank_output ──────────────────────────────────────────────────────

fn ids(strs: &[&str]) -> Vec<String> {
    strs.iter().map(|s| s.to_string()).collect()
}

#[test]
fn merge_rerank_output_keeps_a_clean_full_permutation() {
    let known: HashSet<&str> = ["a", "b", "c"].into_iter().collect();
    let fused = ids(&["a", "b", "c"]);
    let reranked = ids(&["c", "a", "b"]);
    assert_eq!(
        merge_rerank_output(reranked, &fused, &known),
        ids(&["c", "a", "b"])
    );
}

#[test]
fn merge_rerank_output_drops_an_invented_id() {
    let known: HashSet<&str> = ["a", "b"].into_iter().collect();
    let fused = ids(&["a", "b"]);
    let reranked = ids(&["invented", "a", "b"]);
    assert_eq!(
        merge_rerank_output(reranked, &fused, &known),
        ids(&["a", "b"]),
        "an id outside `known` must never surface as a result"
    );
}

#[test]
fn merge_rerank_output_collapses_a_duplicate_to_its_first_occurrence() {
    let known: HashSet<&str> = ["a", "b"].into_iter().collect();
    let fused = ids(&["a", "b"]);
    let reranked = ids(&["a", "a", "b"]);
    assert_eq!(
        merge_rerank_output(reranked, &fused, &known),
        ids(&["a", "b"])
    );
}

#[test]
fn merge_rerank_output_appends_an_omitted_candidate_in_fused_order() {
    let known: HashSet<&str> = ["a", "b", "c"].into_iter().collect();
    let fused = ids(&["a", "b", "c"]);
    // The model only ranked "b"; "a" and "c" were silently dropped.
    let reranked = ids(&["b"]);
    assert_eq!(
        merge_rerank_output(reranked, &fused, &known),
        ids(&["b", "a", "c"]),
        "an omitted candidate must still appear, in its fused-order position"
    );
}

#[test]
fn merge_rerank_output_on_a_completely_empty_response_falls_back_to_fused_order() {
    let known: HashSet<&str> = ["a", "b"].into_iter().collect();
    let fused = ids(&["a", "b"]);
    assert_eq!(merge_rerank_output(Vec::new(), &fused, &known), fused);
}

#[test]
fn merge_rerank_output_handles_invented_duplicate_and_omitted_together() {
    // A realistic messy model response, not three isolated defects: "ghost"
    // was never a candidate, "a" is repeated, and "c" is never mentioned at
    // all. Composing all three in one call catches an interaction the
    // isolated tests above cannot — e.g. an invented id accidentally
    // consuming a `seen` slot that should have been left for its real
    // candidate.
    let known: HashSet<&str> = ["a", "b", "c"].into_iter().collect();
    let fused = ids(&["a", "b", "c"]);
    let reranked = ids(&["b", "ghost", "a", "a"]);
    assert_eq!(
        merge_rerank_output(reranked, &fused, &known),
        ids(&["b", "a", "c"]),
        "invented id dropped, duplicate collapsed to its first occurrence, omitted id appended in fused order"
    );
}

// ── dense_candidate_pool ─────────────────────────────────────────────────────

fn row(id: &str) -> PostingRow {
    PostingRow {
        id: id.to_string(),
        title: String::new(),
        company: String::new(),
        location: String::new(),
        description: String::new(),
    }
}

#[test]
fn dense_candidate_pool_uses_lexical_order_when_lexical_found_something() {
    let eligible = vec![row("a"), row("b"), row("c")];
    let lexical = ids(&["c", "a"]);
    assert_eq!(dense_candidate_pool(&eligible, &lexical), vec!["c", "a"]);
}

#[test]
fn dense_candidate_pool_falls_back_to_cache_order_when_lexical_found_nothing() {
    // Regression: the first version of this fallback read
    // `eligible_by_id.keys()` — a HashMap, whose iteration order is
    // UNSPECIFIED — instead of the ordered `eligible` slice. Many distinct
    // ids make a HashMap-order bug likely to show up as a shuffled result
    // on at least one run; a small fixture could pass by chance.
    let ids_in_order: Vec<String> = (0..20).map(|i| format!("p{i}")).collect();
    let eligible: Vec<PostingRow> = ids_in_order.iter().map(|id| row(id)).collect();
    let pool = dense_candidate_pool(&eligible, &[]);
    let expected: Vec<&str> = ids_in_order.iter().map(String::as_str).collect();
    assert_eq!(
        pool, expected,
        "the empty-lexical fallback must preserve cache order"
    );
}

#[test]
fn dense_candidate_pool_is_bounded_by_dense_candidate_max() {
    let eligible: Vec<PostingRow> = (0..(DENSE_CANDIDATE_MAX + 10))
        .map(|i| row(&i.to_string()))
        .collect();
    assert_eq!(
        dense_candidate_pool(&eligible, &[]).len(),
        DENSE_CANDIDATE_MAX
    );
}

// ── degraded(): the empty-hits contract every non-success return funnels
//    through ────────────────────────────────────────────────────────────────
//
// `run_search`, `run_dense_arm` and `maybe_rerank` themselves take
// `app: &AppHandle` and read `PostingsCache`/`DocumentStore`/`Limiter`/
// `JobPreferencesStore` state directly off it (unlike `commands::autopilot`'s
// `semantic_rerank`/`semantic_rerank_phase`, which are AppHandle-free and take
// a `RerankEnv` fake) — so driving their live orchestration end-to-end needs a
// mock `AppHandle`, which this crate has no harness for by deliberate choice
// (`extension_bridge::test::spawn_detached_runs_without_an_ambient_tokio_runtime`'s
// doc comment, and the `research_answer_tests`/`reembed_tests`/
// `embedding_base_url_tests` notes in `commands::system::test`, all defer the
// same class of test for the same reason). What IS pure and callable here is
// `degraded()` — the single choke point every cancelled/stale-corpus/
// empty-corpus return path funnels through — so its "hits is always empty"
// contract is pinned directly, independent of the AppHandle-bound callers
// that invoke it.
#[test]
fn degraded_never_returns_hits_regardless_of_outcome() {
    let arms = SearchArms {
        lexical: ArmStatus::Ran,
        dense: ArmStatus::Unavailable,
        rerank: ArmStatus::Skipped,
    };
    for outcome in [
        SearchOutcome::Cancelled,
        SearchOutcome::StaleCorpus,
        SearchOutcome::Ok,
    ] {
        let result = degraded(outcome, arms.clone(), 42).expect("degraded never errors");
        assert_eq!(
            result.hits,
            Vec::<String>::new(),
            "{outcome:?} must report zero hits — that is the entire point of `degraded`"
        );
        assert_eq!(result.outcome, outcome);
        assert_eq!(result.corpus_size, 42);
    }
}

// ── wire contract: ArmStatus / SearchOutcome / HybridSearchResult ───────────
//
// "Arm reporting is the product contract" (module doc): the renderer decides
// what to tell the user — "keyword results; semantic ranking unavailable" vs.
// a plain list — purely from these strings. Nothing here exercises
// `run_search`'s orchestration (see the note above `degraded_never_returns_
// hits_regardless_of_outcome`), but a rename, a re-ordered variant, or a
// dropped `#[serde(rename_all = "camelCase")]` on any of these types breaks
// the renderer's status parsing with NO compiler error on either side of the
// IPC boundary — so the wire shape itself is worth pinning against literal
// strings, independent of whatever produces it.
#[test]
fn arm_status_serializes_to_the_documented_wire_strings() {
    assert_eq!(serde_json::to_value(ArmStatus::Ran).unwrap(), json!("ran"));
    assert_eq!(
        serde_json::to_value(ArmStatus::Skipped).unwrap(),
        json!("skipped")
    );
    assert_eq!(
        serde_json::to_value(ArmStatus::Unavailable).unwrap(),
        json!("unavailable")
    );
}

#[test]
fn search_outcome_serializes_to_the_documented_wire_strings() {
    assert_eq!(
        serde_json::to_value(SearchOutcome::Ok).unwrap(),
        json!("ok")
    );
    assert_eq!(
        serde_json::to_value(SearchOutcome::Cancelled).unwrap(),
        json!("cancelled")
    );
    assert_eq!(
        serde_json::to_value(SearchOutcome::StaleCorpus).unwrap(),
        json!("staleCorpus")
    );
}

#[test]
fn hybrid_search_result_serializes_with_camel_case_fields() {
    let result = HybridSearchResult {
        outcome: SearchOutcome::Ok,
        hits: ids(&["a", "b"]),
        arms: SearchArms {
            lexical: ArmStatus::Ran,
            dense: ArmStatus::Skipped,
            rerank: ArmStatus::Unavailable,
        },
        corpus_size: 7,
    };
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        json!({
            "outcome": "ok",
            "hits": ["a", "b"],
            "arms": { "lexical": "ran", "dense": "skipped", "rerank": "unavailable" },
            "corpusSize": 7,
        }),
        "the renderer reads these exact camelCase keys/values off the wire"
    );
}
