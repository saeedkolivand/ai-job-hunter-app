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
    if let Some(posting) = try_linkedin(url).await? {
        return Ok(Some(posting));
    }
    if let Some(posting) = try_workday(url).await? {
        return Ok(Some(posting));
    }
    if let Some(posting) = try_smartrecruiters(url).await? {
        return Ok(Some(posting));
    }
    if let Some(posting) = try_personio(url).await? {
        return Ok(Some(posting));
    }
    generic_html(url).await
}

/// LinkedIn (and similar pages) render "Show more" / "Show less" toggle buttons
/// right after the description markup; strip those trailing labels.
fn clean_description(text: &str) -> String {
    let re = regex::Regex::new(r"(?i)(\s*(show more|show less))+\s*$").unwrap();
    re.replace(text, "").trim().to_string()
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

// ── LinkedIn ────────────────────────────────────────────────────────────────
//
// URL: https://www.linkedin.com/jobs/view/<id>
// Requires authed client from board_login::build_authed_client("linkedin").

async fn try_linkedin(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };
    if !host.contains("linkedin.com") {
        return Ok(None);
    }
    let path = u.path();
    if !path.contains("/jobs/view/") {
        return Ok(None);
    }
    let job_id = path
        .split('/')
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or("");
    if job_id.is_empty() {
        return Ok(None);
    }

    let data_dir = resolve_data_dir();
    let client = crate::scraping::board_login::build_authed_client(&data_dir, "linkedin")?;

    let res = client.get(url).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let html = res.text().await?;

    let doc = Html::parse_document(&html);
    let title_sel = Selector::parse("h1, .top-card-layout__title").unwrap();
    let title = doc
        .select(&title_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    let company_sel = Selector::parse(".topcard__org-name-link, .top-card-layout__card a").unwrap();
    let company = doc
        .select(&company_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_else(|| "LinkedIn".to_string());

    let location_sel = Selector::parse(".topcard__flavor--bullet, .top-card-layout__second-subline").unwrap();
    let location = doc
        .select(&location_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string());

    let desc_sel = Selector::parse(".show-more-less-html__markup, .description__text").unwrap();
    let description = doc
        .select(&desc_sel)
        .next()
        .map(|e| clean_description(&crate::scraping::http::html_to_text(&e.inner_html())));

    log::info!(
        "[scrape_url] linkedin {} description: {} chars",
        job_id,
        description.as_ref().map(|d| d.len()).unwrap_or(0)
    );

    Ok(Some(JobPosting {
        id: format!("linkedin:{}", job_id),
        external_id: Some(job_id.to_string()),
        title,
        company,
        location,
        url: url.to_string(),
        source: "linkedin".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}

// ── Workday ─────────────────────────────────────────────────────────────────
//
// URL: https://<tenant>.<host>.myworkdayjobs.com/<site>/job/<...>/<reqId>
// API: /wday/cxs/<tenant>/<site>/job/<reqId>

async fn try_workday(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host_str = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };
    if !host_str.contains("myworkdayjobs.com") {
        return Ok(None);
    }

    let re = regex::Regex::new(r"^([^.]+)\.(wd\d+)\.myworkdayjobs\.com$").unwrap();
    let caps = match re.captures(host_str) {
        Some(c) => c,
        None => return Ok(None),
    };
    let tenant = caps.get(1).map(|m| m.as_str()).unwrap_or("");
    let host = caps.get(2).map(|m| m.as_str()).unwrap_or("wd1");

    let path = u.path();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 2 {
        return Ok(None);
    }
    let site = segments[0];
    let req_id = segments.last().unwrap_or(&"");
    if req_id.is_empty() {
        return Ok(None);
    }

    let api = format!(
        "https://{}.{}.myworkdayjobs.com/wday/cxs/{}/{}/job/{}",
        tenant, host, tenant, site, req_id
    );

    let client = reqwest::Client::new();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;

    let title = v
        .get("title")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let location = v
        .get("locationsText")
        .and_then(|s| s.as_str())
        .map(str::to_string);
    let description = v
        .get("jobPostingInfo")
        .and_then(|info| info.get("jobDescription"))
        .and_then(|s| s.as_str())
        .map(|s| crate::scraping::http::strip_html(s));

    Ok(Some(JobPosting {
        id: format!("workday:{}", req_id),
        external_id: Some(req_id.to_string()),
        title,
        company: tenant.to_string(),
        location,
        url: url.to_string(),
        source: "workday".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    }))
}

// ── SmartRecruiters ─────────────────────────────────────────────────────────
//
// URL: https://jobs.smartrecruiters.com/<company>/<id>
// API: https://api.smartrecruiters.com/v1/companies/<company>/postings/<id>

async fn try_smartrecruiters(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };
    if !host.contains("smartrecruiters.com") {
        return Ok(None);
    }
    let segments: Vec<&str> = u.path_segments().map(|s| s.collect()).unwrap_or_default();
    if segments.len() < 2 {
        return Ok(None);
    }
    let company = segments[0];
    let job_id = segments[1];

    let api = format!(
        "https://api.smartrecruiters.com/v1/companies/{}/postings/{}",
        urlencoding::encode(company),
        urlencoding::encode(job_id)
    );

    let client = reqwest::Client::new();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;

    let title = v.get("name").and_then(|s| s.as_str()).unwrap_or("").to_string();
    let location = v
        .get("location")
        .and_then(|l| {
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
                .map(|t| crate::scraping::http::strip_html(t))
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

// ── Personio ────────────────────────────────────────────────────────────────
//
// URL: https://<company>.jobs.personio.{de,com}/?id=<id>
// Match against the XML feed item.

async fn try_personio(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };
    if !host.contains("jobs.personio.") {
        return Ok(None);
    }

    let company = host.split('.').next().unwrap_or("");
    if company.is_empty() {
        return Ok(None);
    }

    let id = u
        .query_pairs()
        .find(|(k, _)| k == "id")
        .map(|(_, v)| v.into_owned());
    let id = match id {
        Some(i) if !i.is_empty() => i,
        _ => return Ok(None),
    };

    // Fetch the XML feed and find the matching position.
    let feed_url = format!("https://{}", host);
    let client = reqwest::Client::new();
    let res = client.get(&feed_url).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let xml = res.text().await?;

    let position_re = regex::Regex::new(r"<position>(.*?)</position>").unwrap();
    let id_re = regex::Regex::new(r"<id>(.*?)</id>").unwrap();
    let name_re = regex::Regex::new(r"<name>(.*?)</name>").unwrap();
    let office_re = regex::Regex::new(r"<office>(.*?)</office>").unwrap();
    let desc_re = regex::Regex::new(r"<jobDescription>\s*<value>(.*?)</value>\s*</jobDescription>").unwrap();

    for position_cap in position_re.captures_iter(&xml) {
        if let Some(position_content) = position_cap.get(1) {
            let position_str = position_content.as_str();
            let pos_id = id_re
                .captures(position_str)
                .and_then(|c| c.get(1).map(|m| m.as_str().trim()))
                .unwrap_or("");

            if pos_id == id {
                let title = name_re
                    .captures(position_str)
                    .and_then(|c| c.get(1).map(|m| m.as_str().trim()))
                    .unwrap_or("");
                let office = office_re
                    .captures(position_str)
                    .and_then(|c| c.get(1).map(|m| m.as_str().trim()))
                    .unwrap_or("");
                let desc = desc_re
                    .captures(position_str)
                    .and_then(|c| c.get(1).map(|m| crate::scraping::http::strip_html(m.as_str().trim())))
                    .unwrap_or_default();

                return Ok(Some(JobPosting {
                    id: format!("personio:{}", id),
                    external_id: Some(id.clone()),
                    title: title.to_string(),
                    company: company.to_string(),
                    location: if office.is_empty() { None } else { Some(office.to_string()) },
                    url: url.to_string(),
                    source: "personio".to_string(),
                    description: if desc.is_empty() { None } else { Some(desc) },
                    requirements: None,
                    posted_at: None,
                    captured_at: chrono::Utc::now().timestamp_millis(),
                    extra: HashMap::new(),
                }));
            }
        }
    }

    Ok(None)
}

fn resolve_data_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("AJH_DATA_DIR") {
        return std::path::PathBuf::from(dir);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    std::path::PathBuf::from(home).join(".ajh")
}

#[cfg(test)]
mod test;
