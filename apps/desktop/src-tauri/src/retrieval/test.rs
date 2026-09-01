use super::dense::{cosine, rank_by_similarity};
use super::fusion::reciprocal_rank_fusion;
use super::lexical::{LexicalDoc, LexicalIndex};

// ── dense ────────────────────────────────────────────────────────────────────

#[test]
fn cosine_of_identical_vectors_is_one() {
    let v = vec![1.0, 2.0, 3.0];
    let got = cosine(&v, &v).expect("non-degenerate vectors must score");
    assert!((got - 1.0).abs() < 1e-6, "got {got}");
}

#[test]
fn cosine_of_orthogonal_vectors_is_zero() {
    let a = vec![1.0, 0.0];
    let b = vec![0.0, 1.0];
    let got = cosine(&a, &b).expect("non-degenerate vectors must score");
    assert!(got.abs() < 1e-6, "got {got}");
}

#[test]
fn cosine_rejects_dimension_mismatch_and_zero_vectors() {
    assert!(
        cosine(&[1.0, 2.0], &[1.0]).is_none(),
        "mismatched dims must not score"
    );
    assert!(
        cosine(&[0.0, 0.0], &[1.0, 1.0]).is_none(),
        "a zero vector has no direction"
    );
    assert!(cosine(&[], &[]).is_none(), "empty vectors must not score");
}

#[test]
fn rank_by_similarity_orders_best_first_and_drops_unscorable() {
    let query = vec![1.0, 0.0];
    let candidates = vec![
        ("far".to_string(), vec![0.0, 1.0]),  // orthogonal -> 0.0
        ("near".to_string(), vec![1.0, 0.1]), // close -> high
        ("bad_dim".to_string(), vec![1.0]),   // dropped: dimension mismatch
    ];
    let ranked = rank_by_similarity(&query, &candidates);
    assert_eq!(ranked, vec!["near".to_string(), "far".to_string()]);
}

// ── fusion ───────────────────────────────────────────────────────────────────

#[test]
fn rrf_favors_a_doc_ranked_first_in_both_lists() {
    let lexical = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let dense = vec!["a".to_string(), "c".to_string(), "b".to_string()];
    let fused = reciprocal_rank_fusion(&[lexical, dense]);
    assert_eq!(
        fused[0].0, "a",
        "ranked #1 in both lists must fuse to the top"
    );
}

#[test]
fn rrf_degrades_to_the_only_arm_that_ran() {
    let lexical = vec!["x".to_string(), "y".to_string()];
    let empty: Vec<String> = Vec::new();
    let fused = reciprocal_rank_fusion(&[lexical.clone(), empty]);
    let ids: Vec<&str> = fused.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["x", "y"],
        "an empty (skipped/unavailable) arm must contribute nothing"
    );
}

#[test]
fn rrf_an_id_in_only_one_list_still_surfaces() {
    let lexical = vec!["only_lexical".to_string(), "shared".to_string()];
    let dense = vec!["shared".to_string()];
    let fused = reciprocal_rank_fusion(&[lexical, dense]);
    assert!(
        fused.iter().any(|(id, _)| id == "only_lexical"),
        "a lexical-only hit must not be dropped by fusion"
    );
    // "shared" appears in both lists so it must outrank the lexical-only hit.
    assert_eq!(fused[0].0, "shared");
}

// ── lexical ──────────────────────────────────────────────────────────────────

fn doc<'a>(id: &'a str, title: &'a str, company: &'a str, description: &'a str) -> LexicalDoc<'a> {
    LexicalDoc {
        id,
        title,
        company,
        location: "",
        description,
    }
}

#[test]
fn lexical_search_finds_a_matching_posting() {
    let docs = vec![
        doc(
            "p1",
            "Senior Rust Engineer",
            "Acme",
            "Build backend services in Rust.",
        ),
        doc("p2", "Frontend Designer", "Beta", "React and CSS all day."),
    ];
    let index = LexicalIndex::build(&docs).expect("build must succeed");
    let hits = index
        .search("rust", 10)
        .expect("a clean query must not error");
    assert_eq!(hits, vec!["p1".to_string()]);
}

#[test]
fn lexical_title_hit_outranks_a_description_only_hit() {
    // "rust" is in p1's TITLE; p2 only mentions it once, deep in the description.
    let docs = vec![
        doc(
            "p2",
            "Frontend Designer",
            "Beta",
            "Nice to have: has touched rust once at a hackathon.",
        ),
        doc("p1", "Rust Engineer", "Acme", "General backend work."),
    ];
    let index = LexicalIndex::build(&docs).expect("build must succeed");
    let hits = index
        .search("rust", 10)
        .expect("a clean query must not error");
    assert_eq!(
        hits.first().map(String::as_str),
        Some("p1"),
        "a title hit must rank first"
    );
}

#[test]
fn lexical_search_treats_special_characters_as_literal_text_not_fts5_operators() {
    let docs = vec![
        doc(
            "p1",
            "Engineer",
            "Acme",
            "Experience with golang and Kubernetes.",
        ),
        doc("p2", "Designer", "Beta", "No backend experience."),
    ];
    let index = LexicalIndex::build(&docs).expect("build must succeed");

    // If "-golang" were parsed as FTS5's NOT operator, this would EXCLUDE p1
    // (the only doc mentioning "golang") — a `let _ =` smoke test can't tell
    // "returned nothing because it's a NOT query" from "returned nothing
    // because sanitizing broke". Sanitized as a literal phrase, it finds p1.
    assert_eq!(
        index.search("-golang", 10).expect("must not error"),
        vec!["p1".to_string()],
        "a leading `-` must be literal text, not FTS5's NOT operator"
    );

    // If "title:x" were parsed as an FTS5 column filter, it would be a
    // syntactically valid (if matchless) query; sanitized as a literal
    // phrase, no document contains the literal string "title:x" either way —
    // this only proves it didn't ERROR, paired with the assertion above which
    // proves operators are actually neutralized.
    assert!(
        index
            .search("title:x", 10)
            .expect("must not error")
            .is_empty(),
        "a `column:` prefix must not error and must not match anything here"
    );

    // Must not ERROR on the remaining operator-shaped input either —
    // empirically verified (see `LexicalIndex::search`'s own doc): every
    // character FTS5's grammar assigns meaning to is neutralized by
    // `sanitize_query`'s quoting.
    for query in ["\"unterminated", "NEAR", "*", ":", "()", "AND", "NOT"] {
        index
            .search(query, 10)
            .unwrap_or_else(|e| panic!("{query:?} must not error, got {e}"));
    }
}

#[test]
fn lexical_search_on_empty_query_returns_no_hits() {
    let docs = vec![doc("p1", "Engineer", "Acme", "Go.")];
    let index = LexicalIndex::build(&docs).expect("build must succeed");
    assert!(index.search("   ", 10).expect("must not error").is_empty());
}

/// The genuine, empirically-verified trigger (a security-review claim that a
/// bare punctuation query like `-`/`!!!` breaks this did NOT reproduce — see
/// `LexicalIndex::search`'s own doc): an embedded NUL byte reaches FTS5's
/// query-expression parser as a string it reads as truncated and fails with
/// a real `rusqlite` error on the first row fetch, which must propagate as
/// `Err`, never silently collapse to an empty `Ok(vec![])` — a real failure
/// and a genuine zero-match result must stay distinguishable to the caller.
#[test]
fn lexical_search_propagates_a_real_fts5_error_instead_of_swallowing_it() {
    let docs = vec![doc("p1", "Engineer", "Acme", "Some text here.")];
    let index = LexicalIndex::build(&docs).expect("build must succeed");
    assert!(
        index.search("\0", 10).is_err(),
        "an embedded NUL byte must surface as Err, not a silent empty Ok"
    );
}
