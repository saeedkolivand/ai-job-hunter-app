use super::super::*;
use super::support::*;
use crate::scraping::http::FetchOptions;

// ── FIX 1 (finding #4): retries=0 invariant via the real helper ──────────────

/// INVARIANT: the `FetchOptions` object that `ApifyLinkedInProvider::search`
/// actually constructs (via `apify_fetch_options`) must have `retries == 0`.
///
/// Unlike the old version that only checked the constant, this assertion runs
/// against the REAL helper that `search` calls.  If someone replaces
/// `retries: APIFY_RETRIES` with `retries: 2` inside the helper, this test
/// fails — where the old constant-only test would have stayed green.
#[test]
fn apify_fetch_options_must_have_retries_zero() {
    // Baseline: confirm the default is non-zero so the override below is meaningful.
    assert_eq!(
        FetchOptions::default().retries,
        2,
        "FetchOptions::default() retries is expected to be 2; update this test if the default changes"
    );
    // Assert on the ACTUAL options object the production search path constructs.
    let opts = apify_fetch_options("{}".to_string(), "test-token");
    assert_eq!(
        opts.retries, 0,
        "INVARIANT VIOLATED: apify_fetch_options must set retries=0 — \
         a retry would start another billed actor run"
    );
    // Belt: the shared constant itself must also remain 0.
    assert_eq!(APIFY_RETRIES, 0, "APIFY_RETRIES constant must be 0");
}

// ── FIX 2: cancellation mid-flight ──────────────────────────────────────────

/// A signal cancelled BEFORE the fetch_json call fires the tokio::select!
/// cancel arm immediately — no network call is issued. This tests the
/// `ApifyLinkedInProvider::search` path directly (not the higher-level guard in
/// `search_with_providers`), which is what FIX 2 adds.
#[tokio::test]
async fn apify_search_pre_cancelled_signal_returns_err() {
    let p = apify(Some("fake-token"), true);
    let signal = make_token();
    signal.cancel(); // pre-cancel before calling search

    let result = p
        .search("dev", "Berlin", "de", false, None, None, signal)
        .await;
    assert!(
        result.is_err(),
        "a pre-cancelled signal must make ApifyLinkedInProvider::search return Err"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("cancelled"),
        "error must indicate cancellation; got: {msg}"
    );
}

// ── FIX 3: platform-enforced cost cap in endpoint URL ───────────────────────

/// The run-sync endpoint must carry `maxItems` and `maxTotalChargeUsd` as query
/// params (server-side cost caps) and must NOT embed the Bearer token in the URL.
///
/// Calls the REAL `build_apify_endpoint(actor, max_items)` function that
/// production code calls, so any future refactor that drops either cap (or
/// buries the token in the URL) will break this test.
#[test]
fn apify_endpoint_url_has_max_items_and_no_token() {
    // Call the same function production uses — no local copy.
    // Pass APIFY_MAX_ITEMS as the cap to mirror the full-cap scenario.
    let endpoint = build_apify_endpoint(APIFY_DEFAULT_ACTOR, APIFY_MAX_ITEMS);

    assert!(
        endpoint.contains(&format!("maxItems={APIFY_MAX_ITEMS}")),
        "endpoint must include server-side maxItems={APIFY_MAX_ITEMS}; got: {endpoint}"
    );
    assert!(
        endpoint.contains(&format!("maxTotalChargeUsd={APIFY_MAX_CHARGE_USD}")),
        "endpoint must include maxTotalChargeUsd={APIFY_MAX_CHARGE_USD}; got: {endpoint}"
    );
    // Token must never appear in the URL (lives in the Authorization header only).
    assert!(
        !endpoint.to_lowercase().contains("token"),
        "Bearer token must not appear in the URL; got: {endpoint}"
    );
    // Apify host is fixed — no SSRF possible.
    assert!(
        endpoint.starts_with("https://api.apify.com/"),
        "endpoint must be on api.apify.com; got: {endpoint}"
    );
    // Actor id is embedded in the path.
    assert!(
        endpoint.contains(APIFY_DEFAULT_ACTOR),
        "endpoint must contain the actor id; got: {endpoint}"
    );
}

// ── FIX 4: actor-id validation ───────────────────────────────────────────────

/// Valid `user~actor` ids pass; malformed ids are rejected so they can't reach
/// the API path.
#[test]
fn apify_actor_id_validator_accepts_valid_rejects_malformed() {
    // Valid ids.
    assert!(
        is_valid_apify_actor_id("curious_coder~linkedin-jobs-scraper"),
        "default actor id must be valid"
    );
    assert!(
        is_valid_apify_actor_id("user123~actor-name.v2"),
        "alphanumeric + hyphen + dot must be valid"
    );
    assert!(
        is_valid_apify_actor_id("a~b"),
        "single-char parts must be valid"
    );

    // No tilde → invalid.
    assert!(!is_valid_apify_actor_id("nousernamehere"));
    // Empty user part.
    assert!(!is_valid_apify_actor_id("~actor"));
    // Empty actor part.
    assert!(!is_valid_apify_actor_id("user~"));
    // Path-traversal chars rejected.
    assert!(!is_valid_apify_actor_id("../../etc/passwd"));
    assert!(!is_valid_apify_actor_id("user~actor/path"));
    // Spaces rejected.
    assert!(!is_valid_apify_actor_id("user ~actor"));
    // Two tildes rejected (actor part contains `~` which is not in [A-Za-z0-9_.-]).
    assert!(!is_valid_apify_actor_id("user~act~or"));
}

/// The default actor constant must always pass its own validator.
#[test]
fn apify_default_actor_passes_validator() {
    assert!(
        is_valid_apify_actor_id(APIFY_DEFAULT_ACTOR),
        "APIFY_DEFAULT_ACTOR must pass is_valid_apify_actor_id; got: {APIFY_DEFAULT_ACTOR}"
    );
}

// ── FIX 5: dedupe_by_url strips LinkedIn tracking params ────────────────────

/// Two LinkedIn URLs for the same job that differ only by tracking params
/// (`?trk=…`, `?refId=…`) must collapse to one entry in `dedupe_by_url`.
/// Non-LinkedIn URLs with distinct query strings must remain distinct (some
/// boards encode the job id in the query).
#[test]
fn dedupe_by_url_strips_linkedin_tracking_params() {
    let make_posting = |url: &str| JobPosting {
        id: url.to_string(),
        external_id: Some(url.to_string()),
        title: "Engineer".to_string(),
        company: "Co".to_string(),
        location: None,
        url: url.to_string(),
        source: "aggregator".to_string(),
        description: None,
        requirements: None,
        posted_at: None,
        captured_at: 0,
        extra: std::collections::HashMap::new(),
    };

    // Same LinkedIn job, two different tracking param variants.
    let li1 = make_posting("https://www.linkedin.com/jobs/view/123?trk=organic");
    let li2 = make_posting("https://www.linkedin.com/jobs/view/123?refId=abc&trk=xyz");
    // Non-LinkedIn: different query → distinct jobs (must NOT be merged).
    let other1 = make_posting("https://example.com/jobs?id=1");
    let other2 = make_posting("https://example.com/jobs?id=2");

    let deduped = dedupe_by_url(vec![li1, li2, other1, other2]);
    assert_eq!(
        deduped.len(),
        3,
        "two LinkedIn tracking-param variants should collapse to 1; \
         two non-LinkedIn distinct-query URLs stay separate; expected 3 total"
    );
    let urls: Vec<&str> = deduped.iter().map(|p| p.url.as_str()).collect();
    assert!(
        urls.contains(&"https://example.com/jobs?id=1"),
        "non-LinkedIn URL with id=1 must be kept"
    );
    assert!(
        urls.contains(&"https://example.com/jobs?id=2"),
        "non-LinkedIn URL with id=2 must be kept"
    );
}

/// `canonical_url` only strips query on `linkedin.com` or `*.linkedin.com` —
/// a dot boundary is required so look-alike domains like `evillinkedin.com`
/// (no dot before `linkedin.com`) and `linkedin.example.com` are left intact.
#[test]
fn canonical_url_only_strips_linkedin_dot_com_hosts() {
    // Exact linkedin.com apex → query stripped.
    let apex = canonical_url("https://linkedin.com/jobs/view/1?trk=foo");
    assert!(
        !apex.contains('?'),
        "query must be stripped for apex linkedin.com; got: {apex}"
    );

    // Real www.linkedin.com subdomain → query stripped.
    let li = canonical_url("https://www.linkedin.com/jobs/view/1?trk=foo");
    assert!(
        !li.contains('?'),
        "query must be stripped for www.linkedin.com; got: {li}"
    );

    // A host that ends with "linkedin.com" but has no dot boundary (`evillinkedin.com`)
    // must NOT be treated as LinkedIn — its query is preserved.
    let evil = canonical_url("https://evillinkedin.com/jobs/1?id=99");
    assert!(
        evil.contains("id=99"),
        "query must NOT be stripped for evillinkedin.com (no dot boundary); got: {evil}"
    );

    // A host that merely *contains* the substring "linkedin" in the wrong position
    // (`linkedin.example.com`) → NOT stripped.
    let fake = canonical_url("https://linkedin.example.com/jobs/1?id=1");
    assert!(
        fake.contains("id=1"),
        "query must NOT be stripped for linkedin.example.com; got: {fake}"
    );

    // Non-LinkedIn host → query kept.
    let other = canonical_url("https://example.com/jobs?id=42");
    assert!(
        other.contains("id=42"),
        "query must be kept for non-LinkedIn host; got: {other}"
    );
}

/// Cancellation before the call → no provider runs, including the paid Apify one.
#[tokio::test]
async fn apify_not_run_after_cancellation() {
    let signal = make_token();
    signal.cancel();

    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok(
            "adzuna",
            vec![sample_posting("1", "adzuna")],
        )),
        Box::new(FakeProvider::ok(
            "apify_linkedin",
            vec![sample_posting("2", "linkedin")],
        )),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::manual(100),
        signal,
    )
    .await
    .unwrap();

    assert!(
        result.is_empty(),
        "a cancelled signal must prevent any (especially the paid) provider call"
    );
}
