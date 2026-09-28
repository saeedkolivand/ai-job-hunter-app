//! Ashby board resolver: `jobs.ashbyhq.com/<company>/<id>`, via the public
//! `non-user-graphql` endpoint.

use std::collections::HashMap;

use anyhow::Result;

use crate::scraping::types::JobPosting;

use super::SCRAPE_URL_MAX_BYTES;

// ── Ashby ───────────────────────────────────────────────────────────────────
//
// URL: https://jobs.ashbyhq.com/<company>/<id>

pub(super) async fn try_ashby(url: &str) -> Result<Option<JobPosting>> {
    // Company slug via the single ATS URL-shape authority (`ats_ref`, which owns
    // the host gate); the job id is the second path segment (a bare careers page
    // with no id can't resolve a single posting). Slug casing is preserved —
    // Ashby's board tokens are case-sensitive.
    let company = match crate::scraping::ats_ref::ashby_slug(url) {
        Some(c) => c,
        None => return Ok(None),
    };
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let job_id = match u.path_segments().and_then(|mut s| {
        s.next();
        s.next()
    }) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return Ok(None),
    };

    // Public GraphQL endpoint for a single posting.
    let body = serde_json::json!({
        "operationName": "ApiJobPosting",
        "variables": { "organizationHostedJobsPageName": company, "jobPostingId": job_id },
        "query": "query ApiJobPosting($organizationHostedJobsPageName: String!, $jobPostingId: String!) { jobPosting(organizationHostedJobsPageName: $organizationHostedJobsPageName, jobPostingId: $jobPostingId) { title locationName departmentName descriptionPlain } }"
    });
    let client = crate::net::http::shared();
    let res = client
        .post("https://jobs.ashbyhq.com/api/non-user-graphql?op=ApiJobPosting")
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value =
        crate::net::http::read_json_capped(res, SCRAPE_URL_MAX_BYTES).await?;
    let p = v.get("data").and_then(|d| d.get("jobPosting"));
    let p = match p {
        Some(v) if !v.is_null() => v,
        _ => return Ok(None),
    };
    let title = p
        .get("title")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let location = p
        .get("locationName")
        .and_then(|s| s.as_str())
        .map(str::to_string);
    let description = p
        .get("descriptionPlain")
        .and_then(|s| s.as_str())
        .map(str::to_string);

    Ok(Some(JobPosting {
        id: format!("ashby:{}", job_id),
        external_id: Some(job_id),
        title,
        company,
        location,
        url: url.to_string(),
        source: "ashby".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}
