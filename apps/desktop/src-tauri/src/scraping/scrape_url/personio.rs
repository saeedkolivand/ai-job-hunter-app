//! Personio board resolver: `<company>.jobs.personio.{de,com}/?id=<id>`,
//! matched against the same XML feed the Personio board scraper parses.

use std::collections::HashMap;

use anyhow::Result;

use crate::scraping::types::JobPosting;

use super::SCRAPE_URL_MAX_BYTES;

// ── Personio ────────────────────────────────────────────────────────────────
//
// URL: https://<company>.jobs.personio.{de,com}/?id=<id>
// Match against the XML feed item.

/// Extract the company slug from a Personio job URL.
///
/// Valid hosts are `<company>.jobs.personio.{de,com}` — the first host label
/// (before the first `.`) is the company slug, returned lowercased.
/// Returns `None` for non-Personio hosts, bare `jobs.personio.*` roots (no
/// company subdomain), or malformed/unparseable URLs.
pub(crate) fn personio_company_from_url(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?;
    // Exact/suffix match only — a substring gate would let a look-alike host
    // (`jobs.personio.attacker.tld`) pass.
    if host != "jobs.personio.de"
        && host != "jobs.personio.com"
        && !host.ends_with(".jobs.personio.de")
        && !host.ends_with(".jobs.personio.com")
    {
        return None;
    }
    // The bare roots (`jobs.personio.de` / `jobs.personio.com`) have no
    // company subdomain — the first label would be "jobs", which is wrong.
    if host == "jobs.personio.de" || host == "jobs.personio.com" {
        return None;
    }
    let company = host.split('.').next()?;
    if company.is_empty() {
        return None;
    }
    Some(company.to_ascii_lowercase())
}

pub(super) async fn try_personio(url: &str) -> Result<Option<JobPosting>> {
    let u = match reqwest::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return Ok(None),
    };
    let host = match u.host_str() {
        Some(h) => h,
        None => return Ok(None),
    };

    let company = match personio_company_from_url(url) {
        Some(c) => c,
        None => return Ok(None),
    };

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
    // Route through the IP-validated, IP-pinned, redirect-disabled guarded
    // client: even with the tightened host gate, treat the feed fetch as an
    // attacker-influenced egress and close the DNS-rebinding TOCTOU.
    let res = match crate::net::http::get_guarded(&feed_url).await {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    if !res.status().is_success() {
        return Ok(None);
    }
    // Same unbounded-body risk as the generic-HTML fallback above: this host is
    // attacker-influenced, and `Response::text()` buffers the whole body with no
    // limit. Read it through the shared cap instead.
    let xml = match crate::net::http::read_text_capped(res, SCRAPE_URL_MAX_BYTES).await {
        Ok(x) => x,
        Err(_) => return Ok(None),
    };

    // Shared feed parser (regex set + capture loop) lives in the Personio board.
    // Here we pick the single position whose id matches the URL query and map it
    // onto this resolver's JobPosting shape (original url, personio:{company}:{id}).
    // Use make_job_id so both the board-scrape path and this URL-resolve path
    // produce byte-identical ids for the same posting — deduplication depends on it.
    let position = crate::scraping::boards::personio::parse_xml_feed(&xml)
        .into_iter()
        .find(|p| p.id == id);
    if let Some(pos) = position {
        return Ok(Some(JobPosting {
            id: crate::scraping::boards::personio::make_job_id(&company, &id),
            external_id: Some(id.clone()),
            title: pos.title,
            company: company.clone(),
            location: if pos.office.is_empty() {
                None
            } else {
                Some(pos.office)
            },
            url: url.to_string(),
            source: "personio".to_string(),
            description: if pos.description.is_empty() {
                None
            } else {
                Some(pos.description)
            },
            requirements: None,
            posted_at: None,
            captured_at: chrono::Utc::now().timestamp_millis(),
            extra: HashMap::new(),
        }));
    }

    Ok(None)
}
