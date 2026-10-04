use super::{
    clamp_bytes, fill_with_vendor_results, merge_vendor_results, DiscoveredCompany, MAX_QUERY_BYTES,
};

fn company(ats: &str, slug: &str, source: &str) -> DiscoveredCompany {
    DiscoveredCompany {
        ats_kind: ats.to_string(),
        slug: slug.to_string(),
        display_name: None,
        seen_count: if source == "vendor" { 0 } else { 3 },
        starred: false,
        source: source.to_string(),
    }
}

#[test]
fn vendor_rows_fill_in_after_db_rows() {
    let db = vec![company("greenhouse", "stripe", "scrape")];
    let vendor = vec![company("ashby", "notion", "vendor")];
    let merged = merge_vendor_results(db, vendor, 10);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].slug, "stripe");
    assert_eq!(merged[1].slug, "notion");
}

#[test]
fn a_db_row_wins_over_a_duplicate_vendor_row_case_insensitively() {
    // Ashby preserves casing — the DB row (real seen_count) is `Linear`,
    // the vendor directory only has the lowercase `linear`. Must dedupe.
    let db = vec![company("ashby", "Linear", "scrape")];
    let vendor = vec![company("ashby", "linear", "vendor")];
    let merged = merge_vendor_results(db, vendor, 10);
    assert_eq!(merged.len(), 1, "the vendor duplicate must be dropped");
    assert_eq!(
        merged[0].source, "scrape",
        "the DB row must win, not the vendor row"
    );
}

#[test]
fn distinct_ats_kinds_with_the_same_slug_both_survive() {
    // Same slug string on two different ATS platforms is not a collision.
    let db = vec![company("greenhouse", "acme", "scrape")];
    let vendor = vec![company("lever", "acme", "vendor")];
    assert_eq!(merge_vendor_results(db, vendor, 10).len(), 2);
}

#[test]
fn remaining_cap_is_applied_after_dedup_not_before() {
    // Two unique vendor candidates but only room for one: the dropped
    // duplicate must not consume the one slot that was available — the
    // page must still fill with the second, non-duplicate candidate.
    let db = vec![company("greenhouse", "stripe", "scrape")];
    let vendor = vec![
        company("greenhouse", "stripe", "vendor"), // duplicate of the db row
        company("ashby", "notion", "vendor"),      // the real remaining candidate
    ];
    let merged = merge_vendor_results(db, vendor, 1);
    assert_eq!(
        merged.len(),
        2,
        "a duplicate must not starve the page when a further unique match exists"
    );
    assert_eq!(merged[1].slug, "notion");
}

/// The MINOR this fixed: with 49 db rows (1 slot of room), the previous
/// caller asked `vendored::search` for exactly 1 candidate. If that one
/// candidate collided with a db row, the reply came back with 49 rows
/// even though a further vendor match existed. Faking `search_vendor`
/// lets this reproduce the bug deterministically without a real
/// `DiscoveredCompanyStore`/`AppHandle`.
#[test]
fn a_duplicate_near_the_front_of_the_vendor_pool_does_not_starve_the_page() {
    let db: Vec<DiscoveredCompany> = (0..49)
        .map(|i| company("greenhouse", &format!("db-{i}"), "scrape"))
        .collect();
    let fake_vendor = |_: &str, limit: usize| -> Vec<DiscoveredCompany> {
        vec![
            company("greenhouse", "db-0", "vendor"), // duplicate of a db row
            company("ashby", "notion", "vendor"),    // the real remaining candidate
        ]
        .into_iter()
        .take(limit)
        .collect()
    };

    let merged = fill_with_vendor_results(db, "x", fake_vendor);

    assert_eq!(
        merged.len(),
        50,
        "a duplicate must not starve the page when a further unique vendor match exists"
    );
    assert!(
        merged.iter().any(|c| c.slug == "notion"),
        "the further unique vendor match must be present"
    );
}

#[test]
fn clamp_trims_and_byte_caps_on_char_boundary() {
    assert_eq!(clamp_bytes("  hello  ", MAX_QUERY_BYTES), "hello");
    let euros = "€".repeat(100); // 300 bytes > cap
    let out = clamp_bytes(&euros, MAX_QUERY_BYTES);
    assert!(out.len() <= MAX_QUERY_BYTES, "query byte-clamped");
    assert!(
        out.is_char_boundary(out.len()),
        "clamp must cut on a char boundary (valid UTF-8)"
    );
}

/// The registry predicate `discovery_set_starred` gates on: only a registered
/// company-scoped board id may be starred, so a compromised renderer can't
/// materialize garbage rows for a non-ATS or unknown id.
#[test]
fn only_company_scoped_boards_are_watchable() {
    let watchable =
        |ats: &str| crate::scraping::boards::get(ats).is_some_and(|s| s.requires_company());
    assert!(watchable("greenhouse"), "greenhouse is company-scoped");
    assert!(watchable("ashby"), "ashby is company-scoped");
    assert!(!watchable("linkedin"), "linkedin is not company-scoped");
    assert!(!watchable("aggregator"), "aggregator is not company-scoped");
    assert!(!watchable("not-a-real-board"), "unknown id is rejected");
}
