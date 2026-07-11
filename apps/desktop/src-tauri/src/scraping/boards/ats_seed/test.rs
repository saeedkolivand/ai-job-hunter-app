use super::*;

// ---------------------------------------------------------------------------
// Shape
// ---------------------------------------------------------------------------

#[test]
fn table_is_non_empty_and_every_entry_has_required_fields() {
    let entries = all();
    assert!(!entries.is_empty(), "seed table must not be empty");
    for e in entries {
        assert!(!e.company.is_empty(), "entry has empty company: {e:?}");
        assert!(!e.ats.is_empty(), "entry has empty ats: {e:?}");
        assert!(!e.slug.is_empty(), "entry has empty slug: {e:?}");
    }
}

/// Locks the curated count so a partial paste/merge mistake trips a test
/// instead of silently shipping fewer rows than verified.
#[test]
fn table_has_the_verified_entry_count() {
    assert_eq!(all().len(), 59, "expected 59 verified seed entries");
}

// ---------------------------------------------------------------------------
// `ats` must route to a real board — cross-checked against the live registry,
// not a hardcoded copy of the id list, so a future SCRAPERS rename trips this.
// ---------------------------------------------------------------------------

#[test]
fn every_ats_value_matches_a_registered_scraper_id() {
    let registered: std::collections::HashSet<&str> = crate::scraping::boards::all()
        .iter()
        .map(|s| s.id())
        .collect();
    for e in all() {
        assert!(
            registered.contains(e.ats),
            "seed entry '{}' has ats '{}' which is not a registered SCRAPERS id",
            e.company,
            e.ats
        );
    }
}

// ---------------------------------------------------------------------------
// Personio TLD quirk
// ---------------------------------------------------------------------------

#[test]
fn personio_entries_have_tld_others_dont() {
    for e in all() {
        if e.ats == "personio" {
            assert!(
                e.tld.is_some(),
                "personio entry '{}' must have a tld",
                e.company
            );
        } else {
            assert!(
                e.tld.is_none(),
                "non-personio entry '{}' must not have a tld",
                e.company
            );
        }
    }
}

// ---------------------------------------------------------------------------
// DACH coverage
// ---------------------------------------------------------------------------

#[test]
fn dach_count_is_at_least_twenty() {
    let dach = all().iter().filter(|e| e.dach).count();
    assert!(dach >= 20, "expected >= 20 DACH entries, got {dach}");
}

// ---------------------------------------------------------------------------
// by_ats
// ---------------------------------------------------------------------------

#[test]
fn by_ats_greenhouse_and_personio_are_non_empty() {
    assert!(
        by_ats("greenhouse").count() > 0,
        "expected >=1 greenhouse entry"
    );
    assert!(
        by_ats("personio").count() > 0,
        "expected >=1 personio entry"
    );
}

#[test]
fn by_ats_only_returns_matching_entries() {
    for e in by_ats("lever") {
        assert_eq!(e.ats, "lever");
    }
    assert!(by_ats("not-a-real-board").next().is_none());
}

// ---------------------------------------------------------------------------
// Uniqueness
// ---------------------------------------------------------------------------

#[test]
fn no_duplicate_ats_slug_pairs() {
    let mut seen = std::collections::HashSet::new();
    for e in all() {
        let key = (e.ats, e.slug);
        assert!(seen.insert(key), "duplicate (ats, slug) pair: {key:?}");
    }
}
