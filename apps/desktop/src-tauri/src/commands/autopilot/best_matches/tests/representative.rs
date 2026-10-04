//! Which cluster member the row stands for: the best-scored member decides the score, the canonical
//! member decides the display, and `scoreUrl` says when they differ.

use super::*;

#[test]
fn best_scored_member_wins_within_its_own_scale_even_when_not_canonical() {
    // Cluster = {canonical: combined 40, full JD, direct board}
    //         u {non-canonical: keyword 60, aggregator snippet, no JD}.
    // `resolve_block`'s canonical-preference (has_description desc,
    // non-aggregator source first) picks the FIRST as canonical even
    // though its raw number is lower than the second's — exactly the
    // shape H1 broke: a raw cross-scale compare would have picked the
    // higher-numbered keyword aggregator copy as the row's score,
    // mislabeling a genuinely-scored semantic match as a much stronger
    // "combined" number than the semantic kernel actually gave it. Both
    // scores clear their OWN High cut, so the cluster still qualifies
    // either way — this isolates the block-selection question from
    // `qualifies`.
    let combined_score = MATCH_TIER_COMBINED_HIGH + 1.0;
    let keyword_score = MATCH_TIER_COVERAGE_HIGH + 35.0;
    assert!(
        keyword_score > combined_score,
        "fixture assumption: the keyword number reads as \"better\" raw"
    );
    let canonical_job = FoundJob {
        description: Some("full JD text".into()),
        board: Some("greenhouse".into()),
        ..job(
            "https://x.example.com/job",
            "Senior Rust Engineer",
            "Acme",
            Some(combined_score),
            ScoreSource::Combined,
        )
    };
    let aggregator_copy = FoundJob {
        description: None,
        board: Some(crate::scraping::boards::aggregator::AGGREGATOR_BOARD_ID.into()),
        ..job(
            "https://agg.example.com/job?id=1",
            "Senior Rust Engineer",
            "Acme",
            Some(keyword_score),
            ScoreSource::Keyword,
        )
    };
    let ap = autopilot(
        "a",
        AutopilotStatus::Active,
        vec![canonical_job, aggregator_copy],
    );
    let out = best(&[ap]);
    assert_eq!(
        out.matches.len(),
        1,
        "identical title+company joins one cluster"
    );
    let row = &out.matches[0];
    assert_eq!(
        row.score_source,
        ScoreSource::Combined,
        "Combined beats Keyword regardless of the raw number"
    );
    assert_eq!(row.score, combined_score);
    assert_eq!(
        row.board.as_deref(),
        Some("greenhouse"),
        "display fields still come from the CANONICAL member, not the best-scored one"
    );
    assert_eq!(
        row.score_url, None,
        "the canonical member IS the best-scored one here, so the row is already \
         self-consistent — no scoreUrl needed"
    );
}

#[test]
fn assistant_notes_prefer_canonical_over_first_in_input_order() {
    // First-in-input-order member is NOT canonical (no description, so
    // `resolve_block`'s canonical-preference ranks it below the second
    // member) and carries its OWN note. The canonical member (full JD)
    // carries a DIFFERENT note. The canonical's note must win — the same
    // member every other display field (title/company/url/board/...)
    // already reads from.
    let first_in_input = FoundJob {
        description: None,
        assistant_notes: Some("note from the first-found aggregator copy".into()),
        ..job(
            "https://agg.example.com/job?id=1",
            "Senior Rust Engineer",
            "Acme",
            Some(90.0),
            ScoreSource::Keyword,
        )
    };
    let canonical_job = FoundJob {
        description: Some("full JD text".into()),
        assistant_notes: Some("note from the canonical board copy".into()),
        ..job(
            "https://x.example.com/job",
            "Senior Rust Engineer",
            "Acme",
            Some(80.0),
            ScoreSource::Keyword,
        )
    };
    let ap = autopilot(
        "a",
        AutopilotStatus::Active,
        vec![first_in_input, canonical_job],
    );
    let out = best(&[ap]);
    assert_eq!(
        out.matches.len(),
        1,
        "identical title+company joins one cluster"
    );
    assert_eq!(
        out.matches[0].assistant_notes.as_deref(),
        Some("note from the canonical board copy"),
        "assistant_notes must come from the canonical member, not whichever member \
         happens to be first in input order"
    );
    // The higher-scored `first_in_input` (90.0) is NOT the canonical member
    // here (`canonical_job`, 80.0, wins canonical via its description) — the
    // exact split that produced #1104: `score` and `url` describe two
    // different real postings. `scoreUrl` must say so.
    assert_eq!(out.matches[0].url, "https://x.example.com/job");
    assert_eq!(out.matches[0].score, 90.0);
    assert_eq!(
        out.matches[0].score_url.as_deref(),
        Some("https://agg.example.com/job?id=1"),
        "scoreUrl must identify the actual member `score` was computed from, since it \
         isn't the canonical member `url` displays"
    );
}

#[test]
fn score_url_is_none_when_canonical_and_best_scored_coincide() {
    // Single-member cluster: canonical and best-scored are trivially the
    // same job, so scoreUrl must stay absent — a permanently-populated
    // field for the common case would be noise, not information.
    let ap = single(
        "s",
        "https://solo.example.com/job",
        "Solo Engineer",
        "SoloCo",
        90.0,
    );
    let out = best(&[ap]);
    assert_eq!(out.matches.len(), 1);
    assert_eq!(out.matches[0].score_url, None);
}

#[test]
fn score_url_reproduces_the_1104_bug_shape_directly() {
    // Reproduces issue #1104: a two-member cluster where the richer/
    // canonical member (direct board, full JD) scores LOWER, within the
    // SAME score_source block, than a non-canonical member (an aggregator
    // snippet). The best-scored member decides `score`; the canonical
    // decides `url`. Before this fix the row silently paired one
    // member's url with the OTHER member's score; `scoreUrl` now makes
    // that split explicit and lets a caller resolve it.
    let canonical_job = FoundJob {
        description: Some("full JD text".into()),
        ..job(
            "https://direct.example.com/job",
            "Senior Rust Engineer",
            "Acme",
            Some(60.0),
            ScoreSource::Keyword,
        )
    };
    let best_scored = FoundJob {
        description: None,
        board: Some(crate::scraping::boards::aggregator::AGGREGATOR_BOARD_ID.into()),
        ..job(
            "https://agg.example.com/job?id=7",
            "Senior Rust Engineer",
            "Acme",
            Some(90.0),
            ScoreSource::Keyword,
        )
    };
    let ap = autopilot(
        "a",
        AutopilotStatus::Active,
        vec![canonical_job, best_scored],
    );
    let out = best(&[ap]);
    assert_eq!(
        out.matches.len(),
        1,
        "identical title+company joins one cluster"
    );
    let row = &out.matches[0];
    assert_eq!(
        row.url, "https://direct.example.com/job",
        "display fields still come from the canonical (richer) member"
    );
    assert_eq!(
        row.score, 90.0,
        "the cluster's best score still surfaces (ADR-036 intent preserved)"
    );
    assert_eq!(
        row.score_url.as_deref(),
        Some("https://agg.example.com/job?id=7"),
        "scoreUrl points at the actual posting the displayed score was computed from"
    );
}
