//! Cross-board clustering tests: string/cosine join paths, tombstone vetoes,
//! canonical-preference ordering, and the similarity-threshold boundaries.

use super::*;

fn input(key: &str, title: &str, company: &str, source: &str) -> ClusterInput {
    ClusterInput {
        key: key.to_string(),
        title: title.to_string(),
        company: company.to_string(),
        url: format!("https://example.com/{key}"),
        source: (!source.is_empty()).then(|| source.to_string()),
        has_description: false,
        seen_at: 0,
        vector: None,
        space: None,
    }
}

fn no_tombstones() -> HashSet<(String, String)> {
    HashSet::new()
}

// ── trigram_jaccard ───────────────────────────────────────────────────────

#[test]
fn trigram_jaccard_identical_is_one() {
    assert_eq!(trigram_jaccard("rust developer", "rust developer"), 1.0);
}

#[test]
fn trigram_jaccard_disjoint_is_low() {
    assert!(trigram_jaccard("rust developer", "sales manager") < 0.2);
}

#[test]
fn trigram_jaccard_empty_cases() {
    assert_eq!(trigram_jaccard("", ""), 1.0);
    assert_eq!(trigram_jaccard("x", ""), 0.0);
}

// ── acceptance pair: Acme GmbH / Acme cluster ─────────────────────────────

#[test]
fn acme_gmbh_and_acme_cluster_via_string_path() {
    let items = vec![
        input(
            "k1",
            "Senior Rust Developer (m/w/d) – Berlin",
            "Acme GmbH",
            "greenhouse",
        ),
        input("k2", "Senior Rust Developer", "Acme", "aggregator"),
    ];
    let out = assign_clusters(items, &no_tombstones(), &[]);
    assert_eq!(
        out[0].cluster_id, out[1].cluster_id,
        "both must share a cluster"
    );
    // The direct full-text board (has no aggregator flag) is canonical over
    // the aggregator copy.
    assert_eq!(out[0].cluster_id, "k1");
    assert!(out[0].canonical);
    assert!(!out[1].canonical);
    assert_eq!(out[0].members.len(), 2);
}

// ── Senior vs Junior at the SAME company do NOT cluster ───────────────────

#[test]
fn senior_and_junior_same_company_do_not_cluster() {
    let items = vec![
        input("k1", "Senior Rust Developer", "Acme", ""),
        input("k2", "Junior Rust Developer", "Acme", ""),
    ];
    let out = assign_clusters(items, &no_tombstones(), &[]);
    assert_ne!(
        out[0].cluster_id, out[1].cluster_id,
        "different first-token blocks (senior vs junior) must not merge"
    );
}

// ── cosine path when both vectors present (same space) ────────────────────

#[test]
fn cosine_path_joins_when_both_vectors_same_space() {
    // Titles differ enough that the string path would NOT join (jaccard < .9),
    // but near-identical same-space vectors clear the cosine floor.
    let mut a = input("k1", "Backend Engineer", "Globex", "");
    let mut b = input("k2", "Backend Engineering Specialist", "Globex", "");
    a.vector = Some(vec![1.0, 0.0, 0.0]);
    a.space = Some("ollama/nomic@3".to_string());
    b.vector = Some(vec![0.999, 0.001, 0.0]);
    b.space = Some("ollama/nomic@3".to_string());
    let out = assign_clusters(vec![a, b], &no_tombstones(), &[]);
    assert_eq!(
        out[0].cluster_id, out[1].cluster_id,
        "cosine must join near-identical vectors"
    );
}

#[test]
fn cosine_path_ignored_across_different_spaces_falls_back_to_string() {
    // Same vectors but DIFFERENT spaces → never compared by cosine; the
    // dissimilar titles then keep them apart on the string path.
    let mut a = input("k1", "Backend Engineer", "Globex", "");
    let mut b = input("k2", "Data Platform Architect", "Globex", "");
    a.vector = Some(vec![1.0, 0.0]);
    a.space = Some("ollama/nomic@2".to_string());
    b.vector = Some(vec![1.0, 0.0]);
    b.space = Some("openai/small@2".to_string());
    let out = assign_clusters(vec![a, b], &no_tombstones(), &[]);
    assert_ne!(out[0].cluster_id, out[1].cluster_id);
}

#[test]
fn string_path_used_when_a_vector_is_missing() {
    // One side lacks a vector → string path decides; identical titles join.
    let mut a = input("k1", "Rust Developer", "Globex", "");
    a.vector = Some(vec![1.0, 0.0]);
    a.space = Some("ollama/nomic@2".to_string());
    let b = input("k2", "Rust Developer", "Globex", "");
    let out = assign_clusters(vec![a, b], &no_tombstones(), &[]);
    assert_eq!(out[0].cluster_id, out[1].cluster_id);
}

// ── tombstone veto against ANY member ─────────────────────────────────────

#[test]
fn tombstone_vetoes_join_against_any_member() {
    let items = vec![
        input("k1", "Rust Developer", "Acme", "greenhouse"),
        input("k2", "Rust Developer", "Acme", "lever"),
    ];
    let mut tombstones = HashSet::new();
    tombstones.insert(ordered_pair("k1", "k2"));
    let out = assign_clusters(items, &tombstones, &[]);
    assert_ne!(
        out[0].cluster_id, out[1].cluster_id,
        "a split verdict must keep the two apart despite identical titles"
    );
}

#[test]
fn tombstone_veto_covers_a_third_member_of_the_cluster() {
    // k1 seeds the cluster; k2 joins; k3 is tombstoned only against k2, but
    // the veto is against ANY member, so k3 must NOT join.
    let items = vec![
        input("k1", "Rust Developer", "Acme", "greenhouse"),
        input("k2", "Rust Developer", "Acme", "lever"),
        input("k3", "Rust Developer", "Acme", "aggregator"),
    ];
    let mut tombstones = HashSet::new();
    tombstones.insert(ordered_pair("k2", "k3"));
    let out = assign_clusters(items, &tombstones, &[]);
    assert_eq!(
        out[0].cluster_id, out[1].cluster_id,
        "k1 and k2 still cluster"
    );
    assert_ne!(
        out[2].cluster_id, out[0].cluster_id,
        "k3 vetoed against member k2"
    );
}

// ── canonical preference order ────────────────────────────────────────────

#[test]
fn canonical_prefers_description_then_direct_board() {
    // k_agg has no description + aggregator source; k_dir has a description +
    // direct board → k_dir must be the canonical member.
    let mut k_agg = input("k_agg", "Rust Developer", "Acme", "aggregator");
    k_agg.seen_at = 100; // newer, but description + direct board win first
    let mut k_dir = input("k_dir", "Rust Developer", "Acme", "greenhouse");
    k_dir.has_description = true;
    k_dir.seen_at = 1;
    let out = assign_clusters(vec![k_agg, k_dir], &no_tombstones(), &[]);
    assert_eq!(out[0].cluster_id, "k_dir");
    assert!(
        out[1].canonical,
        "the described direct-board row is canonical"
    );
}

// ── deterministic cluster_id across identical inputs ──────────────────────

#[test]
fn cluster_id_is_deterministic() {
    let build = || {
        vec![
            input("b", "Rust Developer", "Acme", "lever"),
            input("a", "Rust Developer", "Acme", "greenhouse"),
        ]
    };
    let first = assign_clusters(build(), &no_tombstones(), &[]);
    let second = assign_clusters(build(), &no_tombstones(), &[]);
    assert_eq!(first[0].cluster_id, second[0].cluster_id);
    // Tie broken by key asc → "a" is canonical over "b".
    assert_eq!(first[0].cluster_id, "a");
}

// ── new_cluster_count ─────────────────────────────────────────────────────

#[test]
fn new_cluster_count_counts_all_new_clusters_once() {
    // Two boards surface the same NEW job → one cluster, all new → counts 1.
    let items = vec![
        input("k1", "Rust Developer", "Acme", "greenhouse"),
        input("k2", "Rust Developer", "Acme", "aggregator"),
    ];
    let out = assign_clusters(items, &no_tombstones(), &[]);
    let new_keys: HashSet<String> = ["k1", "k2"].iter().map(|s| s.to_string()).collect();
    assert_eq!(new_cluster_count(&out, &new_keys), 1);
}

#[test]
fn new_cluster_count_excludes_member_added_to_known_cluster() {
    // k_known was seen before; k_new is a fresh aggregator copy of it. They
    // cluster, but the cluster is NOT all-new → 0.
    let items = vec![
        input("k_known", "Rust Developer", "Acme", "greenhouse"),
        input("k_new", "Rust Developer", "Acme", "aggregator"),
    ];
    let out = assign_clusters(items, &no_tombstones(), &[]);
    let new_keys: HashSet<String> = ["k_new"].iter().map(|s| s.to_string()).collect();
    assert_eq!(
        new_cluster_count(&out, &new_keys),
        0,
        "a known job resurfacing must not count as a new cluster"
    );
}

// ── threshold boundary pins (catch a value drift OR a `>=`→`>` slip) ──────

#[test]
fn similarity_thresholds_are_pinned() {
    assert_eq!(CLUSTER_COSINE_MIN, 0.92);
    assert_eq!(CLUSTER_TITLE_TRIGRAM_JACCARD_MIN, 0.90);
}

#[test]
fn cosine_join_is_inclusive_at_the_threshold() {
    // `b` is constructed so `cosine([1,0], b)` == CLUSTER_COSINE_MIN EXACTLY
    // (sqrt(1-min²) makes |b| = 1, verified bit-identical in f64). The
    // production join uses `>=`, so this at-threshold pair must be `similar`;
    // a future `>` slip would fail this test.
    let min = CLUSTER_COSINE_MIN;
    let va = vec![1.0, 0.0];
    let vb = vec![min, (1.0 - min * min).sqrt()];
    assert_eq!(
        cosine(&va, &vb),
        min,
        "b must sit exactly on the cosine threshold"
    );
    let mut a = input("k1", "engineer", "co", "");
    let mut b = input("k2", "engineer", "co", "");
    a.vector = Some(va);
    a.space = Some("space".into());
    b.vector = Some(vb);
    b.space = Some("space".into());
    let items = vec![a, b];
    let prepared = [
        Prepared {
            idx: 0,
            norm_title: "engineer".into(),
            block: None,
        },
        Prepared {
            idx: 1,
            norm_title: "engineer".into(),
            block: None,
        },
    ];
    assert!(
        similar(&items, &prepared, 0, 1),
        "cosine exactly at the threshold must join (inclusive `>=`)"
    );
}

#[test]
fn trigram_join_is_inclusive_at_the_threshold() {
    // A pair whose trigram-Jaccard is EXACTLY the threshold: 55 DISTINCT
    // chars → 57 distinct trigrams; replacing the last char shares 54, each
    // side 3 unique → union 60, 54/60 == 0.90 in f64 (verified). Both items
    // lack vectors, forcing the trigram path; the production `>=` must join.
    let base: String = ('a'..='z')
        .chain('A'..='Z')
        .chain('0'..='9')
        .take(55)
        .collect();
    let mut other_chars: Vec<char> = base.chars().collect();
    *other_chars.last_mut().unwrap() = '!';
    let other: String = other_chars.into_iter().collect();
    assert_eq!(
        trigram_jaccard(&base, &other),
        CLUSTER_TITLE_TRIGRAM_JACCARD_MIN,
        "the pair must sit exactly on the trigram threshold"
    );
    let items = vec![input("k1", "x", "co", ""), input("k2", "y", "co", "")];
    let prepared = [
        Prepared {
            idx: 0,
            norm_title: base,
            block: None,
        },
        Prepared {
            idx: 1,
            norm_title: other,
            block: None,
        },
    ];
    assert!(
        similar(&items, &prepared, 0, 1),
        "trigram exactly at the threshold must join (inclusive `>=`)"
    );
}

#[test]
fn empty_company_or_title_is_a_singleton() {
    let items = vec![
        input("k1", "Rust Developer", "", "greenhouse"),
        input("k2", "Rust Developer", "", "lever"),
    ];
    let out = assign_clusters(items, &no_tombstones(), &[]);
    assert_ne!(
        out[0].cluster_id, out[1].cluster_id,
        "empty normalized company forces singletons"
    );
    assert_eq!(out[0].cluster_id, "k1");
    assert_eq!(out[1].cluster_id, "k2");
}
