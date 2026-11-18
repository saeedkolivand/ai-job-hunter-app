//! The repost rule: same source, evidence of absence, and the gap.

use super::*;

#[test]
fn a_return_within_the_gap_is_not_a_repost() {
    let (_dir, store) = open();
    // Scraped daily for weeks, the same source ingesting without the key in
    // between (so the marker rule alone would allow a repost): only the gap
    // check holds it back.
    let mut last = see(&store, T0);
    for d in 1..=20 {
        without_acme(&store, T0 + d * DAY - 1, A);
        last = see(&store, T0 + d * DAY);
    }
    assert_eq!(last.repost_count, 0);
    assert_eq!(last.open_days, 20);
}

#[test]
fn a_return_after_the_gap_from_the_same_source_is_a_repost() {
    let (_dir, store) = open();
    see(&store, T0);
    without_acme(&store, T0 + DAY, A);
    let s = see(&store, T0 + REPOST_GAP_DAYS * DAY);
    assert_eq!(s.repost_count, 1, "exactly the gap counts");
    assert_eq!(s.first_seen_at, T0);
    without_acme(&store, T0 + 8 * DAY, A);
    let s = see(&store, T0 + 2 * REPOST_GAP_DAYS * DAY + DAY);
    assert_eq!(s.repost_count, 2);
    assert!(s.reposted);
}

#[test]
fn just_under_the_gap_is_not_a_repost() {
    let (_dir, store) = open();
    see(&store, T0);
    without_acme(&store, T0 + DAY, A);
    assert_eq!(see(&store, T0 + REPOST_GAP_DAYS * DAY - 1).repost_count, 0);
}

#[test]
fn downtime_with_no_ingest_in_between_is_not_a_repost() {
    let (_dir, store) = open();
    see(&store, T0);
    // App closed / offline for a month: no scrape could have seen the key
    // vanish, so its return is not a repost.
    assert_eq!(see(&store, T0 + 30 * DAY).repost_count, 0);
}

#[test]
fn a_failed_or_partial_run_between_sightings_is_not_evidence_of_absence() {
    let (_dir, store) = open();
    see(&store, T0);
    // A run where the source errored, was skipped or kept a partial harvest is
    // reported by the caller as not-ok: it lists other postings (or none) but
    // its marker must not advance.
    store
        .record_scrape([("Other", "Other Role", "u", A)], &[], T0 + 2 * DAY)
        .unwrap();
    store
        .record_scrape(std::iter::empty(), &[], T0 + 3 * DAY)
        .unwrap();
    assert_eq!(see(&store, T0 + 30 * DAY).repost_count, 0);
}

#[test]
fn an_empty_but_successful_run_is_evidence_of_absence() {
    let (_dir, store) = open();
    see(&store, T0);
    store
        .record_scrape(std::iter::empty(), &[A.to_string()], T0 + 2 * DAY)
        .unwrap();
    assert_eq!(see(&store, T0 + 30 * DAY).repost_count, 1);
}

#[test]
fn another_source_ingesting_without_the_key_is_not_a_repost() {
    let (_dir, store) = open();
    see(&store, T0);
    // A different board, a narrower query or a manual search ran in between.
    without_acme(&store, T0 + DAY, B);
    without_acme(&store, T0 + 2 * DAY, "greenhouse|rust remote only");
    assert_eq!(see(&store, T0 + 30 * DAY).repost_count, 0);
}

#[test]
fn a_return_from_a_different_source_than_the_last_is_not_a_repost() {
    let (_dir, store) = open();
    see(&store, T0);
    without_acme(&store, T0 + DAY, A);
    without_acme(&store, T0 + DAY, B);
    // The key reappears on B, not on A (where it last lived).
    assert_eq!(see_from(&store, T0 + 30 * DAY, B).repost_count, 0);
    assert_eq!(acme_row(&store).last_source, B);
}

#[test]
fn a_row_with_no_recorded_source_never_counts() {
    let (_dir, store) = open();
    // A row from before sources existed (an old synced or restored row).
    store
        .import(&serde_json::json!([{
            "ghostKey": "acme|rust engineer", "firstSeenAt": T0, "lastSeenAt": T0,
            "seenCount": 1, "repostCount": 0, "lastUrl": "u"
        }]))
        .unwrap();
    without_acme(&store, T0 + DAY, A);
    assert_eq!(see(&store, T0 + 30 * DAY).repost_count, 0);
    // From now on it has a source, and the rule applies as usual.
    without_acme(&store, T0 + 31 * DAY, A);
    assert_eq!(see(&store, T0 + 60 * DAY).repost_count, 1);
}

#[test]
fn a_restore_resets_the_markers() {
    let (_dir, store) = open();
    see(&store, T0);
    without_acme(&store, T0 + DAY, A);
    let bundle = store.export();
    // Restoring the old rows must not combine with the marker earned since.
    store.import(&bundle).unwrap();
    assert_eq!(see(&store, T0 + 30 * DAY).repost_count, 0);
}

#[test]
fn one_key_under_many_urls_in_one_scrape_counts_once() {
    let (_dir, store) = open();
    see(&store, T0);
    without_acme(&store, T0 + DAY, A);
    let multi_city = [
        ("Acme", "Rust Engineer", "https://acme.test/berlin", A),
        ("Acme", "Rust Engineer", "https://acme.test/munich", A),
        (
            "Acme GmbH",
            "Rust Engineer (m/w/d)",
            "https://acme.test/hamburg",
            A,
        ),
    ];
    let out = store
        .record_scrape(multi_city, &[A.to_string()], T0 + 30 * DAY)
        .unwrap();
    assert_eq!(out.len(), 1);
    // Seen again after a 30-day absence: ONE repost, however many urls.
    assert_eq!(out.into_values().next().unwrap().repost_count, 1);
    let rows = store.rows();
    let acme = acme_row(&store);
    assert_eq!(rows.len(), 2, "acme once + the unrelated key");
    assert_eq!(acme.seen_count, 2, "the scrape counts once");
    assert_eq!(acme.last_url, "https://acme.test/berlin");
}

#[test]
fn concurrent_urls_never_count_as_reposts_across_scrapes() {
    let (_dir, store) = open();
    // Two cities listed in every scrape for a month, the same source ingesting
    // without the key in between: never gone for the gap, never a repost.
    for d in 0..30 {
        without_acme(&store, T0 + d * DAY, A);
        let both = [
            ("Acme", "Rust Engineer", "https://acme.test/berlin", A),
            ("Acme", "Rust Engineer", "https://acme.test/munich", A),
        ];
        let s = store
            .record_scrape(both, &[A.to_string()], T0 + d * DAY + 1)
            .unwrap();
        assert_eq!(s.into_values().next().unwrap().repost_count, 0);
    }
}
