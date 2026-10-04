use super::*;

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
