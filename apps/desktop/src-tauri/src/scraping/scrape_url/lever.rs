//! Lever board resolver: `jobs.lever.co/<company>/<id>` →
//! `api.lever.co/v0/postings/<company>/<id>`.

use std::collections::HashMap;

use anyhow::Result;

use crate::scraping::types::JobPosting;

use super::SCRAPE_URL_MAX_BYTES;

// ── Lever ───────────────────────────────────────────────────────────────────
//
// URL: https://jobs.lever.co/<company>/<id>
// API: https://api.lever.co/v0/postings/<company>/<id>

pub(super) async fn try_lever(url: &str) -> Result<Option<JobPosting>> {
    let (company, job_id) = match parse_lever_url(url) {
        Some(p) => p,
        None => return Ok(None),
    };
    let api = format!(
        "https://api.lever.co/v0/postings/{}/{}",
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
        .get("text")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let location = v
        .get("categories")
        .and_then(|c| c.get("location"))
        .and_then(|s| s.as_str())
        .map(str::to_string);
    let description = v
        .get("descriptionPlain")
        .and_then(|s| s.as_str())
        .map(str::to_string)
        .or_else(|| {
            v.get("description")
                .and_then(|s| s.as_str())
                .map(crate::scraping::http::html_to_markdown)
        });
    let abs_url = v
        .get("hostedUrl")
        .and_then(|s| s.as_str())
        .unwrap_or(url)
        .to_string();
    let created_at = v.get("createdAt").and_then(|n| n.as_i64());

    Ok(Some(JobPosting {
        id: format!("lever:{}", job_id),
        external_id: Some(job_id),
        title,
        company,
        location,
        url: abs_url,
        source: "lever".to_string(),
        description,
        requirements: None,
        posted_at: created_at,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}

pub(super) fn parse_lever_url(url: &str) -> Option<(String, String)> {
    // Company slug via the single ATS URL-shape authority (`ats_ref`); the job id
    // is the second path segment (`jobs.lever.co/<company>/<id>`).
    let company = crate::scraping::ats_ref::lever_slug(url)?;
    let u = reqwest::Url::parse(url).ok()?;
    let segments: Vec<&str> = u.path_segments()?.collect();
    if segments.len() >= 2 {
        return Some((company, segments[1].to_string()));
    }
    None
}
