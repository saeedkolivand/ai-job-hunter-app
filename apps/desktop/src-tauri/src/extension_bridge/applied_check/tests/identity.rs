use super::super::*;

use super::super::super::test_support::{app_meta, open_store};
use crate::applications::{ApplicationOrigin, ApplicationStatus};

// ── issue #1214 — the identity fallback (mirrors `agent_read::tests`'s
// `job_is_applied_*` family): the exact normalized-key lookup misses a tracked
// posting stored under a different host/path spelling, so
// `resolve_applied_check_url` falls back to a `(board, id)` identity match. ──

/// A posting stored under the regional `de.linkedin.com` host is found when
/// checked as `www.linkedin.com/jobs/view/<id>/` — a byte-exact
/// normalized-string compare can't bridge the host, the identity can. The
/// fallback must return the SAME shape the exact path does (not just `found`).
#[test]
fn resolve_applied_check_matches_a_regional_linkedin_host_by_identity() {
    let (_dir, store) = open_store();
    let id = store
        .upsert_for_origin(
            "https://de.linkedin.com/jobs/view/4185657072",
            "linkedin",
            &app_meta("Acme", "Staff Engineer"),
            ApplicationOrigin::Saved,
            Some(true),
        )
        .unwrap();

    let out = resolve_applied_check(
        &store,
        &json!({ "url": "https://www.linkedin.com/jobs/view/4185657072/" }),
    )
    .unwrap();
    assert!(out.found);
    assert_eq!(out.application_id.as_deref(), Some(id.as_str()));
    assert_eq!(out.status.as_deref(), Some("applied"));
    assert_eq!(out.title.as_deref(), Some("Staff Engineer"));
    assert!(
        out.applied_at.is_some(),
        "an applied row must carry applied_at on the identity path too"
    );
}

/// LinkedIn's slugged `/jobs/view/<slug>-<id>` form and the bare numeric
/// `/jobs/view/<id>` form resolve to the same identity in EITHER direction.
#[test]
fn resolve_applied_check_matches_a_slugged_linkedin_path_by_identity_both_directions() {
    let numeric = "https://www.linkedin.com/jobs/view/4185657072";
    let slugged = "https://www.linkedin.com/jobs/view/senior-engineer-at-acme-4185657072";

    for (stored, checked) in [(numeric, slugged), (slugged, numeric)] {
        let (_dir, store) = open_store();
        let id = store
            .upsert_for_origin(
                stored,
                "linkedin",
                &app_meta("Acme", "Staff Engineer"),
                ApplicationOrigin::Saved,
                Some(true),
            )
            .unwrap();

        let out = resolve_applied_check(&store, &json!({ "url": checked })).unwrap();
        assert!(
            out.found,
            "stored {stored} must be found when checked as {checked}"
        );
        assert_eq!(
            out.application_id.as_deref(),
            Some(id.as_str()),
            "the identity path must resolve to the SAME Application row"
        );
        assert_eq!(out.status.as_deref(), Some("applied"));
        assert_eq!(out.title.as_deref(), Some("Staff Engineer"));
    }
}

/// Identity matching must not become "any LinkedIn posting counts as
/// tracked": a DIFFERENT numeric id on the same board must still miss.
#[test]
fn resolve_applied_check_does_not_match_a_different_linkedin_id() {
    let (_dir, store) = open_store();
    store
        .upsert_for_origin(
            "https://www.linkedin.com/jobs/view/111",
            "linkedin",
            &app_meta("Acme", "Rust Engineer"),
            ApplicationOrigin::Saved,
            Some(true),
        )
        .unwrap();

    let out = resolve_applied_check(
        &store,
        &json!({ "url": "https://de.linkedin.com/jobs/view/senior-engineer-222" }),
    )
    .unwrap();
    assert!(!out.found);
    assert!(out.application_id.is_none());
    assert!(out.status.is_none());
}

/// A board `job_identity` does not cover still resolves EXACTLY as before: an
/// exact normalized-key match is found, a different url is not — the identity
/// fallback is additive, never a replacement.
#[test]
fn resolve_applied_check_without_an_identity_resolves_exactly_as_before() {
    let (_dir, store) = open_store();
    let stored = "https://boards.example.com/jobs/42";
    store
        .upsert_for_origin(
            stored,
            "boards.example.com",
            &app_meta("Acme", "Backend Engineer"),
            ApplicationOrigin::Saved,
            Some(true),
        )
        .unwrap();
    assert!(
        crate::scraping::scrape_url::job_identity(stored).is_none(),
        "boards.example.com must have no id extractor for this fixture to prove the fallback"
    );

    // Same (normalized) url → found through the exact path, as before.
    let out = resolve_applied_check(
        &store,
        &json!({ "url": "https://boards.example.com/jobs/42?utm_source=newsletter" }),
    )
    .unwrap();
    assert!(out.found);
    assert_eq!(out.status.as_deref(), Some("applied"));

    // A DIFFERENT url (no identity to fall back on) → still not found.
    let out = resolve_applied_check(
        &store,
        &json!({ "url": "https://boards.example.com/jobs/43" }),
    )
    .unwrap();
    assert!(!out.found);
}

/// The exact-match fast path is unchanged: a byte-identical normalized key is
/// found with ALL five fields populated — `found`, `applicationId`, `status`,
/// `title`, `appliedAt` — the same shape the identity path returns.
#[test]
fn resolve_applied_check_exact_match_still_returns_all_fields() {
    let (_dir, store) = open_store();
    let url = "https://www.linkedin.com/jobs/view/4185657072";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Staff Engineer"),
            ApplicationOrigin::Saved,
            Some(true),
        )
        .unwrap();

    let out = resolve_applied_check(&store, &json!({ "url": url })).unwrap();
    assert!(out.found);
    assert_eq!(out.application_id.as_deref(), Some(id.as_str()));
    assert_eq!(out.status.as_deref(), Some("applied"));
    assert_eq!(out.title.as_deref(), Some("Staff Engineer"));
    assert!(
        out.applied_at.is_some(),
        "an applied row must carry applied_at on the exact path too"
    );
}

/// The identity fallback's projection scan must rebuild the SAME FULL
/// [`crate::applications::Application`] the exact-key path would — a future
/// refactor that satisfied the scan with a half-built row (id + url only,
/// every other field defaulted) fails here, because every field is compared
/// through JSON, not just `found`.
#[test]
fn resolve_applied_check_identity_fallback_returns_the_same_full_row_as_the_exact_path() {
    let (_dir, store) = open_store();
    let url = "https://de.linkedin.com/jobs/view/4185657072";
    let id = store
        .upsert_for_origin(
            url,
            "linkedin",
            &app_meta("Acme", "Staff Engineer"),
            ApplicationOrigin::Saved,
            Some(true),
        )
        .unwrap();

    let exact = store
        .find_by_job_url(&normalize_job_url(url))
        .expect("the exact-key lookup finds the row");
    let identity = crate::scraping::scrape_url::job_identity(url)
        .expect("a linkedin url always has an identity");
    let by_identity = store
        .find_by_job_identity(&identity)
        .expect("the identity scan finds the same row");

    assert_eq!(
        serde_json::to_value(&by_identity).unwrap(),
        serde_json::to_value(&exact).unwrap(),
        "the identity scan must rebuild the FULL Application row the exact path returns — \
         a half-built projection row (id/url only) fails this comparison"
    );
    assert_eq!(by_identity.id, id);
    assert_eq!(by_identity.status, ApplicationStatus::Applied);
    assert_eq!(by_identity.title, "Staff Engineer");
}
