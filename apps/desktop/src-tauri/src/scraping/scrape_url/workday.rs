//! Workday board resolver:
//! `<tenant>.<host>.myworkdayjobs.com/<site>/job/<...>/<reqId>` →
//! `/wday/cxs/<tenant>/<site>/job/<reqId>`.

use std::collections::HashMap;

use anyhow::Result;

use crate::scraping::types::JobPosting;

use super::SCRAPE_URL_MAX_BYTES;

// ── Workday ─────────────────────────────────────────────────────────────────
//
// URL: https://<tenant>.<host>.myworkdayjobs.com/<site>/job/<...>/<reqId>
// API: /wday/cxs/<tenant>/<site>/job/<reqId>

pub(super) async fn try_workday(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host_str = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };
    // Exact/suffix match only — a substring gate (`contains`) would accept a
    // look-alike host (`myworkdayjobs.com.attacker.tld`) which matters now that
    // re-dispatch runs these handlers on attacker-influenced redirect targets.
    if host_str != "myworkdayjobs.com" && !host_str.ends_with(".myworkdayjobs.com") {
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
        .get("locationsText")
        .and_then(|s| s.as_str())
        .map(str::to_string);
    let description = v
        .get("jobPostingInfo")
        .and_then(|info| info.get("jobDescription"))
        .and_then(|s| s.as_str())
        .map(crate::scraping::http::html_to_markdown);

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
