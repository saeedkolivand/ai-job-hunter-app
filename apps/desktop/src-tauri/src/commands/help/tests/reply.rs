use super::*;

// ── Fusion + reply assembly ──────────────────────────────────────────────────

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn the_reply_is_truncated_to_the_requested_limit() {
    let result = assemble(
        (ids(&["a", "b", "c", "d"]), ArmStatus::Ran),
        (Vec::new(), ArmStatus::Skipped),
        2,
    );
    assert_eq!(result.results.len(), 2);
    assert_eq!(result.results[0].id, "a");
    assert_eq!(result.results[1].id, "b");
}

#[test]
fn an_entry_both_arms_found_outranks_one_only_the_lexical_arm_found() {
    // The whole point of fusing: `b` is only SECOND in the lexical list, but
    // it is the one entry both arms surfaced, so it must come out on top of
    // `a` (lexical rank 1, absent from the dense list). An implementation
    // that concatenated the arms, or that let one arm win outright, would
    // return `a` first.
    let result = assemble(
        (ids(&["a", "b"]), ArmStatus::Ran),
        (ids(&["b", "c"]), ArmStatus::Ran),
        3,
    );
    let order: Vec<&str> = result.results.iter().map(|h| h.id.as_str()).collect();
    assert_eq!(order, vec!["b", "a", "c"], "scores: {:?}", result.results);
    let scores: Vec<f64> = result.results.iter().map(|h| h.score).collect();
    assert!(
        scores.windows(2).all(|w| w[0] >= w[1]),
        "results must be best-first: {scores:?}"
    );
    // The score is RRF's, not a rank index or a BM25/cosine value: an id in
    // both lists scores 1/(60+1) + 1/(60+2), an id in one scores 1/(60+1).
    let expected_b = 1.0 / (fusion::RRF_K + 2.0) + 1.0 / (fusion::RRF_K + 1.0);
    assert!(
        (result.results[0].score - expected_b).abs() < 1e-12,
        "expected the RRF score {expected_b}, got {}",
        result.results[0].score
    );
}

#[test]
fn keyword_results_still_come_back_when_the_dense_arm_is_unavailable() {
    let result = assemble(
        (ids(&["a", "b"]), ArmStatus::Ran),
        (Vec::new(), ArmStatus::Unavailable),
        3,
    );
    assert_eq!(
        result.results.len(),
        2,
        "an embedding failure must not empty the reply"
    );
    assert_eq!(result.mode, HelpSearchMode::Keyword);
    assert_eq!(result.arms.dense, ArmStatus::Unavailable);
    assert_eq!(result.arms.lexical, ArmStatus::Ran);
}

#[test]
fn mode_is_hybrid_only_when_the_dense_arm_actually_ran() {
    assert_eq!(mode_of(ArmStatus::Ran), HelpSearchMode::Hybrid);
    assert_eq!(
        mode_of(ArmStatus::Skipped),
        HelpSearchMode::Keyword,
        "the preference being off is keyword results, not hybrid ones"
    );
    assert_eq!(
        mode_of(ArmStatus::Unavailable),
        HelpSearchMode::Keyword,
        "an embedding failure is keyword results, not hybrid ones"
    );
}

#[test]
fn help_arm_statuses_serialize_as_the_wire_contract_tags() {
    // `ArmStatus` is shared with `hybrid_search`, so pin the exact tags
    // `HelpSearchResultSchema`'s `z.enum`s declare — a variant added or
    // renamed over there would otherwise widen this command's wire contract
    // silently.
    let json = serde_json::to_value(assemble(
        (ids(&["a"]), ArmStatus::Ran),
        (Vec::new(), ArmStatus::Skipped),
        1,
    ))
    .unwrap();
    assert_eq!(json["mode"], "keyword");
    assert_eq!(json["arms"]["lexical"], "ran");
    assert_eq!(json["arms"]["dense"], "skipped");
    assert_eq!(json["results"][0]["id"], "a");
    assert!(json["results"][0]["score"].is_number());

    let hybrid = serde_json::to_value(assemble(
        (ids(&["a"]), ArmStatus::Ran),
        (ids(&["a"]), ArmStatus::Ran),
        1,
    ))
    .unwrap();
    assert_eq!(hybrid["mode"], "hybrid");
    let unavailable = serde_json::to_value(assemble(
        (Vec::new(), ArmStatus::Unavailable),
        (Vec::new(), ArmStatus::Unavailable),
        1,
    ))
    .unwrap();
    assert_eq!(unavailable["arms"]["lexical"], "unavailable");
    assert_eq!(unavailable["arms"]["dense"], "unavailable");
}
