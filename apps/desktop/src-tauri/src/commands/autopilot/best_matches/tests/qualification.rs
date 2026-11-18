//! Who makes the list and in what order: per-kernel High cuts, the two-block sort, the cap, and the
//! wire shape of a row.

use super::*;

#[test]
fn combined_block_sorts_before_keyword_block_regardless_of_raw_score() {
    let keyword_score = MATCH_TIER_COVERAGE_HIGH + 40.0;
    let combined_score = MATCH_TIER_COMBINED_HIGH + 5.0;
    let hot_keyword = single(
        "hk",
        "https://k.example.com/job",
        "A Engineer",
        "AltCo",
        keyword_score,
    );
    let modest_combined = autopilot(
        "mc",
        AutopilotStatus::Active,
        vec![job(
            "https://c.example.com/job",
            "B Engineer",
            "BravoCo",
            Some(combined_score),
            ScoreSource::Combined,
        )],
    );
    let out = best(&[hot_keyword, modest_combined]);
    assert_eq!(out.matches.len(), 2);
    assert_eq!(
        out.matches[0].score_source,
        ScoreSource::Combined,
        "the combined block sorts FIRST even though its raw number ({combined_score}) is \
         lower than the keyword row's ({keyword_score}) — the two axes are not comparable"
    );
    assert_eq!(out.matches[1].score_source, ScoreSource::Keyword);
}

#[test]
fn autopilot_count_excludes_non_contributing_autopilots() {
    let a = single(
        "a",
        "https://a.example.com/job",
        "Data Engineer",
        "AlphaCo",
        90.0,
    );
    let b = single(
        "b",
        "https://b.example.com/job",
        "Data Scientist",
        "BetaCo",
        90.0,
    );
    let c = single(
        "c",
        "https://c.example.com/job",
        "Data Analyst",
        "GammaCo",
        10.0,
    );
    let out = best(&[a, b, c]);
    assert_eq!(out.matches.len(), 2);
    assert_eq!(
        out.autopilot_count, 2,
        "an autopilot with zero qualifying rows doesn't count"
    );
}

#[test]
fn archived_excluded_paused_included_and_marked() {
    let archived = autopilot(
        "arc",
        AutopilotStatus::Archived,
        vec![job(
            "https://x.example.com/job",
            "Backend Engineer",
            "Widgets Co",
            Some(90.0),
            ScoreSource::Keyword,
        )],
    );
    let paused = autopilot(
        "p",
        AutopilotStatus::Paused,
        vec![job(
            "https://y.example.com/job",
            "Backend Engineer",
            "Gizmos Inc",
            Some(90.0),
            ScoreSource::Keyword,
        )],
    );
    let out = best(&[archived, paused]);
    assert_eq!(
        out.matches.len(),
        1,
        "an archived record's jobs never appear"
    );
    assert!(
        out.matches[0].sources[0].paused,
        "a paused autopilot's rows are marked paused, not excluded"
    );
}

#[test]
fn qualification_cut_depends_on_score_source() {
    // A score that clears the (lower) coverage cut but not the (higher)
    // combined cut — derived from the consts so this stays correct if
    // either cut moves (both are documented "not calibrated"), rather
    // than pinning today's specific numbers.
    let score = MATCH_TIER_COVERAGE_HIGH + 1.0;
    assert!(
        score < MATCH_TIER_COMBINED_HIGH,
        "fixture assumption: the coverage High cut sits below the combined one"
    );
    let keyword_ap = single(
        "k",
        "https://k.example.com/job",
        "Data Engineer",
        "KeyCo",
        score,
    );
    let combined_ap = autopilot(
        "c",
        AutopilotStatus::Active,
        vec![job(
            "https://c.example.com/job",
            "Data Engineer II",
            "CombCo",
            Some(score),
            ScoreSource::Combined,
        )],
    );
    let out = best(&[keyword_ap, combined_ap]);
    assert_eq!(
        out.matches.len(),
        1,
        "a coverage-qualifying score does not also qualify under the combined cut"
    );
    assert_eq!(out.matches[0].score_source, ScoreSource::Keyword);
}

#[test]
fn qualifies_at_the_exact_high_cut_for_both_kernels() {
    // The boundary is reachable in practice (coverage is a `matched /
    // total * 100` percentage, so an exact 55.0 is a real score) and the
    // renderer's `scoreTier` uses `>=` too — both must agree at the cut,
    // not just above it.
    let keyword_ap = single(
        "k",
        "https://k.example.com/job",
        "Data Engineer",
        "KeyCo",
        MATCH_TIER_COVERAGE_HIGH,
    );
    let combined_ap = autopilot(
        "c",
        AutopilotStatus::Active,
        vec![job(
            "https://c.example.com/job",
            "Data Engineer II",
            "CombCo",
            Some(MATCH_TIER_COMBINED_HIGH),
            ScoreSource::Combined,
        )],
    );
    let out = best(&[keyword_ap, combined_ap]);
    assert_eq!(
        out.matches.len(),
        2,
        "a score exactly AT the High cut qualifies, for both kernels"
    );
}

#[test]
fn total_counts_qualifying_rows_before_the_cap() {
    let jobs: Vec<FoundJob> = (0..120)
        .map(|i| {
            job(
                &format!("https://many.example.com/job/{i}"),
                &format!("Engineer {i}"),
                &format!("Co{i}"),
                Some(90.0),
                ScoreSource::Keyword,
            )
        })
        .collect();
    let ap = autopilot("many", AutopilotStatus::Active, jobs);
    let out = best(&[ap]);
    assert_eq!(out.total, 120);
    assert_eq!(
        out.matches.len(),
        120,
        "the shared computation is uncapped (#1448); the IPC command applies BEST_MATCHES_CAP"
    );
    assert!(out.matches.len() > BEST_MATCHES_CAP);
}

#[test]
fn unscored_clusters_never_qualify() {
    let ap = autopilot(
        "u",
        AutopilotStatus::Active,
        vec![job(
            "https://u.example.com/job",
            "Support Engineer",
            "Unco",
            None,
            ScoreSource::Keyword,
        )],
    );
    let out = best(&[ap]);
    assert!(out.matches.is_empty());
    assert_eq!(out.total, 0);
}
