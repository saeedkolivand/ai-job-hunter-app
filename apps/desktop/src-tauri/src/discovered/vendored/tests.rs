use super::*;

/// Every embedded asset decodes, is non-empty, and contains no empty or
/// purely-numeric slug (crawler junk the import filter should have
/// dropped already — this is a regression guard on that filter, not a
/// re-run of it).
#[test]
fn every_platform_parses_non_empty_and_clean() {
    for (ats, entries) in INDEX.iter() {
        assert!(!entries.is_empty(), "{ats}: vendored slug list is empty");
        for e in entries {
            assert!(
                !e.slug.is_empty(),
                "{ats}: an entry decoded to an empty slug"
            );
            assert!(
                !e.slug.chars().all(|c| c.is_ascii_digit()),
                "{ats}: purely-numeric slug leaked through the import filter: {}",
                e.slug
            );
        }
    }
}

/// Every platform key matches a registered company-scoped board — the
/// same registry predicate `commands::discovery::discovery_set_starred`
/// gates starring on, so a vendored row can never suggest an ATS the
/// scraping engine has no extractor/board for.
#[test]
fn every_platform_is_a_supported_company_scoped_board() {
    for (ats, _) in PLATFORMS {
        let scraper = crate::scraping::boards::get(ats);
        assert!(scraper.is_some(), "{ats}: not a registered board id");
        assert!(
            scraper.unwrap().requires_company(),
            "{ats}: registered but not company-scoped"
        );
    }
}

/// A corrupted checkout or a hand-edited asset must fail loudly, not ship
/// silently — same guard `geonames` runs on its embedded dumps.
#[test]
fn embedded_assets_match_their_recorded_digests() {
    use std::fmt::Write;

    use sha2::{Digest, Sha256};

    // `Sha256::digest` yields a byte array with no hex `Display`; fold it
    // the same way `commands::geocoding::tests::index_build`'s equivalent guard does.
    let digest = |bytes: &[u8]| {
        Sha256::digest(bytes)
            .iter()
            .fold(String::with_capacity(64), |mut acc, b| {
                let _ = write!(acc, "{b:02x}");
                acc
            })
    };
    let by_ats: HashMap<&str, &[u8]> = PLATFORMS.iter().copied().collect();
    for (ats, expected) in DIGESTS {
        let actual = digest(by_ats[ats]);
        assert_eq!(
            &actual, expected,
            "{ats}: ats-slugs/{ats}.txt.gz digest drifted from the recorded one — \
             update DIGESTS (and ats-slugs/README.md) if this asset was intentionally refreshed"
        );
    }
}

#[test]
fn search_is_case_insensitive_and_preserves_slug_casing() {
    // Ashby's list is lowercase on disk; a mixed-case query must still
    // find it, and the returned slug must be the on-disk (lowercase) form
    // — never re-cased by the query.
    let hits = search("NOTION", 10);
    assert!(
        hits.iter()
            .any(|h| h.ats_kind == "ashby" && h.slug == "notion"),
        "expected a case-insensitive match for a known ashby slug"
    );
}

#[test]
fn search_respects_limit_and_marks_source() {
    let hits = search("a", 5);
    assert!(hits.len() <= 5, "limit must be respected");
    assert!(
        hits.iter()
            .all(|h| h.source == "vendor" && !h.starred && h.seen_count == 0),
        "every vendored hit must be unstarred, zero-seen, source=vendor"
    );
}

#[test]
#[ignore = "manual perf probe, not CI — see vendored.rs search() doc comment for the recorded numbers"]
fn perf_probe() {
    let build_start = std::time::Instant::now();
    LazyLock::force(&INDEX);
    eprintln!("index build: {:?}", build_start.elapsed());
    for q in ["a", "acme", "notion", "zzz-does-not-exist"] {
        let start = std::time::Instant::now();
        let hits = search(q, 50);
        eprintln!("query {q:?}: {:?} ({} hits)", start.elapsed(), hits.len());
    }
}

#[test]
fn empty_query_returns_nothing() {
    assert!(search("", 50).is_empty());
    assert!(search("   ", 50).is_empty());
}

#[test]
fn unknown_slug_returns_no_hits() {
    assert!(search("this-company-does-not-exist-anywhere-xyz", 10).is_empty());
}
