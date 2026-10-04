use serde_json::json;

use super::super::tests::ids;
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

// ── run_lexical_arm ──────────────────────────────────────────────────────────

fn lexical_doc<'a>(id: &'a str, title: &'a str, description: &'a str) -> LexicalDoc<'a> {
    LexicalDoc {
        id,
        title,
        company: "",
        location: "",
        description,
    }
}

#[test]
fn run_lexical_arm_reports_ran_on_a_clean_query() {
    let docs = vec![lexical_doc("p1", "Engineer", "Builds things with Rust.")];
    let (ranks, status) = run_lexical_arm(&docs, "rust", 10);
    assert_eq!(status, ArmStatus::Ran);
    assert_eq!(ranks, vec!["p1".to_string()]);
}

/// The regression this PR fixes: a genuine FTS5 failure must report
/// `Unavailable`, never `Ran`-with-empty-hits — the two used to be
/// indistinguishable to the renderer, contradicting the module's own
/// "degrade, never silently claim more than ran" contract. The trigger is
/// the EMPIRICALLY VERIFIED one (an embedded NUL byte — see
/// `retrieval::lexical::LexicalIndex::search`'s doc), not the bare-
/// punctuation claim from the original review, which did not reproduce.
#[test]
fn run_lexical_arm_reports_unavailable_on_a_real_fts5_failure_not_ran() {
    let docs = vec![lexical_doc("p1", "Engineer", "Some text.")];
    let (ranks, status) = run_lexical_arm(&docs, "\0", 10);
    assert_eq!(
        status,
        ArmStatus::Unavailable,
        "a genuine FTS5 failure must never report as Ran"
    );
    assert!(ranks.is_empty());
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

// ── dense_pair ───────────────────────────────────────────────────────────────
//
// Mutation-checked by hand (verified, then reverted before landing): deleting
// the `if candidate.space != *query_space { return None; }` guard reddens
// `dense_pair_refuses_to_score_across_embedding_spaces` below (it would then
// return `Some` for two different-space vectors) while every dimension/value
// test stays green — proof the space check is load-bearing, not decorative.

fn embedding_space(
    provider: &str,
    model: &str,
    dim: usize,
) -> crate::commands::ai_provider::EmbeddingSpace {
    crate::commands::ai_provider::EmbeddingSpace {
        provider: provider.to_string(),
        model: model.to_string(),
        dim,
        version: crate::commands::ai_provider::EMBEDDING_VECTOR_VERSION,
    }
}

fn embedding_vector(
    values: Vec<f64>,
    space: crate::commands::ai_provider::EmbeddingSpace,
) -> crate::commands::ai_provider::EmbeddingVector {
    crate::commands::ai_provider::EmbeddingVector { values, space }
}

#[test]
fn dense_pair_scores_a_candidate_sharing_the_query_space() {
    let space = embedding_space("ollama", "qwen3-embedding:4b", 3);
    let query_space = space.clone();
    let candidate = embedding_vector(vec![1.0, 2.0, 3.0], space);
    let pair = dense_pair("p1", &query_space, &candidate);
    assert_eq!(pair, Some(("p1".to_string(), vec![1.0f32, 2.0, 3.0])));
}

#[test]
fn dense_pair_refuses_to_score_across_embedding_spaces() {
    // Same dimension, different provider — a cosine over these two would be
    // a numerically plausible value that means nothing at all
    // (`commands::ai_provider::compare`'s own rule: "incomparable vectors are
    // never silently scored").
    let query_space = embedding_space("ollama", "qwen3-embedding:4b", 768);
    let other_space = embedding_space("openai", "text-embedding-3-small", 768);
    let candidate = embedding_vector(vec![0.1; 768], other_space);
    assert_eq!(
        dense_pair("p1", &query_space, &candidate),
        None,
        "a candidate from a DIFFERENT embedding space must never be scored, \
         even at an equal dimension"
    );
}

#[test]
fn dense_pair_refuses_a_dimension_mismatch_too() {
    let query_space = embedding_space("ollama", "model-a", 768);
    let candidate = embedding_vector(vec![0.1; 384], embedding_space("ollama", "model-a", 384));
    assert_eq!(dense_pair("p1", &query_space, &candidate), None);
}

// ── dense_candidate_pool: synonym-gap alias exclusion (PR #1091 review) ─────
//
// Moved here from `tests/lexical_synonym_gaps.rs`, which originally asserted
// this against a HAND-MIRRORED copy of `dense_candidate_pool`'s logic — that
// function is private to this module, so an external integration test could
// not call it directly. Flagged Major (a mirror has no seam to check it
// against the real function, so it can drift silently). This version calls
// the REAL `dense_candidate_pool`, with real `PostingRow`s, via the
// `use super::*;` this test module already has. `tests/lexical_synonym_gaps.rs`
// keeps the BM25 miss-count measurement (row 1) and points here for this half.

/// A COPY of `documents::keywords::SYNONYMS`, kept in lockstep with the
/// canonical frozen copy in `tests/lexical_synonym_gaps.rs`'s own
/// `SYNONYM_PAIRS` (that file owns the BM25 miss-count measurement; this one
/// owns the dense-candidate-pool exclusion measurement, and needs the exact
/// same 24 pairs so both suites measure the same claim).
///
/// Deliberately NOT read off `crate::documents::keywords::SYNONYMS` despite
/// living in the same crate: that table is scoring data pinned to
/// `MATCH_FORMULA_VERSION` (see its own doc comment), and this suite must
/// never be able to move it by editing its own fixture list.
/// [`pool_synonym_pairs_match_the_live_table`] guards this copy against
/// drift from the live table, independently of the integration test's own
/// guard over its copy.
const POOL_SYNONYM_PAIRS: &[(&str, &str)] = &[
    ("js", "javascript"),
    ("ts", "typescript"),
    ("py", "python"),
    ("golang", "go"),
    ("k8s", "kubernetes"),
    ("kube", "kubernetes"),
    ("node", "nodejs"),
    ("react.js", "react"),
    ("vue.js", "vue"),
    ("next.js", "nextjs"),
    ("nuxt.js", "nuxtjs"),
    ("psql", "postgresql"),
    ("postgres", "postgresql"),
    ("mongo", "mongodb"),
    ("tf", "tensorflow"),
    ("sklearn", "scikit-learn"),
    ("scikit", "scikit-learn"),
    ("ci/cd", "cicd"),
    ("c/c++", "cpp"),
    ("c++", "cpp"),
    ("objective-c", "objectivec"),
    ("llms", "llm"),
    ("genai", "generativeai"),
    ("gen-ai", "generativeai"),
];

#[test]
fn pool_synonym_pairs_match_the_live_table() {
    assert_eq!(
        POOL_SYNONYM_PAIRS,
        crate::documents::keywords::SYNONYMS,
        "documents::keywords::SYNONYMS changed — resync POOL_SYNONYM_PAIRS above (and the \
         independent copy in tests/lexical_synonym_gaps.rs) by hand"
    );
}

/// Alias-only posting exclusion from the dense candidate pool once a
/// distractor containing the canonical term exists, over the REAL
/// `dense_candidate_pool` and REAL `PostingRow`s. Uses [`run_lexical_arm`]
/// — the same pure lexical-arm entry point `run_search` calls — to get real
/// lexical ranks over a 2-posting corpus, then feeds those ranks straight
/// into `dense_candidate_pool` alongside the full eligible set. No
/// embeddings, no `AppHandle`: the pool selection is pure.
#[test]
fn dense_candidate_pool_excludes_alias_only_posting_when_a_distractor_hits() {
    let mut skipped_because_lexical_found_the_alias: Vec<&str> = Vec::new();

    for &(alias, canonical) in POOL_SYNONYM_PAIRS {
        let alias_row = PostingRow {
            description: alias.to_string(),
            ..row("alias")
        };
        let distractor_row = PostingRow {
            // The canonical term lives in the title (BM25's highest-weighted
            // column) so the distractor is unambiguously the strongest
            // lexical hit — the point under test is exclusion from the
            // POOL, not a close ranking call.
            title: canonical.to_string(),
            description: "Distractor posting mentioning the canonical term.".to_string(),
            ..row("distractor")
        };
        let eligible = vec![alias_row, distractor_row];
        let docs: Vec<LexicalDoc<'_>> = eligible.iter().map(to_lexical_doc).collect();
        let (lexical_ranks, status) = run_lexical_arm(&docs, canonical, 10);
        assert_eq!(status, ArmStatus::Ran, "{alias}: lexical arm did not run");

        if lexical_ranks.iter().any(|id| id == "alias") {
            // The two pairs `tests/lexical_synonym_gaps.rs` measures as
            // lexical HITS (react.js/vue.js — unicode61 splits on '.') would
            // trivially "pass" an exclusion assertion for the wrong reason:
            // lexical already found the alias, so there is nothing for the
            // pool policy to exclude. Skip them explicitly instead of
            // asserting something meaningless, and check the skip set below
            // is EXACTLY those two.
            skipped_because_lexical_found_the_alias.push(alias);
            continue;
        }
        assert!(
            lexical_ranks.iter().any(|id| id == "distractor"),
            "{alias}: distractor was not found lexically for {canonical:?} — fixture is broken, \
             this pair cannot test pool exclusion"
        );

        let pool = dense_candidate_pool(&eligible, &lexical_ranks);
        assert!(
            !pool.contains(&"alias"),
            "{alias}: alias-only posting appeared in the dense candidate pool even though it is \
             part of the eligible corpus and lexical only found the distractor — the \
             pool-exclusion property (ADR-039) did not hold for this pair"
        );
        assert!(
            pool.contains(&"distractor"),
            "{alias}: distractor unexpectedly absent from its own candidate pool"
        );
    }

    assert_eq!(
        skipped_because_lexical_found_the_alias,
        vec!["react.js", "vue.js"],
        "expected exactly react.js/vue.js to be skipped as lexical hits (see \
         tests/lexical_synonym_gaps.rs's own measurement); a different skip set means the two \
         suites' measurements disagree"
    );
}
