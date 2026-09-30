use super::super::super::test_support::*;
use super::super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(super) fn make_input(companies: Vec<String>) -> BoardSearchInput {
    BoardSearchInput {
        companies,
        ..default_search_input()
    }
}

pub(super) fn make_ctx() -> ScrapeContext {
    default_ctx()
}

// ---------------------------------------------------------------------------
// Scraper metadata
// ---------------------------------------------------------------------------

#[test]
fn test_workable_scraper_id() {
    assert_eq!(WorkableScraper.id(), "workable");
}

#[test]
fn test_workable_scraper_display_name() {
    assert_eq!(WorkableScraper.display_name(), "Workable");
}

#[test]
fn test_workable_scraper_mode() {
    assert_eq!(WorkableScraper.mode(), ScraperMode::Http);
}

#[test]
fn test_workable_requires_company() {
    assert!(
        WorkableScraper.requires_company(),
        "Workable is a company-scoped board and must return true for requires_company()"
    );
}

// ---------------------------------------------------------------------------
// Slug guard — is_valid_workable_slug (path-segment SSRF/traversal guard)
// ---------------------------------------------------------------------------

#[test]
fn slug_validation_accepts_valid_slugs() {
    assert!(is_valid_workable_slug("careers-at-sleek"));
    assert!(is_valid_workable_slug("acme123"));
    assert!(is_valid_workable_slug(&"a".repeat(63)));
}

#[test]
fn slug_validation_rejects_invalid_slugs() {
    assert!(
        !is_valid_workable_slug("acme/../secret"),
        "path traversal must be rejected"
    );
    assert!(
        !is_valid_workable_slug("acme?x=1"),
        "query injection must be rejected"
    );
    assert!(!is_valid_workable_slug("acme.corp"), "dot must be rejected");
    assert!(!is_valid_workable_slug("-acme"), "leading hyphen rejected");
    assert!(!is_valid_workable_slug("acme-"), "trailing hyphen rejected");
    assert!(!is_valid_workable_slug(""), "empty slug rejected");
    assert!(
        !is_valid_workable_slug(&"a".repeat(64)),
        "exceeds 63-char limit"
    );
}

/// Every curated `ats_seed` slug for this board must pass the production
/// path-segment guard — regression guard against a seed entry silently
/// drifting out of validator-compatible shape.
#[test]
fn ats_seed_workable_slugs_pass_the_guard() {
    let entries: Vec<_> = crate::scraping::boards::ats_seed::by_ats("workable").collect();
    assert!(!entries.is_empty(), "workable must have seed entries");
    for e in entries {
        assert!(
            is_valid_workable_slug(e.slug),
            "seed slug '{}' ({}) must pass is_valid_workable_slug",
            e.slug,
            e.company
        );
    }
}

// ---------------------------------------------------------------------------
// normalize_workable_companies — lowercase-before-dedup ordering
// ---------------------------------------------------------------------------

/// Case-only variants must collapse to ONE entry — dedup has to run on the
/// same casing the slug is lowercased to for the outbound request, not on
/// the raw input casing (which would let "Acme" and "acme" both survive and
/// fire two identical fetches for the same tenant).
#[test]
fn normalize_workable_companies_collapses_case_variants() {
    let input = vec!["Acme".to_string(), "acme".to_string(), "ACME".to_string()];
    let result = normalize_workable_companies(&input);
    assert_eq!(
        result,
        vec!["acme"],
        "case-only variants of the same slug must dedupe to one lowercase entry"
    );
}

/// Distinct slugs (differing by more than casing) are both kept, still
/// lowercased and in first-seen order.
#[test]
fn normalize_workable_companies_keeps_distinct_slugs_lowercased() {
    let input = vec!["Acme".to_string(), "Beta".to_string(), "acme".to_string()];
    let result = normalize_workable_companies(&input);
    assert_eq!(result, vec!["acme", "beta"]);
}

// ---------------------------------------------------------------------------
// URL guard — is_valid_workable_job_url (host-lock)
// ---------------------------------------------------------------------------

#[test]
fn url_guard_accepts_apply_workable_host_only() {
    assert!(is_valid_workable_job_url(
        "https://apply.workable.com/careers-at-sleek/j/ABCDEF/"
    ));
    assert!(
        !is_valid_workable_job_url("http://apply.workable.com/j/ABCDEF/"),
        "non-https must be rejected"
    );
    assert!(
        !is_valid_workable_job_url("https://evil.example/j/ABCDEF/"),
        "off-host url must be rejected"
    );
    assert!(
        !is_valid_workable_job_url("https://evil-apply.workable.com/j/ABCDEF/"),
        "lookalike host must be rejected (exact host match only)"
    );
    assert!(
        !is_valid_workable_job_url("not-a-url"),
        "unparseable url rejected"
    );
}

/// Embedded userinfo must be rejected even on the correct host — `host_str()`
/// ignores userinfo, so `is_valid_workable_job_url` needs its own explicit
/// username/password check (CodeRabbit finding on PR #535).
#[test]
fn url_guard_rejects_embedded_userinfo() {
    assert!(
        !is_valid_workable_job_url("https://spoof@apply.workable.com/j/ABC"),
        "userinfo on the correct host must still be rejected"
    );
    assert!(
        !is_valid_workable_job_url("https://spoof:pw@apply.workable.com/j/ABC"),
        "userinfo with a password must still be rejected"
    );
}

// ---------------------------------------------------------------------------
// parse_workable_date — RFC3339 and bare-date formats
// ---------------------------------------------------------------------------

#[test]
fn parse_workable_date_accepts_rfc3339_and_bare_date() {
    let rfc3339 = parse_workable_date("2024-03-15T00:00:00Z");
    assert!(rfc3339.is_some());
    let bare = parse_workable_date("2024-03-15");
    assert!(bare.is_some());
    assert_eq!(rfc3339, bare, "midnight RFC3339 and bare date must match");
    assert!(parse_workable_date("not-a-date").is_none());
}
