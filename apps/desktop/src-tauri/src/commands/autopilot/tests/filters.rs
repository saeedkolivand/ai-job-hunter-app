//! The phase-1 keyword filters and the minimum-score gate.

use super::super::keyword_rank::{matches_keyword_filters, passes_min_score};
use super::support::*;
use crate::autopilot::AutopilotFilter;

fn filter(keywords: Option<&[&str]>, exclude: Option<&[&str]>) -> AutopilotFilter {
    AutopilotFilter {
        min_match_score: 0.0,
        keywords: keywords.map(|v| v.iter().map(|s| s.to_string()).collect()),
        exclude_keywords: exclude.map(|v| v.iter().map(|s| s.to_string()).collect()),
    }
}

// The `country_code` save-time derivation tests moved to
// `commands::geocoding` with the helpers themselves (they are now shared
// with the manual scrape path).

#[test]
fn no_filters_keep_everything() {
    let p = posting("Rust Engineer", Some("We use Rust and Go"));
    assert!(matches_keyword_filters(&p, &filter(None, None)));
    // Empty lists are also a no-op.
    assert!(matches_keyword_filters(&p, &filter(Some(&[]), Some(&[]))));
}

#[test]
fn must_include_requires_all_keywords() {
    let p = posting("Rust Engineer", Some("We use Rust and Kubernetes"));
    assert!(matches_keyword_filters(
        &p,
        &filter(Some(&["rust", "kubernetes"]), None)
    ));
    // Missing one required keyword → dropped.
    assert!(!matches_keyword_filters(
        &p,
        &filter(Some(&["rust", "elixir"]), None)
    ));
}

#[test]
fn exclude_drops_on_any_match() {
    let p = posting("Senior PHP Developer", Some("Legacy PHP codebase"));
    assert!(!matches_keyword_filters(&p, &filter(None, Some(&["php"]))));
    assert!(matches_keyword_filters(
        &p,
        &filter(None, Some(&["python"]))
    ));
}

#[test]
fn matching_is_case_insensitive_over_title_and_description() {
    let p = posting("Backend Role", Some("Postgres and REDIS"));
    // "Backend" only in title, "redis" only in description, different cases.
    assert!(matches_keyword_filters(
        &p,
        &filter(Some(&["Backend", "redis"]), None)
    ));
}

// Autopilot now ranks with the shared keyword-coverage kernel
// (`documents::keywords::coverage_score`) — the same embedding-free ATS
// sub-score the Jobs page uses — instead of the deleted Jaccard
// `simple_similarity`. A résumé covering all the JD's keywords scores high; an
// unrelated résumé scores 0; partial overlap lands strictly in between.
#[test]
fn ranking_uses_shared_keyword_coverage_kernel() {
    use crate::documents::keywords::coverage_score;

    // resume = description (all JD keywords covered) → full coverage.
    assert_eq!(
        coverage_score("rust kubernetes docker", "rust kubernetes docker"),
        100.0
    );
    // No overlapping keywords → 0.
    assert_eq!(coverage_score("rust", "java"), 0.0);
    // Résumé covers only part of the JD's keywords → strictly between.
    let partial = coverage_score("rust kubernetes", "rust kubernetes docker terraform");
    assert!(
        partial > 0.0 && partial < 100.0,
        "partial coverage must be strictly between 0 and 100; got {partial}"
    );
}

#[test]
fn min_score_gate_keeps_at_or_above_threshold() {
    assert!(passes_min_score(&found(Some(80.0)), 50.0));
    assert!(passes_min_score(&found(Some(50.0)), 50.0)); // boundary is inclusive
    assert!(!passes_min_score(&found(Some(49.9)), 50.0));
}

#[test]
fn min_score_gate_keeps_unscored_jobs() {
    // No resume / no description → no score → never filtered out by the gate.
    assert!(passes_min_score(&found(None), 50.0));
    assert!(passes_min_score(&found(None), 100.0));
}
