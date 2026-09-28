//! Greenhouse board resolver: `boards.greenhouse.io/<company>/jobs/<job_id>`
//! → `boards-api.greenhouse.io/v1/boards/<company>/jobs/<job_id>`.

use std::collections::HashMap;

use anyhow::Result;

use crate::scraping::types::JobPosting;

use super::SCRAPE_URL_MAX_BYTES;

// ── Greenhouse ──────────────────────────────────────────────────────────────
//
// URL: https://boards.greenhouse.io/<company>/jobs/<job_id>
// API: https://boards-api.greenhouse.io/v1/boards/<company>/jobs/<job_id>

pub(super) async fn try_greenhouse(url: &str) -> Result<Option<JobPosting>> {
    let (company, job_id) = match parse_greenhouse_url(url) {
        Some(p) => p,
        None => return Ok(None),
    };
    let api = format!(
        "https://boards-api.greenhouse.io/v1/boards/{}/jobs/{}",
        urlencoding::encode(&company),
        urlencoding::encode(&job_id),
    );
    let client = crate::net::http::shared();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value =
        crate::net::http::read_json_capped(res, SCRAPE_URL_MAX_BYTES).await?;
    let title = v
        .get("title")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let location = v
        .get("location")
        .and_then(|l| l.get("name"))
        .and_then(|s| s.as_str())
        .map(str::to_string);
    let description = v
        .get("content")
        .and_then(|s| s.as_str())
        .map(crate::scraping::http::html_to_markdown);
    let abs_url = v
        .get("absolute_url")
        .and_then(|s| s.as_str())
        .unwrap_or(url)
        .to_string();
    let updated_at = v
        .get("updated_at")
        .and_then(|s| s.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp_millis());

    Ok(Some(JobPosting {
        id: format!("greenhouse:{}", job_id),
        external_id: Some(job_id),
        title,
        company,
        location,
        url: abs_url,
        source: "greenhouse".to_string(),
        description,
        requirements: None,
        posted_at: updated_at,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}

pub(super) fn parse_greenhouse_url(url: &str) -> Option<(String, String)> {
    // Company slug via the single ATS URL-shape authority (`ats_ref`); this fn
    // layers the job id on top for single-job resolution. Patterns:
    //  /<company>/jobs/<id>            (boards.greenhouse.io)
    //  /embed/job_app?for=<company>&token=<id>
    let company = crate::scraping::ats_ref::greenhouse_slug(url)?;
    let u = reqwest::Url::parse(url).ok()?;
    let segments: Vec<&str> = u.path_segments()?.collect();
    if segments.first() == Some(&"embed") {
        let token = u
            .query_pairs()
            .find(|(k, _)| k == "token")
            .map(|(_, v)| v.into_owned())?;
        return Some((company, token));
    }
    if segments.len() >= 3 && segments[1] == "jobs" {
        return Some((company, segments[2].to_string()));
    }
    None
}
