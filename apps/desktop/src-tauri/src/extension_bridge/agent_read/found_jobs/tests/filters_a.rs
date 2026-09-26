//! Server-side filter tests (issue #1167), part A: `minScore`/`country`/`remote`/`applied`.

use super::super::*;
use super::support::*;
use crate::autopilot::FoundJob;
use std::collections::HashSet;

#[test]
fn found_jobs_min_score_filter_excludes_lower_and_unscored_rows() {
    let low = FoundJob {
        score: Some(50.0),
        ..numbered_job(1)
    };
    let high = FoundJob {
        score: Some(90.0),
        ..numbered_job(2)
    };
    let unscored = FoundJob {
        score: None,
        ..numbered_job(3)
    };
    let records = vec![autopilot_with_jobs("ap-1", vec![low, high, unscored])];
    let filters = FoundJobsFilters::from_payload(&json!({ "minScore": 70 })).unwrap();
    let out = resolve_found_jobs(&records, Some("ap-1"), &filters, &no_applied(), 0, 20).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(out["jobs"][0]["url"], "https://boards.example.com/jobs/2");
}

#[test]
fn found_jobs_country_filter_matches_location_case_insensitively() {
    let berlin = FoundJob {
        location: Some("Berlin, Germany".to_string()),
        ..numbered_job(1)
    };
    let paris = FoundJob {
        location: Some("Paris, France".to_string()),
        ..numbered_job(2)
    };
    let records = vec![autopilot_with_jobs("ap-1", vec![berlin, paris])];
    let filters = FoundJobsFilters::from_payload(&json!({ "country": "GERMANY" })).unwrap();
    let out = resolve_found_jobs(&records, Some("ap-1"), &filters, &no_applied(), 0, 20).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(out["jobs"][0]["url"], "https://boards.example.com/jobs/1");
}

#[test]
fn found_jobs_remote_filter_reuses_the_scrape_time_marker_list() {
    let remote = FoundJob {
        location: Some("Remote (Worldwide)".to_string()),
        ..numbered_job(1)
    };
    let onsite = FoundJob {
        location: Some("Berlin, Germany".to_string()),
        ..numbered_job(2)
    };
    let records = vec![autopilot_with_jobs(
        "ap-1",
        vec![remote.clone(), onsite.clone()],
    )];
    let remote_only = FoundJobsFilters::from_payload(&json!({ "remote": true })).unwrap();
    let out =
        resolve_found_jobs(&records, Some("ap-1"), &remote_only, &no_applied(), 0, 20).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(out["jobs"][0]["url"], "https://boards.example.com/jobs/1");

    let records = vec![autopilot_with_jobs("ap-1", vec![remote, onsite])];
    let onsite_only = FoundJobsFilters::from_payload(&json!({ "remote": false })).unwrap();
    let out =
        resolve_found_jobs(&records, Some("ap-1"), &onsite_only, &no_applied(), 0, 20).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(out["jobs"][0]["url"], "https://boards.example.com/jobs/2");
}

/// Round-3 fix (H1): an all-remote board (WeWorkRemotely/RemoteOK/Remotive/
/// Jobicy) stores `location: None` or a jurisdiction string with no marker
/// word ("USA Only" from Remotive's `candidate_required_location`) — the
/// board's OWN `board_remote` classification (`JobPosting.extra["remote"]`,
/// persisted at `build_found_job` time) must count as remote even when
/// `location` text alone gives no signal, mirroring `location_verdict`'s
/// `board_remote` short-circuit. Before the fix this OR was missing
/// entirely, so `--remote true` silently dropped these rows and
/// `--remote false` returned them as on-site.
#[test]
fn found_jobs_remote_filter_also_trusts_the_boards_own_remote_flag() {
    let no_location = FoundJob {
        location: None,
        board_remote: true,
        ..numbered_job(1)
    };
    let jurisdiction_only = FoundJob {
        location: Some("USA Only".to_string()),
        board_remote: true,
        ..numbered_job(2)
    };
    let onsite = FoundJob {
        location: Some("Berlin, Germany".to_string()),
        board_remote: false,
        ..numbered_job(3)
    };
    let records = vec![autopilot_with_jobs(
        "ap-1",
        vec![
            no_location.clone(),
            jurisdiction_only.clone(),
            onsite.clone(),
        ],
    )];
    let remote_only = FoundJobsFilters::from_payload(&json!({ "remote": true })).unwrap();
    let out =
        resolve_found_jobs(&records, Some("ap-1"), &remote_only, &no_applied(), 0, 20).unwrap();
    assert_eq!(out["total"], 2);
    let urls: Vec<&str> = out["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| j["url"].as_str().unwrap())
        .collect();
    assert!(urls.contains(&"https://boards.example.com/jobs/1"));
    assert!(urls.contains(&"https://boards.example.com/jobs/2"));

    let records = vec![autopilot_with_jobs(
        "ap-1",
        vec![no_location, jurisdiction_only, onsite],
    )];
    let onsite_only = FoundJobsFilters::from_payload(&json!({ "remote": false })).unwrap();
    let out =
        resolve_found_jobs(&records, Some("ap-1"), &onsite_only, &no_applied(), 0, 20).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(out["jobs"][0]["url"], "https://boards.example.com/jobs/3");
}

#[test]
fn found_jobs_applied_filter_matches_the_derived_applied_set() {
    let applied_job = numbered_job(1);
    let unapplied_job = numbered_job(2);
    let records = vec![autopilot_with_jobs(
        "ap-1",
        vec![applied_job.clone(), unapplied_job],
    )];
    let mut applied_urls = HashSet::new();
    applied_urls.insert(crate::applications::normalize_job_url(&applied_job.url));
    let filters = FoundJobsFilters::from_payload(&json!({ "applied": true })).unwrap();
    let out = resolve_found_jobs(&records, Some("ap-1"), &filters, &applied_urls, 0, 20).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(out["jobs"][0]["url"], "https://boards.example.com/jobs/1");
    assert_eq!(out["jobs"][0]["applied"], true);
}

/// Round-4 perf fix (PR #1182 round-5) — `candidate_jobs` derives `applied`
/// through the precomputed-identity path (`agent_read::job_is_applied_indexed`),
/// not the per-call scan `agent_read::job_is_applied` uses; this must still
/// bridge a regional LinkedIn host to a bare `linkedin.com` application the
/// same way `job_is_applied`'s own identity fallback does — the two must never
/// disagree about the same job (see that fn's own doc).
#[test]
fn found_jobs_applied_matches_a_regional_linkedin_host_through_the_indexed_path() {
    let job = FoundJob {
        url: "https://de.linkedin.com/jobs/view/4185657072".to_string(),
        ..numbered_job(1)
    };
    let records = vec![autopilot_with_jobs("ap-1", vec![job])];
    let mut applied_urls = HashSet::new();
    applied_urls.insert(crate::applications::normalize_job_url(
        "https://www.linkedin.com/jobs/view/4185657072",
    ));
    let out =
        resolve_found_jobs(&records, Some("ap-1"), &no_filters(), &applied_urls, 0, 20).unwrap();
    assert_eq!(out["jobs"][0]["applied"], true);
}
