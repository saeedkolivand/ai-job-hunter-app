//! `resolve_watched_companies`: watched stars become a per-board company-slug map (ADR-030 §e).

use super::super::*;

// ── watched-companies resolution (ADR-030 §e) ──────────────────────────────

fn watched(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(a, s)| (a.to_string(), s.to_string()))
        .collect()
}

fn boards(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

#[test]
fn watched_resolution_maps_each_ats_to_its_own_slugs() {
    // Stars for greenhouse + ashby, both boards selected → each board maps to
    // ONLY its own ATS's slugs (no cross-ATS mixing).
    let map = resolve_watched_companies(
        &watched(&[
            ("greenhouse", "stripe"),
            ("ashby", "Linear"),
            ("greenhouse", "airbnb"),
        ]),
        &boards(&["greenhouse", "ashby"]),
    );
    assert_eq!(
        map.get("greenhouse"),
        Some(&vec!["stripe".to_string(), "airbnb".to_string()])
    );
    assert_eq!(map.get("ashby"), Some(&vec!["Linear".to_string()]));
}

#[test]
fn watched_resolution_dedups_within_an_ats_preserving_order() {
    let map = resolve_watched_companies(
        &watched(&[("greenhouse", "acme"), ("greenhouse", "acme")]),
        &boards(&["greenhouse"]),
    );
    assert_eq!(map.get("greenhouse"), Some(&vec!["acme".to_string()]));
}

#[test]
fn watched_resolution_omits_boards_with_no_matching_star() {
    // A lever star and a greenhouse star, but the run selected greenhouse+ashby:
    // greenhouse maps to its slug; ashby is ABSENT (skipped by the engine); the
    // lever star is irrelevant (not a selected board).
    let map = resolve_watched_companies(
        &watched(&[("lever", "spotify"), ("greenhouse", "stripe")]),
        &boards(&["greenhouse", "ashby"]),
    );
    assert_eq!(map.get("greenhouse"), Some(&vec!["stripe".to_string()]));
    assert!(!map.contains_key("ashby"), "ashby has no star → absent");
    assert!(!map.contains_key("lever"), "lever isn't a selected board");
}

#[test]
fn watched_resolution_empty_star_set_yields_empty_map() {
    // Flag on but nothing starred → empty map → the engine skips every
    // company-scoped board `needs-company` (never the curated seed).
    let map = resolve_watched_companies(&[], &boards(&["greenhouse", "ashby"]));
    assert!(map.is_empty());
}

/// Store→resolver seam (ADR-030 §e): stars in a REAL `DiscoveredCompanyStore`
/// (incl. one starred cold → materialized `source='seed'` row) resolve — via the
/// same `store.watched()` + registry `requires_company` filter `autopilot_scrape`
/// composes — into a per-board override map. The missing link between the store
/// tests (start from tuples) and the pure-resolver tests (hand-built `watched()`).
#[test]
fn watched_stars_resolve_from_the_store_into_per_board_targets() {
    use crate::discovered::DiscoveredCompanyStore;
    let dir = tempfile::TempDir::new().unwrap();
    let store = DiscoveredCompanyStore::open(dir.path()).unwrap();

    let name = Some("Stripe".to_string());
    store
        .upsert_batch(&[("greenhouse".into(), "stripe".into(), name, "scrape".into())])
        .unwrap();
    store.set_starred("greenhouse", "stripe", true).unwrap();
    store.set_starred("ashby", "Linear", true).unwrap(); // cold star → seed row
    store.set_starred("lever", "spotify", true).unwrap(); // board not selected below

    // Filter the selected boards to company-scoped ones via the SAME predicate.
    let selected: Vec<String> = ["greenhouse", "ashby", "linkedin"]
        .iter()
        .filter(|b| crate::scraping::boards::get(b).is_some_and(|s| s.requires_company()))
        .map(|s| s.to_string())
        .collect();

    let map = resolve_watched_companies(&store.watched(), &selected);

    assert_eq!(map.get("greenhouse"), Some(&vec!["stripe".to_string()]));
    // A cold-starred (materialized-seed) company still routes to its own board.
    assert_eq!(map.get("ashby"), Some(&vec!["Linear".to_string()]));
    // Non-company board filtered out; the unselected board's star is irrelevant.
    assert!(!map.contains_key("linkedin"));
    assert!(!map.contains_key("lever"));
}
