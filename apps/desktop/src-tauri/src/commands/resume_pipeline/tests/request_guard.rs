//! `resume_pipeline_run`'s synchronous gate (#1496): a request that could never run is refused
//! before ids are returned, and `jobUrl` names a found job.

use serde_json::json;

use super::super::request::{needs_found_job, resolve_found_job, validate};
use crate::autopilot::tests::support::{create_ap, found_job_full, temp_store};
use crate::autopilot::Autopilot;
use crate::ipc_contracts::resume_pipeline::ResumePipelineRunRequest;

fn request(extra: serde_json::Value) -> ResumePipelineRunRequest {
    let mut body = json!({ "resumeText": "Jane Doe\nEngineer" });
    body.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(body).expect("request deserializes")
}

fn err_text<T>(result: crate::error::AppResult<T>) -> String {
    result.err().map(|e| e.to_string()).unwrap_or_default()
}

/// The request from the issue: a résumé and a `jobUrl`, no `jobId`/`jobAdText`, accepted and
/// then failed out of sight. Mutation check: drop the `job_url` arm of the job check and the
/// url-only case fails; drop the whole job check and the "neither" case fails.
#[test]
fn a_request_naming_no_posting_is_refused_before_ids_exist() {
    let neither = request(json!({}));
    assert!(err_text(validate(&neither)).contains("jobId, jobAdText or jobUrl"));

    let blank_only = request(json!({ "jobId": "  ", "jobAdText": "\n" }));
    assert!(validate(&blank_only).is_err(), "whitespace names nothing");

    for named in [
        json!({ "jobId": "job-1" }),
        json!({ "jobAdText": "a posting" }),
        json!({ "jobUrl": "https://boards.example/jobs/1" }),
    ] {
        validate(&request(named.clone())).unwrap_or_else(|e| panic!("{named} must pass: {e}"));
    }
}

#[test]
fn a_request_with_no_resume_or_no_document_to_write_is_refused() {
    let no_resume: ResumePipelineRunRequest =
        serde_json::from_value(json!({ "jobId": "job-1" })).unwrap();
    assert!(err_text(validate(&no_resume)).contains("resumeId or resumeText"));

    let writes_nothing = request(json!({
        "jobId": "job-1", "includeResume": false, "includeCoverLetter": false
    }));
    assert!(err_text(validate(&writes_nothing)).contains("at least one document"));
}

fn autopilot_with(jobs: Vec<crate::autopilot::FoundJob>) -> Autopilot {
    let (_dir, store) = temp_store();
    let mut ap = create_ap(&store, "Rust", "remotive", 0.0, "manual");
    ap.found_jobs = jobs;
    ap
}

fn described(url: &str, description: Option<&str>) -> crate::autopilot::FoundJob {
    let mut job = found_job_full(url, "Staff Engineer", "Example Corp", 1);
    job.board = Some("remotive".to_string());
    job.description = description.map(str::to_string);
    job
}

/// Mutation check: make `resolve_found_job` a no-op and `job_ad_text` stays empty.
#[test]
fn a_job_url_resolves_to_the_found_jobs_description_and_identity() {
    let ap = autopilot_with(vec![described(
        "https://boards.example/jobs/42/",
        Some("Own the payments platform."),
    )]);
    // A different spelling of the same link (no trailing slash) still matches.
    let mut req = request(json!({ "jobUrl": "https://boards.example/jobs/42" }));
    assert!(needs_found_job(&req));
    resolve_found_job(&mut req, &[ap]).expect("resolves");
    assert_eq!(req.job_ad_text, "Own the payments platform.");
    assert_eq!(req.job_title, "Staff Engineer");
    assert_eq!(req.company_name, "Example Corp");
    assert_eq!(req.board, "remotive");
}

#[test]
fn a_caller_supplied_identity_is_never_overwritten_by_the_lookup() {
    let ap = autopilot_with(vec![described(
        "https://boards.example/jobs/42",
        Some("text"),
    )]);
    let mut req = request(json!({
        "jobUrl": "https://boards.example/jobs/42", "jobTitle": "My Title"
    }));
    resolve_found_job(&mut req, &[ap]).expect("resolves");
    assert_eq!(req.job_title, "My Title");
}

#[test]
fn a_job_url_that_resolves_to_nothing_usable_is_refused_by_name() {
    let ap = autopilot_with(vec![
        described("https://boards.example/jobs/1", Some("text")),
        described("https://boards.example/jobs/2", None),
        described("https://boards.example/jobs/3", Some("   ")),
    ]);
    let aps = [ap];

    let mut unknown = request(json!({ "jobUrl": "https://boards.example/jobs/999" }));
    assert!(
        err_text(resolve_found_job(&mut unknown, &aps)).contains("does not match any found job")
    );

    for bare in [
        "https://boards.example/jobs/2",
        "https://boards.example/jobs/3",
    ] {
        let mut req = request(json!({ "jobUrl": bare }));
        assert!(
            err_text(resolve_found_job(&mut req, &aps)).contains("no stored description"),
            "{bare}"
        );
    }

    let mut not_http = request(json!({ "jobUrl": "javascript:alert(1)" }));
    assert!(err_text(resolve_found_job(&mut not_http, &aps)).contains("valid http(s) link"));
}

/// Id wins over text wins over url: a request that already names its posting is not re-resolved.
#[test]
fn an_id_or_text_means_the_url_is_not_looked_up() {
    let with_id = request(json!({ "jobId": "job-1", "jobUrl": "https://a.example/1" }));
    let with_text = request(json!({ "jobAdText": "ad", "jobUrl": "https://a.example/1" }));
    assert!(!needs_found_job(&with_id));
    assert!(!needs_found_job(&with_text));
}

fn matching(url: &str, found_at: u64, text: &str) -> crate::autopilot::FoundJob {
    let mut job = described(url, Some(text));
    job.found_at = found_at;
    job
}

/// A row held for an identity review may be a different posting than its url says.
/// Mutation check: drop the `identity_review.is_none()` clause and this resolves.
#[test]
fn a_row_held_for_identity_review_is_skipped_and_the_refusal_says_so() {
    let mut held = matching("https://boards.example/jobs/5", 1, "description text");
    held.identity_review = Some(crate::autopilot::IdentityReview {
        resolved_title: "Other Role".into(),
        resolved_company: "Other Corp".into(),
        resolved_url: "https://boards.example/jobs/other".into(),
        resolved_external_id: None,
        detected_at: 1,
    });
    let aps = [autopilot_with(vec![held])];
    let mut req = request(json!({ "jobUrl": "https://boards.example/jobs/5" }));
    assert!(err_text(resolve_found_job(&mut req, &aps)).contains("identity review"));
    assert!(req.job_ad_text.is_empty());
}

/// A provisional row's stored text is the truncated snippet, not the full description.
#[test]
fn a_provisionally_scored_row_has_no_full_description() {
    let mut truncated = matching("https://boards.example/jobs/6", 1, "a snippet");
    truncated.score_provisional = true;
    let aps = [autopilot_with(vec![truncated])];
    let mut req = request(json!({ "jobUrl": "https://boards.example/jobs/6" }));
    assert!(err_text(resolve_found_job(&mut req, &aps)).contains("truncated description"));
}

/// A usable row is chosen over a held one for the same url, and ties break the same way
/// whatever the iteration order.
#[test]
fn the_pick_among_several_matches_is_deterministic() {
    let url = "https://boards.example/jobs/7";
    let mut held = matching(url, 9, &"a very long held description ".repeat(10));
    held.identity_review = Some(crate::autopilot::IdentityReview {
        resolved_title: "Other Role".into(),
        resolved_company: "Other Corp".into(),
        resolved_url: "https://boards.example/jobs/other".into(),
        resolved_external_id: None,
        detected_at: 1,
    });
    let older = matching(url, 1, "same length 1");
    let newer = matching(url, 2, "same length 2");
    let forward = [autopilot_with(vec![
        held.clone(),
        older.clone(),
        newer.clone(),
    ])];
    let reverse = [autopilot_with(vec![newer, older, held])];
    let pick = |aps: &[Autopilot]| {
        let mut req = request(json!({ "jobUrl": url }));
        resolve_found_job(&mut req, aps).expect("resolves");
        req.job_ad_text
    };
    assert_eq!(pick(&forward), "same length 2");
    assert_eq!(pick(&forward), pick(&reverse));
}

/// #1497: a stored row that arrived entity-escaped reaches the prompts decoded.
#[test]
fn an_escaped_stored_description_is_decoded_before_it_reaches_the_prompts() {
    let ap = autopilot_with(vec![described(
        "https://boards.example/jobs/77",
        Some("&lt;p&gt;Own the payments platform.&lt;/p&gt;"),
    )]);
    let mut req = request(json!({ "jobUrl": "https://boards.example/jobs/77" }));
    resolve_found_job(&mut req, &[ap]).expect("resolves");
    assert_eq!(req.job_ad_text, "Own the payments platform.");
}

/// Two postings that differ only by a query-string id collapse under `normalize_job_url` on a
/// host outside its allowlist. Mutation check: match on the normalized form only and the exact
/// case picks the longer description of the OTHER job, the ambiguous case resolves silently.
#[test]
fn a_job_url_prefers_the_exact_posting_and_refuses_an_ambiguous_collapse() {
    let first = "https://careers.example.org/view?id=1";
    let second = "https://careers.example.org/view?id=2";
    assert_eq!(
        crate::applications::normalize_job_url(first),
        crate::applications::normalize_job_url(second),
        "premise: the normalizer collapses the pair"
    );
    let ap = autopilot_with(vec![
        described(first, Some("short text")),
        described(
            second,
            Some("a much longer description of the other posting"),
        ),
    ]);

    let mut exact = request(json!({ "jobUrl": first }));
    resolve_found_job(&mut exact, std::slice::from_ref(&ap)).expect("exact wins");
    assert_eq!(exact.job_ad_text, "short text");

    let mut ambiguous = request(json!({ "jobUrl": "https://careers.example.org/view?id=3" }));
    let err = err_text(resolve_found_job(&mut ambiguous, &[ap]));
    assert!(err.contains("jobId"), "{err}");
}
