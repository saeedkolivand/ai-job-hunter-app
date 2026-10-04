//! What a re-surfaced job refreshes on its existing row, and what it must never clobber.

use super::super::*;
use super::support::*;
use crate::autopilot::merge::merge_found_jobs;

#[test]
fn merge_preserves_and_refreshes_board_across_resurface() {
    // An existing row persisted before `board` existed (None) must pick up the
    // board when the same URL re-surfaces, and a never-seen URL keeps its board.
    let mut existing = found_job("https://a.com/1", 100);
    existing.board = None; // legacy row, no provenance yet

    let mut resurfaced = found_job("https://a.com/1", 999);
    resurfaced.board = Some("linkedin".into());
    let mut fresh = found_job("https://a.com/2", 200);
    fresh.board = Some("aggregator".into());

    let merged = merge_found_jobs(&[existing], vec![resurfaced, fresh]);

    let a1 = merged.iter().find(|j| j.url == "https://a.com/1").unwrap();
    assert_eq!(
        a1.board,
        Some("linkedin".to_string()),
        "re-surfaced existing row picks up the incoming board"
    );
    let a2 = merged.iter().find(|j| j.url == "https://a.com/2").unwrap();
    assert_eq!(
        a2.board,
        Some("aggregator".to_string()),
        "appended new row keeps its board (via ..inc spread)"
    );
}

#[test]
fn merge_preserves_a_real_description_over_a_blank_resurface() {
    // LinkedIn search results always carry `description: Some("")` (never
    // `None`) — that is its "unknown" sentinel, not `None`. A posting
    // enriched by `autopilot_helpers::linkedin_enrich` on a prior run must
    // NOT lose that real description just because it resurfaces in a fresh
    // LinkedIn scrape, which reports the same blank sentinel every time the
    // posting is still listed.
    let mut existing = found_job("https://a.com/1", 100);
    existing.description = Some("A real, fetched job description.".to_string());

    let mut resurfaced = found_job("https://a.com/1", 999);
    resurfaced.description = Some(String::new()); // LinkedIn's blank sentinel

    let merged = merge_found_jobs(&[existing], vec![resurfaced]);

    let a1 = merged.iter().find(|j| j.url == "https://a.com/1").unwrap();
    assert_eq!(
        a1.description,
        Some("A real, fetched job description.".to_string()),
        "a blank resurface must never clobber an already-known real description"
    );
}

#[test]
fn merge_preserves_and_refreshes_trust_across_resurface() {
    // Same legacy-migration case as the board test above: an existing row
    // persisted before `trust` existed (None) must pick up the incoming trust
    // when the same URL re-surfaces, and a never-seen URL keeps its trust.
    let mut existing = found_job("https://a.com/1", 100);
    existing.trust = None; // legacy row, no trust assessment yet

    let resurfaced_trust = crate::scraping::trust::assess_trust(
        "https://linkedin.com/jobs/view/1",
        "Acme",
        "A real description.",
    );
    let mut resurfaced = found_job("https://a.com/1", 999);
    resurfaced.trust = Some(resurfaced_trust.clone());
    let fresh_trust = crate::scraping::trust::assess_trust(
        "https://boards.greenhouse.io/acme/jobs/2",
        "Acme",
        "A real description.",
    );
    let mut fresh = found_job("https://a.com/2", 200);
    fresh.trust = Some(fresh_trust.clone());

    let merged = merge_found_jobs(&[existing], vec![resurfaced, fresh]);

    let a1 = merged.iter().find(|j| j.url == "https://a.com/1").unwrap();
    assert_eq!(
        a1.trust,
        Some(resurfaced_trust),
        "re-surfaced existing row picks up the incoming trust"
    );
    let a2 = merged.iter().find(|j| j.url == "https://a.com/2").unwrap();
    assert_eq!(
        a2.trust,
        Some(fresh_trust),
        "appended new row keeps its trust (via ..inc spread)"
    );
}

#[test]
fn merge_preserves_and_refreshes_posted_at_across_resurface() {
    // Same legacy-migration case as the board/trust tests above: an existing
    // row persisted before `posted_at` existed (None) — or scraped from a
    // board that didn't expose a publish date on an earlier run — must pick up
    // the incoming date when the same URL re-surfaces, and a never-seen URL
    // keeps its own date.
    let mut existing = found_job("https://a.com/1", 100);
    existing.posted_at = None; // legacy/dateless row

    let mut resurfaced = found_job("https://a.com/1", 999);
    resurfaced.posted_at = Some(1_700_000_000_000);
    let mut fresh = found_job("https://a.com/2", 200);
    fresh.posted_at = Some(1_650_000_000_000);

    let merged = merge_found_jobs(&[existing], vec![resurfaced, fresh]);

    let a1 = merged.iter().find(|j| j.url == "https://a.com/1").unwrap();
    assert_eq!(
        a1.posted_at,
        Some(1_700_000_000_000),
        "re-surfaced existing row backfills the incoming posted_at"
    );
    let a2 = merged.iter().find(|j| j.url == "https://a.com/2").unwrap();
    assert_eq!(
        a2.posted_at,
        Some(1_650_000_000_000),
        "appended new row keeps its posted_at (via ..inc spread)"
    );
}

#[test]
fn merge_keeps_a_known_posted_at_when_the_resurfaced_row_has_none() {
    // The mirror of the backfill case above: a row with an already-known date
    // must NOT lose it when the same job re-surfaces from a board/run that
    // didn't report one this time (several boards send no date at all, and
    // even a board that usually does can drop it on one page). The guard is
    // `if inc.posted_at.is_some()` — an unconditional `row.posted_at =
    // inc.posted_at` would silently erase a known date here.
    let mut existing = found_job("https://a.com/1", 100);
    existing.posted_at = Some(1_700_000_000_000); // known date from a prior run

    let mut resurfaced = found_job("https://a.com/1", 999);
    resurfaced.posted_at = None; // this run's copy has no date

    let merged = merge_found_jobs(&[existing], vec![resurfaced]);

    let a1 = merged.iter().find(|j| j.url == "https://a.com/1").unwrap();
    assert_eq!(
        a1.posted_at,
        Some(1_700_000_000_000),
        "a known posted_at must survive a resurface that reports no date"
    );
}

#[test]
fn merge_score_provisional_moves_with_score_across_resurface() {
    // `score_provisional` describes WHICH score is on the row, so a resurface
    // that refreshes `score` must refresh the flag alongside it — in BOTH
    // directions. Resurfacing is ORDINARY autopilot behavior (the same job
    // returned by more than one board, or seen again on a later run), not an
    // edge case, so a desync here would be routinely user-visible.

    // (a) aggregator-first (provisional score) resurfaced by a full-text board
    // (authoritative score) → the flag must flip to false with the new score.
    let mut existing_provisional = found_job("https://a.com/1", 100);
    existing_provisional.score = Some(40.0);
    existing_provisional.score_provisional = true;

    let mut resurfaced_authoritative = found_job("https://a.com/1", 999);
    resurfaced_authoritative.score = Some(72.0);
    resurfaced_authoritative.score_provisional = false;

    let merged = merge_found_jobs(&[existing_provisional], vec![resurfaced_authoritative]);
    let row = merged.iter().find(|j| j.url == "https://a.com/1").unwrap();
    assert_eq!(row.score, Some(72.0));
    assert!(
        !row.score_provisional,
        "a full-text board's authoritative score resurfacing over an old \
         aggregator snippet score must clear the provisional flag"
    );

    // (b) full-text-first (authoritative) resurfaced by the aggregator
    // (snippet) → the flag must flip to TRUE with the snippet score — the
    // worse direction: a snippet score must never display as authoritative.
    let mut existing_authoritative = found_job("https://b.com/1", 100);
    existing_authoritative.score = Some(72.0);
    existing_authoritative.score_provisional = false;

    let mut resurfaced_provisional = found_job("https://b.com/1", 999);
    resurfaced_provisional.score = Some(40.0);
    resurfaced_provisional.score_provisional = true;

    let merged = merge_found_jobs(&[existing_authoritative], vec![resurfaced_provisional]);
    let row = merged.iter().find(|j| j.url == "https://b.com/1").unwrap();
    assert_eq!(row.score, Some(40.0));
    assert!(
        row.score_provisional,
        "an aggregator snippet score resurfacing over a prior authoritative \
         score must set the provisional flag — a snippet score must never \
         display as authoritative"
    );
}

#[test]
fn merge_score_source_moves_with_score_across_resurface() {
    // The mirror of the test above, for the OTHER field paired with `score`.
    // `score_source` says which KERNEL produced the number, and it drives the
    // user-facing label (`autopilot.scoreLabel.{coverage,combined}`) plus its
    // tier cut points — so a resurface that refreshes `score` must refresh
    // `score_source` alongside it, in both directions.

    // (a) a semantic re-rank's combined score, resurfaced by a later run whose
    // re-rank never reached this job (semantic turned off, the daily ceiling,
    // the wall clock, the degrade breaker, an offline provider). The keyword
    // number must not inherit the previous run's "combined" label.
    let mut existing_combined = found_job("https://a.com/1", 100);
    existing_combined.score = Some(91.0);
    existing_combined.score_source = ScoreSource::Combined;

    let mut resurfaced_keyword = found_job("https://a.com/1", 999);
    resurfaced_keyword.score = Some(62.0);
    resurfaced_keyword.score_source = ScoreSource::Keyword;

    let merged = merge_found_jobs(&[existing_combined], vec![resurfaced_keyword]);
    let row = merged.iter().find(|j| j.url == "https://a.com/1").unwrap();
    assert_eq!(row.score, Some(62.0));
    assert_eq!(
        row.score_source,
        ScoreSource::Keyword,
        "a keyword score resurfacing over a prior combined score must carry the \
         keyword label — the two kernels are different scales, and the stale \
         label would relabel 62 as a semantic verdict it never was"
    );

    // (b) the reverse: a keyword row that this run's re-rank DID reach must
    // pick the combined label up, or the semantic number keeps being displayed
    // (and banded) as plain coverage.
    let mut existing_keyword = found_job("https://b.com/1", 100);
    existing_keyword.score = Some(62.0);
    existing_keyword.score_source = ScoreSource::Keyword;

    let mut resurfaced_combined = found_job("https://b.com/1", 999);
    resurfaced_combined.score = Some(91.0);
    resurfaced_combined.score_source = ScoreSource::Combined;

    let merged = merge_found_jobs(&[existing_keyword], vec![resurfaced_combined]);
    let row = merged.iter().find(|j| j.url == "https://b.com/1").unwrap();
    assert_eq!(row.score, Some(91.0));
    assert_eq!(
        row.score_source,
        ScoreSource::Combined,
        "a re-ranked score must arrive with its own label"
    );
}
