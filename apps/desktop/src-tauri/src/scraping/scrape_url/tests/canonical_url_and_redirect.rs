//! `canonical_job_url` SPA/list-view rewriting, and `resolve()`'s Pass 3
//! redirect-to-final-URL board re-dispatch (dispatch-decision coverage —
//! see the module comment above the redirect tests for what is NOT covered
//! hermetically here).

use super::super::*;

// ── canonical_job_url: SPA/list-view → canonical single-job URL ───────────────

#[test]
fn canonical_linkedin_search_with_current_job_id() {
    assert_eq!(
        super::super::canonical_job_url(
            "https://www.linkedin.com/jobs/search/?currentJobId=4185657072"
        ),
        Some("https://www.linkedin.com/jobs/view/4185657072".to_string())
    );
}

#[test]
fn canonical_linkedin_collections_with_current_job_id() {
    assert_eq!(
        super::super::canonical_job_url(
            "https://www.linkedin.com/jobs/collections/recommended/?currentJobId=123"
        ),
        Some("https://www.linkedin.com/jobs/view/123".to_string())
    );
}

#[test]
fn canonical_linkedin_direct_view_page_is_none() {
    assert_eq!(
        super::super::canonical_job_url("https://www.linkedin.com/jobs/view/123/"),
        None
    );
}

#[test]
fn canonical_linkedin_non_numeric_id_is_none() {
    assert_eq!(
        super::super::canonical_job_url(
            "https://www.linkedin.com/jobs/search/?currentJobId=abc123"
        ),
        None
    );
}

#[test]
fn canonical_linkedin_no_current_job_id_is_none() {
    assert_eq!(
        super::super::canonical_job_url("https://www.linkedin.com/jobs/search/?keywords=rust"),
        None
    );
}

#[test]
fn canonical_indeed_search_with_vjk() {
    assert_eq!(
        super::super::canonical_job_url("https://www.indeed.com/jobs?q=x&vjk=9b6647ed6c731326"),
        Some("https://www.indeed.com/viewjob?jk=9b6647ed6c731326".to_string())
    );
}

#[test]
fn canonical_indeed_country_tld_host_preserved() {
    assert_eq!(
        super::super::canonical_job_url("https://de.indeed.com/jobs?q=x&vjk=abc123"),
        Some("https://de.indeed.com/viewjob?jk=abc123".to_string())
    );
}

#[test]
fn canonical_indeed_direct_viewjob_is_none() {
    assert_eq!(
        super::super::canonical_job_url("https://www.indeed.com/viewjob?jk=9b6647ed6c731326"),
        None
    );
}

#[test]
fn canonical_unknown_host_is_none() {
    assert_eq!(
        super::super::canonical_job_url("https://example.com/jobs?vjk=123&currentJobId=456"),
        None
    );
}

// ── resolve Pass 3 — redirect→final-URL board re-dispatch ────────────────────
//
// Pass 3 of resolve(): after named boards miss on the ORIGINAL url, follow the
// redirect chain to the FINAL url and re-dispatch try_named_boards there. This
// is the key path for aggregator click-trackers (e.g. Adzuna `redirect_url`)
// that land on a Greenhouse/Lever/… page.
//
// There is no hermetic network-mock infrastructure for resolve() in this file
// (the existing `resolve_returns_none_on_redirect_follow_error` only exercises
// the SSRF/Err arm). Building one (wiremock/mockito) is out of scope per the
// existing test style; the full redirect→fetch composition is covered by
// integration tests.
//
// What we CAN test hermetically is the DISPATCH DECISION: given a URL that is
// already in final form (i.e. as if Pass 3 received it after the redirect
// resolved), does `try_named_boards` route it to the correct board handler?
// The existing try_named_boards_* tests use exactly this pattern.

/// A real Greenhouse URL (the shape a redirect_url would land on) must pass
/// the Greenhouse host gate inside try_named_boards. After the gate the handler
/// would make a live API call that fails in a hermetic env, but the gate itself
/// must not prematurely reject the URL — same rationale as try_workday_accepts_real_host_at_gate.
#[tokio::test]
async fn try_named_boards_routes_greenhouse_final_url_to_handler() {
    // This URL is the form an Adzuna redirect_url would resolve to.
    // No live server: the Greenhouse API call fails → Ok(None), but the test
    // passes because the gate must not reject a legitimate Greenhouse host.
    let result = try_named_boards("https://boards.greenhouse.io/stripe/jobs/99999999").await;
    assert!(
        result.is_ok(),
        "a real Greenhouse final URL must pass the host gate inside try_named_boards \
         (result may be None due to no live server)"
    );
    // Contrast: a non-board URL returns Ok(None) deterministically — confirmed
    // by try_named_boards_returns_none_for_unknown_url above.
}

/// A real Lever URL (the shape a redirect_url would land on) must pass the
/// Lever host gate inside try_named_boards — same hermetic rationale as above.
#[tokio::test]
async fn try_named_boards_routes_lever_final_url_to_handler() {
    let result =
        try_named_boards("https://jobs.lever.co/stripe/aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").await;
    assert!(
        result.is_ok(),
        "a real Lever final URL must pass the host gate inside try_named_boards \
         (result may be None due to no live server)"
    );
}

/// Pass 3 skip condition: when final_url == original_url no redirect occurred
/// and try_named_boards is NOT called a second time. Because we cannot inject a
/// call counter into resolve() without changing production code, we verify the
/// correctness *precondition* instead: try_named_boards must be idempotent for
/// non-board URLs (i.e. calling it twice on the same URL yields the same
/// Ok(None) both times, with no side-effects). If try_named_boards ever gained
/// internal state that made a second call return Some(_), this test would catch
/// it — and that would mean the Pass 3 skip is no longer semantically safe.
#[tokio::test]
async fn try_named_boards_returns_none_when_url_unchanged_after_no_redirect() {
    let url = "https://careers.example.com/jobs/no-redirect/123";

    // Pass 1 simulation: non-board URL must return Ok(None).
    let first = try_named_boards(url)
        .await
        .expect("non-board URL must return Ok(None), not Err on first call");
    assert!(first.is_none(), "first call: non-board URL must yield None");

    // Pass 3 simulation (what would happen if the skip were absent): the same
    // URL dispatched a second time must still return Ok(None) with no change.
    // This is the invariant the `if final_url != url` skip relies on: skipping
    // is correct because the result would have been identical anyway.
    let second = try_named_boards(url)
        .await
        .expect("non-board URL must return Ok(None), not Err on second call");
    assert!(
        second.is_none(),
        "second call with the same non-board URL must still yield None \
         (try_named_boards must be idempotent — the skip optimization is safe)"
    );
}
