//! SmartRecruiters board resolver: `jobs.smartrecruiters.com/<company>/<id>`
//! → `api.smartrecruiters.com/v1/companies/<company>/postings/<id>`.

use std::collections::HashMap;

use anyhow::Result;

use crate::scraping::types::JobPosting;

use super::SCRAPE_URL_MAX_BYTES;

// ── SmartRecruiters ─────────────────────────────────────────────────────────
//
// URL: https://jobs.smartrecruiters.com/<company>/<id>
// API: https://api.smartrecruiters.com/v1/companies/<company>/postings/<id>

pub(super) async fn try_smartrecruiters(url: &str) -> Result<Option<JobPosting>> {
    // Company slug via the single ATS URL-shape authority (`ats_ref`, which owns
    // the `*.smartrecruiters.com` host gate — the suffix match rejects the
    // redirect-resolved look-alike hosts this handler may be re-dispatched on);
    // the job id is the second path segment.
    let company = match crate::scraping::ats_ref::smartrecruiters_slug(url) {
        Some(c) => c,
        None => return Ok(None),
    };
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let segments: Vec<&str> = u.path_segments().map(|s| s.collect()).unwrap_or_default();
    if segments.len() < 2 {
        return Ok(None);
    }
    let company = company.as_str();
    let job_id = segments[1];

    let api = format!(
        "https://api.smartrecruiters.com/v1/companies/{}/postings/{}",
        urlencoding::encode(company),
        urlencoding::encode(job_id)
    );

    let client = crate::net::http::shared();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value =
        crate::net::http::read_json_capped(res, SCRAPE_URL_MAX_BYTES).await?;

    let title = v
        .get("name")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let location = v.get("location").and_then(|l| {
        let city = l.get("city").and_then(|s| s.as_str());
        let country = l.get("country").and_then(|s| s.as_str());
        match (city, country) {
            (Some(c), Some(co)) => Some(format!("{}, {}", c, co)),
            (Some(c), None) => Some(c.to_string()),
            (None, Some(co)) => Some(co.to_string()),
            _ => None,
        }
    });

    let description = v
        .get("jobAd")
        .and_then(|ja| ja.get("sections"))
        .and_then(|s| s.as_object())
        .map(|sections| {
            sections
                .values()
                .filter_map(|sec| sec.get("text").and_then(|t| t.as_str()))
                .map(crate::scraping::http::html_to_markdown)
                .collect::<Vec<_>>()
                .join("\n\n")
        });

    Ok(Some(JobPosting {
        id: format!("smartrecruiters:{}", job_id),
        external_id: Some(job_id.to_string()),
        title,
        company: company.to_string(),
        location,
        url: url.to_string(),
        source: "smartrecruiters".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}
