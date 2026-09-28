//! LinkedIn board resolver: `linkedin.com/jobs/view/<id>` via the
//! authenticated client, with a generic-HTML-fallback fill for any field
//! the CSS selectors miss (auth-gated/redesigned shell).

use std::collections::HashMap;

use anyhow::Result;
use scraper::{Html, Selector};

use crate::scraping::types::JobPosting;

use super::{clean_description, parse_from_html, SCRAPE_URL_MAX_BYTES};

// ── LinkedIn ────────────────────────────────────────────────────────────────
//
// URL: https://www.linkedin.com/jobs/view/<id>
// Requires authed client from board_login::build_authed_client("linkedin").

pub(super) async fn try_linkedin(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };
    // Exact/suffix match only. The authed (cookie-bearing) LinkedIn client must
    // *only* ever talk to real `*.linkedin.com` — a substring gate would let a
    // look-alike host (`linkedin.com.attacker.tld`) pass and exfiltrate the
    // user's session cookies cross-host (SSRF + cookie exfil).
    if host != "linkedin.com" && host != "www.linkedin.com" && !host.ends_with(".linkedin.com") {
        return Ok(None);
    }
    let path = u.path();
    if !path.contains("/jobs/view/") {
        return Ok(None);
    }
    let job_id = path.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
    if job_id.is_empty() {
        return Ok(None);
    }

    let data_dir = crate::platform::config::data_dir();
    let client = crate::scraping::board_login::build_authed_client(&data_dir, "linkedin")?;

    let res = client.get(url).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let html = crate::net::http::read_text_capped(res, SCRAPE_URL_MAX_BYTES).await?;

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

    let location_sel =
        Selector::parse(".topcard__flavor--bullet, .top-card-layout__second-subline").unwrap();
    let location = doc
        .select(&location_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string());

    let desc_sel = Selector::parse(".show-more-less-html__markup, .description__text").unwrap();
    let description = doc
        .select(&desc_sel)
        .next()
        .map(|e| clean_description(&crate::scraping::http::html_to_text(&e.inner_html())));

    // Selectors miss when LinkedIn ships an auth-gated/redesigned shell. Fall back
    // to the shared JSON-LD / __NEXT_DATA__ / main-content parse and fill ONLY the
    // fields the selectors left empty (keep good selector values, keep source).
    let (title, description, location, company) =
        if title.is_empty() || description.as_deref().unwrap_or("").trim().is_empty() {
            let fb = parse_from_html(url, &html);
            let title = if title.is_empty() {
                fb.as_ref().map(|f| f.title.clone()).unwrap_or_default()
            } else {
                title
            };
            let description = description
                .filter(|d| !d.trim().is_empty())
                .or_else(|| fb.as_ref().and_then(|f| f.description.clone()));
            let location = location.or_else(|| fb.as_ref().and_then(|f| f.location.clone()));
            let company = if company == "LinkedIn" {
                fb.as_ref()
                    .map(|f| f.company.clone())
                    .filter(|c| !c.is_empty())
                    .unwrap_or(company)
            } else {
                company
            };
            (title, description, location, company)
        } else {
            (title, description, location, company)
        };

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
