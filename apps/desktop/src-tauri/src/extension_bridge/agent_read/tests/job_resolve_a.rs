//! Tests for `resolve_job`/`job_is_applied`'s identity-based matching (`job/resolve.rs`).

use crate::autopilot::{Autopilot, FoundJob};

use super::super::job::resolve::{job_is_applied, resolve_job};
use super::support::{blank_autopilot, full_found_job};

#[test]
fn resolve_job_finds_by_normalized_url_across_autopilots() {
    let records = vec![Autopilot {
        found_jobs: vec![full_found_job()],
        ..blank_autopilot("ap-1")
    }];
    let normalized =
        crate::applications::normalize_job_url("https://boards.example.com/jobs/42?utm_source=x");
    let out = resolve_job(
        &records,
        None,
        &normalized,
        &std::collections::HashSet::new(),
    )
    .expect("found");
    // `title` is now fenced too (`fence_posting_display_fields`) — this test is about the
    // URL-matching lookup, not fencing (see the dedicated fencing test below), so it only
    // checks the real content survived, not the exact wrapper.
    assert!(out["title"].as_str().unwrap().contains("Backend Engineer"));
}

/// Issue #1166/#1169 (HIGH) — `job`'s `applied` must be DERIVED off
/// `applied_urls`, never a plain passthrough of the stored `FoundJob::applied`
/// (which is always `false` on the stored record — see that field's own
/// doc). This fails against the pre-fix `resolve_job`, which ignored the
/// `applied_urls` set entirely and echoed the stored (always-`false`) bit.
#[test]
fn resolve_job_derives_applied_from_the_applied_urls_set_not_the_stored_bit() {
    let stored_url = "https://boards.example.com/jobs/42";
    let records = vec![Autopilot {
        found_jobs: vec![FoundJob {
            url: stored_url.to_string(),
            applied: false, // the stored bit — deliberately the OPPOSITE of the derived answer
            ..full_found_job()
        }],
        ..blank_autopilot("ap-1")
    }];
    let normalized = crate::applications::normalize_job_url(stored_url);
    let mut applied_urls = std::collections::HashSet::new();
    applied_urls.insert(crate::applications::normalize_job_url(stored_url));

    let applied_out = resolve_job(&records, None, &normalized, &applied_urls).expect("found");
    assert_eq!(
        applied_out["applied"], true,
        "a url present in applied_urls must read as applied, even though the stored bit is false"
    );

    let not_applied_out = resolve_job(
        &records,
        None,
        &normalized,
        &std::collections::HashSet::new(),
    )
    .expect("found");
    assert_eq!(
        not_applied_out["applied"], false,
        "a url absent from applied_urls must read as not applied"
    );
}

// ── round-4 advisory findings (PR #1182): T3/T4 ───────────────────────────

/// T4 — a job stored under a regional LinkedIn host must still read as
/// applied when the application was recorded under the bare `linkedin.com`
/// spelling for the SAME numeric id; a byte-exact normalized-string compare
/// cannot bridge that, `job_identity` can.
#[test]
fn job_is_applied_matches_a_regional_linkedin_host_by_identity() {
    let mut applied_urls = std::collections::HashSet::new();
    applied_urls.insert(crate::applications::normalize_job_url(
        "https://www.linkedin.com/jobs/view/4185657072",
    ));
    assert!(job_is_applied(
        "https://de.linkedin.com/jobs/view/4185657072",
        &applied_urls
    ));
}

/// T4 — the numeric-only `/jobs/view/<id>` form and LinkedIn's slugged form
/// must resolve to the same identity in either direction.
#[test]
fn job_is_applied_matches_a_slugged_linkedin_path_by_identity() {
    let mut applied_urls = std::collections::HashSet::new();
    applied_urls.insert(crate::applications::normalize_job_url(
        "https://www.linkedin.com/jobs/view/4185657072",
    ));
    assert!(job_is_applied(
        "https://www.linkedin.com/jobs/view/senior-engineer-at-acme-4185657072",
        &applied_urls
    ));
}

/// T4 — an application recorded from a `currentJobId=` search/SPA-view
/// spelling must still match: `import_flow::import_job`'s own pipeline
/// rewrites that spelling to the canonical `/jobs/view/<id>` form via
/// `canonical_job_url` BEFORE `normalize_job_url` ever runs, so this is
/// exactly what `ApplicationStore::applied_job_urls` holds for it — this
/// test drives the SAME two calls in the SAME order to stay honest about
/// what is actually stored.
#[test]
fn job_is_applied_matches_a_current_job_id_recorded_application_by_identity() {
    let current_job_id_url = "https://www.linkedin.com/jobs/search/?currentJobId=4185657072";
    let canonical =
        crate::scraping::scrape_url::canonical_job_url(current_job_id_url).expect("rewritten");
    let mut applied_urls = std::collections::HashSet::new();
    applied_urls.insert(crate::applications::normalize_job_url(&canonical));

    // The FOUND job's own stored spelling differs (regional host, slugged
    // path) from the recorded application's — a byte-exact normalized-string
    // compare would miss it; only identity bridges the two.
    assert!(job_is_applied(
        "https://de.linkedin.com/jobs/view/senior-engineer-4185657072",
        &applied_urls
    ));
}

/// T4 — identity matching must not turn into "any LinkedIn job counts as
/// applied": a different numeric id on the same board must still miss.
#[test]
fn job_is_applied_does_not_match_a_different_linkedin_id() {
    let mut applied_urls = std::collections::HashSet::new();
    applied_urls.insert(crate::applications::normalize_job_url(
        "https://www.linkedin.com/jobs/view/111",
    ));
    assert!(!job_is_applied(
        "https://www.linkedin.com/jobs/view/222",
        &applied_urls
    ));
}

/// T4-cont (PR #1182 round-5 fix) — a stored job url and its recorded
/// application can share the SAME percent-escaped spelling (e.g. `%2D`) on a
/// board `job_identity` doesn't cover (only linkedin/indeed have a stable id
/// space). Decoding only the found-job side before comparing broke this:
/// `applied_urls` is keyed by `normalize_job_url(raw)`, never decoded, so the
/// decoded job url no longer byte-matched the raw-spelling entry, and with no
/// identity fallback for this board the lookup fell straight to `false`.
#[test]
fn job_is_applied_matches_the_same_percent_escaped_spelling_without_decoding() {
    let raw_url = "https://boards.example.com/jobs/senior%2Dengineer";
    let mut applied_urls = std::collections::HashSet::new();
    applied_urls.insert(crate::applications::normalize_job_url(raw_url));
    assert!(job_is_applied(raw_url, &applied_urls));
}
