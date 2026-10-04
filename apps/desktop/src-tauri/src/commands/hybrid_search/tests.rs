use serde_json::json;

use super::*;

/// Shared fixture: `&[&str]` as owned ids (also used by the arm modules' tests).
pub(super) fn ids(strs: &[&str]) -> Vec<String> {
    strs.iter().map(|s| s.to_string()).collect()
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
// `embedding_base_url_tests` notes in `commands::system::tests`, all defer the
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
