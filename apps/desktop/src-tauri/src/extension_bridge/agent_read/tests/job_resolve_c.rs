//! Tests for `resolve_job`'s url-variant matching, the job-miss detail, and `job_lookup_key` (`job.rs`, `job/resolve.rs`).

use crate::autopilot::{Autopilot, FoundJob};

use super::super::job::resolve::{resolve_job, JOB_NOT_FOUND_MESSAGE};
use super::super::job::{job_caller_identity, job_lookup_key};
use super::super::reply::{agent_result_reply, error_detail};
use super::super::*;
use super::support::{blank_autopilot, full_found_job};

/// The issue #1128 repro, both directions. The caller's url goes through
/// the REAL caller-side pipeline ([`job_lookup_key`]) and the stored url
/// through [`resolve_job`]'s own compare, so this fails if EITHER half
/// stops decoding — a one-sided fix would leave the mirror image broken.
#[test]
fn resolve_job_matches_a_percent_encoded_variant_of_the_same_url() {
    let plain = "https://de.linkedin.com/jobs/view/ai-software-engineer-at-hyra-4464018189";
    let encoded =
        "https://de.linkedin.com/jobs/view/ai%2Dsoftware%2Dengineer%2Dat%2Dhyra%2D4464018189";

    for (stored, looked_up) in [(plain, encoded), (encoded, plain)] {
        let records = vec![Autopilot {
            found_jobs: vec![FoundJob {
                url: stored.to_string(),
                ..full_found_job()
            }],
            ..blank_autopilot("ap-1")
        }];
        let out = resolve_job(
            &records,
            job_caller_identity(looked_up),
            &job_lookup_key(looked_up),
            &std::collections::HashSet::new(),
        )
        .unwrap_or_else(|e| panic!("stored {stored} must match {looked_up}: {e}"));
        assert!(out["title"].as_str().unwrap().contains("Backend Engineer"));
    }
}

/// Issue #1166's own repro table: every url below must resolve to the SAME
/// stored posting by `(board, id)` identity, not a byte-exact string match.
/// Drives the real caller-side pipeline (`job_caller_identity` +
/// `job_lookup_key`, the exact two calls `job_resource` makes) against ONE
/// fixed stored url.
#[test]
fn resolve_job_matches_every_linkedin_url_variant_by_identity() {
    let stored = "https://www.linkedin.com/jobs/view/4464018189";
    let records = vec![Autopilot {
        found_jobs: vec![FoundJob {
            url: stored.to_string(),
            ..full_found_job()
        }],
        ..blank_autopilot("ap-1")
    }];
    let variants = [
        stored,
        "https://www.linkedin.com/jobs/view/4464018189/",
        "https://www.linkedin.com/jobs/view/4464018189?trk=abc&refId=z",
        "https://linkedin.com/jobs/view/4464018189",
        "https://de.linkedin.com/jobs/view/4464018189",
        "https://uk.linkedin.com/jobs/view/senior-engineer-4464018189",
        "https://www.linkedin.com/jobs/search/?currentJobId=4464018189",
        "http://www.linkedin.com/jobs/view/4464018189",
        "www.linkedin.com/jobs/view/4464018189",
    ];
    for caller_url in variants {
        let out = resolve_job(
            &records,
            job_caller_identity(caller_url),
            &job_lookup_key(caller_url),
            &std::collections::HashSet::new(),
        )
        .unwrap_or_else(|e| panic!("{caller_url} must resolve to the stored posting: {e}"));
        assert!(
            out["title"].as_str().unwrap().contains("Backend Engineer"),
            "{caller_url} resolved to the wrong posting"
        );
    }
}

#[test]
fn resolve_job_does_not_match_a_different_linkedin_id() {
    let records = vec![Autopilot {
        found_jobs: vec![FoundJob {
            url: "https://www.linkedin.com/jobs/view/111".to_string(),
            ..full_found_job()
        }],
        ..blank_autopilot("ap-1")
    }];
    let caller_url = "https://de.linkedin.com/jobs/view/222";
    let err = resolve_job(
        &records,
        job_caller_identity(caller_url),
        &job_lookup_key(caller_url),
        &std::collections::HashSet::new(),
    )
    .unwrap_err();
    assert_eq!(err.to_string(), JOB_NOT_FOUND_MESSAGE);
}

/// A board with no id extractor (`job_identity` returns `None` for both
/// halves) must still resolve through the pre-#1166 normalized-string
/// fallback — the identity compare is additive, never a replacement.
#[test]
fn resolve_job_matches_a_non_identity_board_by_normalized_string_only() {
    let stored = "https://boards.example.com/jobs/42";
    let records = vec![Autopilot {
        found_jobs: vec![FoundJob {
            url: stored.to_string(),
            ..full_found_job()
        }],
        ..blank_autopilot("ap-1")
    }];
    let caller_url = "https://www.boards.example.com/jobs/42/?utm_source=newsletter";
    assert!(
        job_caller_identity(caller_url).is_none(),
        "boards.example.com has no id extractor"
    );
    let out = resolve_job(
        &records,
        job_caller_identity(caller_url),
        &job_lookup_key(caller_url),
        &std::collections::HashSet::new(),
    )
    .expect("must still match by normalized string alone");
    assert!(out["title"].as_str().unwrap().contains("Backend Engineer"));
}

#[test]
fn resolve_job_miss_carries_a_detail_naming_best_matches_and_found_jobs() {
    let detail = error_detail(RES_JOB, JOB_NOT_FOUND_MESSAGE).expect("detail present");
    assert!(detail.contains("best-matches"));
    assert!(detail.contains("found-jobs"));
}

#[test]
fn agent_result_reply_attaches_the_job_miss_detail_on_the_wire() {
    let reply = agent_result_reply(
        "req-1",
        RES_JOB,
        Err(AppError::Validation(JOB_NOT_FOUND_MESSAGE.to_string())),
    );
    let parsed: Value = serde_json::from_str(&reply).unwrap();
    let detail = parsed["payload"]["detail"]
        .as_str()
        .expect("detail present on the wire");
    assert!(detail.contains("best-matches"));
    assert!(detail.contains("found-jobs"));
}

#[test]
fn error_detail_is_none_for_an_unrelated_refusal() {
    assert!(error_detail(RES_JOB, "url is required").is_none());
    assert!(error_detail(RES_PROFILE, JOB_NOT_FOUND_MESSAGE).is_none());
}

/// The scheme guard must still see what a browser would: the decode runs
/// BEFORE `normalize_job_url`, so `%6A` becoming `j` turns this into the
/// `javascript:` url the guard rejects, rather than a scheme-less string
/// that slips past a raw-byte check.
#[test]
fn job_lookup_key_still_refuses_a_percent_encoded_javascript_scheme() {
    assert_eq!(job_lookup_key("%6Aavascript:alert(1)"), "");
}

#[test]
fn resolve_job_refuses_with_fixed_sentinel_when_absent() {
    let err = resolve_job(
        &[],
        None,
        "https://nowhere.example.com/x",
        &std::collections::HashSet::new(),
    )
    .unwrap_err();
    assert_eq!(err.to_string(), JOB_NOT_FOUND_MESSAGE);
}

// ── automations ──────────────────────────────────────────────────────────
