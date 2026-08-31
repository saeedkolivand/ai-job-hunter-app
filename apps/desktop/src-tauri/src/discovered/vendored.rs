//! Vendored community ATS company-slug directory — the "community slug
//! directories" feeder ADR-030 §b names explicitly (`DiscoveredCompany.source`
//! is free-text "so future feeders \[...\] require no schema changes").
//!
//! Unlike organically-[`super::harvest_ats_refs`]ed rows, this is a static
//! offline snapshot (~27k slugs, gzip-embedded via [`include_bytes!`])
//! covering the four company-scoped ATS platforms
//! `scraping::ats_ref::extract_ats_ref` can resolve today: BambooHR,
//! Greenhouse, Lever, Ashby.
//!
//! **Attribution (required in distributed builds):** vendored from
//! [`Feashliaa/job-board-aggregator`](https://github.com/Feashliaa/job-board-aggregator),
//! © Riley Dorrington. The upstream *code* repo is MIT, but its README carves
//! the `data/` datasets themselves out under **CC BY-NC 4.0**
//! (<https://creativecommons.org/licenses/by-nc/4.0/>) — this app is itself
//! PolyForm Noncommercial, so no license conflict. Full provenance (upstream
//! commit, snapshot date, per-asset SHA-256) lives in `ats-slugs/README.md`.
//!
//! It is searched **only**
//! at typeahead time (`commands::discovery::discovery_search_companies`,
//! merged behind organic/starred rows) — it is deliberately NEVER read by
//! `scraping::boards::ats_seed` or the engine's company auto-populate path,
//! which would turn "27k known slugs" into "27k real HTTP requests" the first
//! time a user left a company field blank. Rows here never touch
//! `discovered_companies` (no DB writes) unless the user stars one, at which
//! point [`super::DiscoveredCompanyStore::set_starred`]'s existing
//! seed-materialization path takes over exactly as it does for a curated
//! `ats_seed` entry.

use std::collections::HashMap;
use std::io::Read;
use std::sync::LazyLock;

use super::DiscoveredCompany;

/// `(ats_kind, gzip bytes)` for every vendored platform — must match a
/// `Scraper::id()` in the `SCRAPERS` registry (asserted by
/// `every_platform_is_a_supported_company_scoped_board`). Order is
/// display/log order only; [`search`] scans every platform regardless.
const PLATFORMS: &[(&str, &[u8])] = &[
    (
        "bamboohr",
        include_bytes!("../../ats-slugs/bamboohr.txt.gz"),
    ),
    (
        "greenhouse",
        include_bytes!("../../ats-slugs/greenhouse.txt.gz"),
    ),
    ("lever", include_bytes!("../../ats-slugs/lever.txt.gz")),
    ("ashby", include_bytes!("../../ats-slugs/ashby.txt.gz")),
];

/// SHA-256 of the four embedded assets, recorded when they were vendored (see
/// `ats-slugs/README.md` for the upstream commit + snapshot date). Asserted by
/// a test so a corrupted checkout or a hand-edited asset fails the suite
/// rather than silently shipping data of unknown origin (mirrors
/// `commands::geocoding::geonames`'s `CITIES_SHA256`).
#[cfg(test)]
const DIGESTS: &[(&str, &str)] = &[
    (
        "bamboohr",
        "0a36b2c423767ce4771679e0e08f72de33021a09ac60cd251f1deb16bdd19644",
    ),
    (
        "greenhouse",
        "7190fb24209f8c4c7083ae445ccd5c89132a97d71524a7f1c251e1a34c1fdb9c",
    ),
    (
        "lever",
        "c08c0860188d6e7942ce3311ab9c15d1e945747ca6c52e2e5628ac7bfdd50854",
    ),
    (
        "ashby",
        "2ab565a3f37b4689f23d5e9389cd1e1a67f37b438713902c67e5fda776e21c87",
    ),
];

/// One decoded row: the ASCII-lowercased form used for a case-insensitive
/// substring match, alongside the original slug (casing preserved — Ashby
/// tokens are case-sensitive per `ats_ref::ashby_slug`). Folding once at load
/// time, rather than per query, is what keeps a 27k-row scan cheap on every
/// keystroke — see `search`'s doc comment for measured numbers.
struct Entry {
    folded: String,
    slug: String,
}

/// Built once on first use, then only read — same discipline as
/// `commands::geocoding::geonames::INDEX`.
static INDEX: LazyLock<HashMap<&'static str, Vec<Entry>>> = LazyLock::new(load);

fn load() -> HashMap<&'static str, Vec<Entry>> {
    PLATFORMS
        .iter()
        .map(|(ats, gz)| (*ats, decode(ats, gz)))
        .collect()
}

/// Gunzip + split one platform's newline-delimited slug list. A corrupt or
/// truncated asset degrades to an empty list for that platform (never a
/// panic) — mirrors `geonames::decode_cities`.
fn decode(ats: &str, gz: &[u8]) -> Vec<Entry> {
    let mut decoded = String::new();
    if let Err(e) = flate2::read::GzDecoder::new(gz).read_to_string(&mut decoded) {
        log::warn!("vendored ats slugs ({ats}): bundled asset unreadable: {e}");
        return Vec::new();
    }
    decoded
        .lines()
        .filter(|s| !s.is_empty())
        .map(|slug| Entry {
            folded: slug.to_ascii_lowercase(),
            slug: slug.to_string(),
        })
        .collect()
}

/// Search the vendored directory for `query` (case-insensitive substring),
/// across all four platforms, capped at `limit`. Every row ties on
/// `starred=false, seen_count=0` (there is nothing else to rank a vendored row
/// on), so the tie-break is alphabetical by slug — same tie-break
/// `DiscoveredCompanyStore::search`'s `ORDER BY` ends on.
///
/// An empty query returns nothing: unlike the DB (whose empty-query path
/// surfaces the user's own top-seen/starred rows), an alphabetically-first
/// slice of ~27k unfamiliar slugs is not a useful "browse everything"
/// default, and it's also the one query shape that can't be avoided.
///
/// **Measured (release build, this asset, via `perf_probe`):** ~2.9 ms for
/// the one-time `LazyLock` build (gunzip + fold ~27k rows across 4
/// platforms), then **worst case ~1.8 ms** for `"a"` (a near-universal
/// single-letter substring — every row gets scanned and most match) down to
/// under 0.1 ms for a query that matches nothing. A full linear scan +
/// fold-free `str::contains` over already-folded rows is comfortably inside
/// the typeahead's 250 ms debounce, so no bucket/prefix index was built (same
/// call `geonames` makes for its larger asset).
pub fn search(query: &str, limit: usize) -> Vec<DiscoveredCompany> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut hits: Vec<(&str, &str)> = Vec::new();
    for (ats, entries) in INDEX.iter() {
        for entry in entries {
            if entry.folded.contains(&q) {
                hits.push((ats, entry.slug.as_str()));
            }
        }
    }
    // Alphabetical by slug, then ats (stable, deterministic across the
    // HashMap's unordered platform iteration above).
    hits.sort_unstable_by(|a, b| a.1.cmp(b.1).then_with(|| a.0.cmp(b.0)));
    hits.into_iter()
        .take(limit)
        .map(|(ats, slug)| DiscoveredCompany {
            ats_kind: ats.to_string(),
            slug: slug.to_string(),
            display_name: None,
            seen_count: 0,
            starred: false,
            source: "vendor".to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
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
        // the same way `commands::geocoding::test`'s equivalent guard does.
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
}
