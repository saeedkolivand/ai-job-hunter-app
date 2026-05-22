//! URL → JobPosting resolver.
//!
//! Given an arbitrary job posting URL, return a `JobPosting` by:
//! 1. Recognising known board URL patterns and hitting their public API
//!    (Greenhouse, Lever, Ashby).
//! 2. Falling back to a generic HTML scraper that extracts the `<title>`
//!    and meta description.

use crate::scraping::types::JobPosting;
use anyhow::Result;
use scraper::{Html, Selector};
use std::collections::HashMap;

pub async fn resolve(url: &str) -> Result<Option<JobPosting>> {
    if let Some(posting) = try_greenhouse(url).await? {
        return Ok(Some(posting));
    }
    if let Some(posting) = try_lever(url).await? {
        return Ok(Some(posting));
    }
    if let Some(posting) = try_ashby(url).await? {
        return Ok(Some(posting));
    }
    generic_html(url).await
}

// ── Greenhouse ──────────────────────────────────────────────────────────────
//
// URL: https://boards.greenhouse.io/<company>/jobs/<job_id>
// API: https://boards-api.greenhouse.io/v1/boards/<company>/jobs/<job_id>

async fn try_greenhouse(url: &str) -> Result<Option<JobPosting>> {
    let (company, job_id) = match parse_greenhouse_url(url) {
        Some(p) => p,
        None => return Ok(None),
    };
    let api = format!(
        "https://boards-api.greenhouse.io/v1/boards/{}/jobs/{}",
        urlencoding::encode(&company),
        urlencoding::encode(&job_id),
    );
    let client = reqwest::Client::new();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;
    let title = v.get("title").and_then(|s| s.as_str()).unwrap_or("").to_string();
    let location = v
        .get("location")
        .and_then(|l| l.get("name"))
        .and_then(|s| s.as_str())
        .map(str::to_string);
    let description = v
        .get("content")
        .and_then(|s| s.as_str())
        .map(|s| crate::scraping::http::strip_html(s));
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

fn parse_greenhouse_url(url: &str) -> Option<(String, String)> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?;
    if !host.ends_with("greenhouse.io") {
        return None;
    }
    let segments: Vec<&str> = u.path_segments()?.collect();
    // Patterns:
    //  /<company>/jobs/<id>            (boards.greenhouse.io)
    //  /embed/job_app?for=<company>&token=<id>
    if segments.len() >= 3 && segments[1] == "jobs" {
        return Some((segments[0].to_string(), segments[2].to_string()));
    }
    if segments.first() == Some(&"embed") {
        let mut company = None;
        let mut token = None;
        for (k, v) in u.query_pairs() {
            match k.as_ref() {
                "for" => company = Some(v.into_owned()),
                "token" => token = Some(v.into_owned()),
                _ => {}
            }
        }
        if let (Some(c), Some(t)) = (company, token) {
            return Some((c, t));
        }
    }
    None
}

// ── Lever ───────────────────────────────────────────────────────────────────
//
// URL: https://jobs.lever.co/<company>/<id>
// API: https://api.lever.co/v0/postings/<company>/<id>

async fn try_lever(url: &str) -> Result<Option<JobPosting>> {
    let (company, job_id) = match parse_lever_url(url) {
        Some(p) => p,
        None => return Ok(None),
    };
    let api = format!(
        "https://api.lever.co/v0/postings/{}/{}",
        urlencoding::encode(&company),
        urlencoding::encode(&job_id),
    );
    let client = reqwest::Client::new();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;
    let title = v.get("text").and_then(|s| s.as_str()).unwrap_or("").to_string();
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
                .map(|s| crate::scraping::http::strip_html(s))
        });
    let abs_url = v
        .get("hostedUrl")
        .and_then(|s| s.as_str())
        .unwrap_or(url)
        .to_string();
    let created_at = v
        .get("createdAt")
        .and_then(|n| n.as_i64());

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

fn parse_lever_url(url: &str) -> Option<(String, String)> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?;
    if !host.ends_with("lever.co") {
        return None;
    }
    let segments: Vec<&str> = u.path_segments()?.collect();
    if segments.len() >= 2 {
        return Some((segments[0].to_string(), segments[1].to_string()));
    }
    None
}

// ── Ashby ───────────────────────────────────────────────────────────────────
//
// URL: https://jobs.ashbyhq.com/<company>/<id>

async fn try_ashby(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };
    if !host.ends_with("ashbyhq.com") {
        return Ok(None);
    }
    let segments: Vec<&str> = match u.path_segments().map(|s| s.collect()) {
        Some(v) => v,
        None => return Ok(None),
    };
    if segments.len() < 2 {
        return Ok(None);
    }
    let company = segments[0].to_string();
    let job_id = segments[1].to_string();

    // Public GraphQL endpoint for a single posting.
    let body = serde_json::json!({
        "operationName": "ApiJobPosting",
        "variables": { "organizationHostedJobsPageName": company, "jobPostingId": job_id },
        "query": "query ApiJobPosting($organizationHostedJobsPageName: String!, $jobPostingId: String!) { jobPosting(organizationHostedJobsPageName: $organizationHostedJobsPageName, jobPostingId: $jobPostingId) { title locationName departmentName descriptionPlain } }"
    });
    let client = reqwest::Client::new();
    let res = client
        .post("https://jobs.ashbyhq.com/api/non-user-graphql?op=ApiJobPosting")
        .header("content-type", "application/json")
        .body(body.to_string())
        .send()
        .await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;
    let p = v.get("data").and_then(|d| d.get("jobPosting"));
    let p = match p {
        Some(v) if !v.is_null() => v,
        _ => return Ok(None),
    };
    let title = p.get("title").and_then(|s| s.as_str()).unwrap_or("").to_string();
    let location = p.get("locationName").and_then(|s| s.as_str()).map(str::to_string);
    let description = p.get("descriptionPlain").and_then(|s| s.as_str()).map(str::to_string);

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

// ── Generic HTML fallback ───────────────────────────────────────────────────

async fn generic_html(url: &str) -> Result<Option<JobPosting>> {
    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
        )
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let res = client.get(url).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let html = res.text().await?;

    let (title, description) = parse_generic_html(&html);
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();

    Ok(Some(JobPosting {
        id: format!("url:{}", url),
        external_id: None,
        title,
        company: host,
        location: None,
        url: url.to_string(),
        source: "url".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}

fn parse_generic_html(html: &str) -> (String, Option<String>) {
    let doc = Html::parse_document(html);
    let title_sel = Selector::parse("title, h1").unwrap();
    let title = doc
        .select(&title_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default();
    let meta_sel = Selector::parse("meta[name=\"description\"], meta[property=\"og:description\"]").unwrap();
    let description = doc
        .select(&meta_sel)
        .next()
        .and_then(|e| e.value().attr("content").map(str::to_string));
    (title, description)
}
