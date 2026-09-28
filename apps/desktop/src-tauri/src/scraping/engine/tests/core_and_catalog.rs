//! `ScraperEngine::catalog`/`health`/`set_concurrency` + job-registry
//! (`cancel`/`register_token`) tests.

use super::super::*;

#[test]
fn test_catalog() {
    let engine = ScraperEngine::new();
    let catalog = engine.catalog();
    assert_eq!(catalog.len(), 25);

    // Check specific scrapers
    assert!(catalog.iter().any(|s| s.id == "linkedin"));
    assert!(catalog.iter().any(|s| s.id == "ycombinator"));
    assert!(catalog.iter().any(|s| s.id == "aggregator"));
    assert!(catalog.iter().any(|s| s.id == "freehire"));
    assert!(catalog.iter().any(|s| s.id == "greenhouse"));
    // The 25-count alone doesn't prove which ids make it up — assert the
    // newest boards are actually present, not just that *some* 25 ids are.
    assert!(catalog.iter().any(|s| s.id == "workable"));
    assert!(catalog.iter().any(|s| s.id == "comeet"));
    assert!(catalog.iter().any(|s| s.id == "jobicy"));

    // Retired anti-bot boards must not appear in the catalog.
    assert!(!catalog.iter().any(|s| s.id == "indeed"));
    assert!(!catalog.iter().any(|s| s.id == "glassdoor"));
    assert!(!catalog.iter().any(|s| s.id == "xing"));
    assert!(!catalog.iter().any(|s| s.id == "workday"));
    assert!(!catalog.iter().any(|s| s.id == "stepstone"));
}

#[test]
fn test_catalog_supports_location_flags() {
    // Verified catalog (trust PR F): only boards that consume the requested
    // location SERVER-SIDE claim support. Everything else falls back to the
    // conservative central post-filter, so it must report false.
    let engine = ScraperEngine::new();
    let catalog = engine.catalog();
    let entry = |id: &str| {
        catalog
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing board: {id}"))
    };

    // Server-side location consumers (verified by reading each `search()`).
    assert!(
        entry("aggregator").supports_location,
        "aggregator routes market + `where`"
    );
    assert!(
        entry("linkedin").supports_location,
        "linkedin resolves geoId + distance"
    );
    assert!(
        entry("arbeitsagentur").supports_location,
        "arbeitsagentur sends `wo`"
    );

    // Boards that ignore location or only filter it client-side must be false.
    for id in [
        "remotive",
        "remoteok",
        "wwr",
        "themuse",
        "germantechjobs",
        "arbeitnow",
        "ycombinator",
        "greenhouse",
        "lever",
        "comeet",
    ] {
        assert!(
            !entry(id).supports_location,
            "{id} must not claim server-side location support"
        );
    }
}

#[test]
fn test_catalog_seeded_companies() {
    let engine = ScraperEngine::new();
    let catalog = engine.catalog();
    let entry = |id: &str| {
        catalog
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing board: {id}"))
    };

    // Seeded ATS board: non-empty, carries a known curated company.
    assert!(
        entry("greenhouse")
            .seeded_companies
            .iter()
            .any(|c| c == "Stripe"),
        "greenhouse must include the curated 'Stripe' seed"
    );

    // Non-ATS board: no curated seed, must be empty.
    assert!(
        entry("aggregator").seeded_companies.is_empty(),
        "aggregator has no curated company seed"
    );
}

#[test]
fn test_catalog_auth_tiers() {
    use crate::scraping::types::AuthRequirement;

    let engine = ScraperEngine::new();
    let catalog = engine.catalog();

    let entry = |id: &str| {
        catalog
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing board: {id}"))
    };

    // Optional — guest works; login enriches
    assert_eq!(
        entry("linkedin").auth,
        AuthRequirement::Optional,
        "linkedin must be Optional"
    );

    // Guest default — no override needed
    assert_eq!(
        entry("greenhouse").auth,
        AuthRequirement::Guest,
        "greenhouse must be Guest (default)"
    );
    assert_eq!(
        entry("ycombinator").auth,
        AuthRequirement::Guest,
        "ycombinator must be Guest (default)"
    );
    assert_eq!(
        entry("arbeitsagentur").auth,
        AuthRequirement::Guest,
        "arbeitsagentur must be Guest (default)"
    );
}

#[test]
fn test_catalog_requires_company_flags() {
    let engine = ScraperEngine::new();
    let catalog = engine.catalog();

    let entry = |id: &str| {
        catalog
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing board: {id}"))
    };

    // The 11 ATS boards must declare requires_company = true.
    for ats_id in &[
        "greenhouse",
        "lever",
        "ashby",
        "recruitee",
        "personio",
        "smartrecruiters",
        "pinpoint",
        "rippling",
        "breezy",
        "bamboohr",
        "workable",
    ] {
        assert!(
            entry(ats_id).requires_company,
            "ATS board '{ats_id}' must have requires_company=true"
        );
    }

    // All other boards must keep the default false.
    for non_ats_id in &[
        "linkedin",
        "ycombinator",
        "remoteok",
        "remotive",
        "arbeitnow",
        "themuse",
        "wwr",
        "berlinstartupjobs",
        "germantechjobs",
        "arbeitsagentur",
        "aggregator",
        "comeet",
    ] {
        assert!(
            !entry(non_ats_id).requires_company,
            "board '{non_ats_id}' must have requires_company=false (default)"
        );
    }
}

#[test]
fn test_catalog_listed_flags() {
    let engine = ScraperEngine::new();
    let catalog = engine.catalog();

    let entry = |id: &str| {
        catalog
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing board: {id}"))
    };

    // Representative boards across auth tiers are listed
    assert!(entry("greenhouse").listed, "greenhouse must be listed");
    assert!(entry("linkedin").listed, "linkedin must be listed");
    assert!(entry("ycombinator").listed, "ycombinator must be listed");
    assert!(
        entry("arbeitsagentur").listed,
        "arbeitsagentur must be listed"
    );

    // Comeet is registered (dispatchable) but HIDDEN from the picker until its
    // response shape is live-verified (trust PR G) — it must still be present in
    // the catalog, just with `listed = false`.
    assert!(
        catalog.iter().any(|e| e.id == "comeet"),
        "comeet must stay registered (dispatchable)"
    );
    assert!(
        !entry("comeet").listed,
        "comeet must be hidden from the picker until live-verified"
    );

    // Every board except the hidden Comeet is listed (25 registered, 1 hidden).
    let listed_count = catalog.iter().filter(|e| e.listed).count();
    assert_eq!(
        listed_count,
        catalog.len() - 1,
        "all boards except the hidden Comeet should be listed"
    );
    assert_eq!(
        listed_count, 24,
        "24 of the 25 registered boards are listed"
    );
}

#[test]
fn test_health() {
    let engine = ScraperEngine::new();
    let health = engine.health();
    assert_eq!(health.mode, "in-process");
    assert!(health.ready);
    assert_eq!(health.scrapers.len(), 25);
}

#[test]
fn test_set_concurrency() {
    let engine = ScraperEngine::new();
    engine.set_concurrency(1); // low-memory tier
    engine.set_concurrency(2); // balanced
    engine.set_concurrency(4); // performance
    engine.set_concurrency(0); // clamps to >= 1, must not panic
}

#[tokio::test]
async fn test_token_registration() {
    let engine = ScraperEngine::new();
    let token = tokio_util::sync::CancellationToken::new();

    engine.register_token("job-1", token.clone()).await;
    engine.unregister_token("job-1").await;
}

#[tokio::test]
async fn test_cancel_nonexistent_job() {
    let engine = ScraperEngine::new();
    // Should not panic for nonexistent job
    engine.cancel("nonexistent").await;
}
