//! Within-block ordering of best-matches rows beyond the score itself.

use super::*;

/// #1497: a description-less row's keyword score is inflated by a title match alone, so it must
/// not float above a described row of the same block even with a higher raw score.
#[test]
fn description_less_rows_sort_below_described_rows_within_a_block() {
    let bare = FoundJob {
        description: None,
        ..job(
            "https://a.example.com/job",
            "A Engineer",
            "AltCo",
            Some(MATCH_TIER_COVERAGE_HIGH + 30.0),
            ScoreSource::Keyword,
        )
    };
    let described = FoundJob {
        description: Some("Build and run services in Rust and Postgres.".to_string()),
        ..job(
            "https://b.example.com/job",
            "B Engineer",
            "BravoCo",
            Some(MATCH_TIER_COVERAGE_HIGH + 5.0),
            ScoreSource::Keyword,
        )
    };
    let out = best(&[autopilot(
        "ap",
        AutopilotStatus::Active,
        vec![bare, described],
    )]);
    assert_eq!(out.matches.len(), 2);
    assert_eq!(out.matches[0].company, "BravoCo", "described row first");
    assert_eq!(out.matches[1].company, "AltCo");
    assert!(
        out.matches[0].score < out.matches[1].score,
        "score untouched"
    );
}
