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

/// Map a board's search / SPA "list + detail pane" view URL (where the SELECTED
/// job's id lives in a query param) to the canonical single-job URL. Returns
/// `None` when the URL is already a direct job page or the host is unrecognized —
/// the caller then uses the URL as-is. This is the single, centralized place that
/// knows "which job is selected in this SPA view"; every board plugs in via one
/// match arm. Ids are validated before being interpolated into a URL we will
/// later fetch (defense-in-depth alongside the import path's SSRF guard).
pub fn canonical_job_url(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    let query = |key: &str| {
        u.query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
    };

    // LinkedIn: /jobs/search|collections/...?currentJobId=<id> → /jobs/view/<id>.
    // Numeric id only. Skip when already a direct /jobs/view/ page.
    if host == "linkedin.com" || host == "www.linkedin.com" || host.ends_with(".linkedin.com") {
        if !u.path().contains("/jobs/view/") {
            if let Some(id) = query("currentJobId") {
                if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
                    return Some(format!("https://www.linkedin.com/jobs/view/{id}"));
                }
            }
        }
        return None;
    }

    // Indeed (incl. country TLDs like de.indeed.com): ?vjk=<id> → /viewjob?jk=<id>.
    // Alphanumeric id only. Skip when already a /viewjob page.
    if host == "indeed.com" || host.ends_with(".indeed.com") {
        if !u.path().contains("/viewjob") {
            if let Some(id) = query("vjk") {
                if !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric()) {
                    return Some(format!("https://{host}/viewjob?jk={id}"));
                }
            }
        }
        return None;
    }

    // TODO(import): Glassdoor (jobListingId/jl), Xing, StepStone — need a real
    // captured URL to pin the param + canonical template. Tracked as a follow-up.
    None
}

/// LinkedIn (and similar pages) render "Show more" / "Show less" toggle buttons
/// right after the description markup; strip those trailing labels.
static SHOW_MORE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)(\s*(show more|show less))+\s*$").unwrap());

fn clean_description(text: &str) -> String {
    SHOW_MORE_RE.replace(text, "").trim().to_string()
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
    let client = crate::net::http::shared();
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
        .get("location")
        .and_then(|l| l.get("name"))
        .and_then(|s| s.as_str())
        .map(str::to_string);
    let description = v
        .get("content")
        .and_then(|s| s.as_str())
        .map(crate::scraping::http::strip_html);
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
    let client = crate::net::http::shared();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;
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
                .map(crate::scraping::http::strip_html)
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
    let v: serde_json::Value = res.json().await?;
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

// ── Generic HTML fallback ───────────────────────────────────────────────────

async fn generic_html(url: &str) -> Result<Option<JobPosting>> {
    // The only egress that fetches the raw, attacker-controlled user URL. The
    // guarded fetch IP-validates + IP-pins the resolved address (closing the
    // DNS-rebinding TOCTOU); a rejected/unsafe host is treated as "no scraper
    // matched" so the paste-a-URL flow degrades gracefully (the bridge then
    // replies a clean "could not parse" import error).
    // Follow redirects (e.g. an aggregator `redirect_url` that bounces to the real
    // posting) — each hop is re-validated by get_guarded. Capped to 2 hops: an
    // aggregator redirect_url → real posting is typically 1 hop, and 2 covers a CDN
    // bounce. The cap is deliberately small because the per-slot rate limiter on the
    // calling IPC command charges ONE slot per resolve, so one slot must equal a
    // small, bounded number of outbound fetches (worst case 1 + 2 = 3).
    let res = match crate::net::http::get_guarded_following_redirects(url, 2).await {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    if !res.status().is_success() {
        return Ok(None);
    }
    let html = res.text().await?;
    Ok(parse_from_html(url, &html))
}

/// Build a `JobPosting` from already-fetched HTML — the fetch-free half of
/// [`generic_html`]. The extension-bridge **Scan mode** supplies the
/// authenticated DOM (a logged-in board page the desktop's anonymous fetch
/// can't see), so it reuses this exact parse path instead of re-fetching.
///
/// Prefers JSON-LD `JobPosting` fields (title/description/location/company) when
/// the page ships them, falling back to the generic `<title>`/`<h1>` + meta
/// description and the `parse_generic_company` employer-name heuristic. Always
/// returns `Some` for a successfully-parsed document — the title may be an empty
/// string (e.g. a page with only a meta description) so the description-on-demand
/// flow still surfaces the page. (The fetch half short-circuits earlier on a
/// non-success status / rejected host.)
pub fn parse_from_html(url: &str, html: &str) -> Option<JobPosting> {
    let (mut title, mut description) = parse_generic_html(html);
    let mut location = None;

    // JSON-LD `JobPosting` is the richest source when present — let it override
    // the generic title/description and supply a location the meta tags lack.
    if let Some(jl) = json_ld_job_posting(html) {
        if !jl.title.is_empty() {
            title = jl.title;
        }
        if jl.description.is_some() {
            description = jl.description;
        }
        if jl.location.is_some() {
            location = jl.location;
        }
    }

    // `__NEXT_DATA__` fallback: fill ONLY fields JSON-LD left missing (don't
    // clobber good JSON-LD values).
    if title.is_empty() || description.is_none() {
        if let Some(nd) = next_data_job(html) {
            if title.is_empty() && !nd.title.is_empty() {
                title = nd.title;
            }
            if description.is_none() {
                description = nd.description;
            }
            if location.is_none() {
                location = nd.location;
            }
        }
    }

    // Main-content text as a last-resort description.
    if description.is_none() {
        description = main_content_text(html);
    }

    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    // Prefer a real employer name (JSON-LD / og:site_name) over the bare host.
    let company = parse_generic_company(html).unwrap_or(host);

    Some(JobPosting {
        id: format!("url:{}", url),
        external_id: None,
        title,
        company,
        location,
        url: url.to_string(),
        source: "url".to_string(),
        description,
        requirements: None,
        posted_at: None,
        captured_at: chrono::Utc::now().timestamp_millis(),
        extra: HashMap::new(),
    })
}

/// The subset of JSON-LD `JobPosting` fields the generic parse path consumes.
struct JsonLdJob {
    title: String,
    description: Option<String>,
    location: Option<String>,
}

/// Format one JSON-LD `PostalAddress`-shaped node to a display string.
/// Locality-first (`"City, Region"` / `"City"` / `"Region"`); `addressCountry`
/// is a fallback ONLY when both locality and region are absent.
// ponytail: country used only as a fallback when no locality/region, to preserve
// the locality-first golden ("Berlin, BE" stays "Berlin, BE", not ", BE, DE").
fn fmt_address(addr: &serde_json::Value) -> Option<String> {
    let locality = addr.get("addressLocality").and_then(|s| s.as_str());
    let region = addr.get("addressRegion").and_then(|s| s.as_str());
    match (locality, region) {
        (Some(c), Some(r)) => Some(format!("{c}, {r}")),
        (Some(c), None) => Some(c.to_string()),
        (None, Some(r)) => Some(r.to_string()),
        (None, None) => addr
            .get("addressCountry")
            .and_then(|s| s.as_str())
            .map(str::to_string),
    }
}

/// Pull a display location from a `JobPosting`'s `jobLocation`, which may be a
/// single node OR an array of nodes. Each node's `address` is formatted via
/// [`fmt_address`]; multiple addresses join with `"; "`.
fn job_location(node: &serde_json::Value) -> Option<String> {
    let parts: Vec<String> = match node.get("jobLocation") {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|n| n.get("address").and_then(fmt_address))
            .collect(),
        Some(single) => single
            .get("address")
            .and_then(fmt_address)
            .into_iter()
            .collect(),
        None => return None,
    };
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

/// Does this node's `@type` denote a `JobPosting`? Tolerates a string or an
/// array of types (schema.org allows multiple types on one node).
fn is_job_posting(node: &serde_json::Value) -> bool {
    match node.get("@type") {
        Some(serde_json::Value::String(s)) => s == "JobPosting",
        Some(serde_json::Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some("JobPosting")),
        _ => false,
    }
}

/// Extract a [`JsonLdJob`] from a single node IF it is a titled `JobPosting`.
fn job_from_node(node: &serde_json::Value) -> Option<JsonLdJob> {
    if !is_job_posting(node) {
        return None;
    }
    let title = node
        .get("title")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if title.is_empty() {
        return None;
    }
    let description = node
        .get("description")
        .and_then(|s| s.as_str())
        .map(crate::scraping::http::strip_html)
        .filter(|s| !s.trim().is_empty());
    Some(JsonLdJob {
        title,
        description,
        location: job_location(node),
    })
}

/// Walk a JSON-LD value (object values, array elements, `@graph` — which is just
/// an object-valued array the recursion descends naturally) for the FIRST titled
/// `JobPosting` node.
// ponytail: depth-12 cap guards cyclic/pathologically-nested JSON-LD; raise it if
// a real page legitimately nests a JobPosting deeper.
fn find_job(node: &serde_json::Value, depth: u8) -> Option<JsonLdJob> {
    if let Some(job) = job_from_node(node) {
        return Some(job);
    }
    if depth >= 12 {
        return None;
    }
    match node {
        serde_json::Value::Array(arr) => arr.iter().find_map(|n| find_job(n, depth + 1)),
        serde_json::Value::Object(map) => map.values().find_map(|n| find_job(n, depth + 1)),
        _ => None,
    }
}

/// Best-effort pull of a JSON-LD `JobPosting` node from anywhere in any
/// `application/ld+json` block (top-level, `@graph`, or arbitrarily nested) —
/// title/description/jobLocation. Returns `None` when no `JobPosting` node
/// carries a usable title.
fn json_ld_job_posting(html: &str) -> Option<JsonLdJob> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(r#"script[type="application/ld+json"]"#).ok()?;
    for node in doc.select(&sel) {
        let raw = node.text().collect::<String>();
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if let Some(job) = find_job(&json, 0) {
            return Some(job);
        }
    }
    None
}

/// `__NEXT_DATA__` fallback: Next.js ships the page's props as JSON in a
/// `script#__NEXT_DATA__` blob. Find a `JobPosting` node OR, failing that, a
/// job-shaped node (a non-empty string `title` plus at least one of
/// `description` / `hiringOrganization` / `jobLocation`) and pull the same
/// fields the JSON-LD path does.
// ponytail: Next.js prop-shape sniffing; upgrade path = a per-board JSON path if a
// real page needs one. Same depth-12 cap as `find_job`.
fn next_data_job(html: &str) -> Option<JsonLdJob> {
    /// Pull a `JsonLdJob` out of a job-shaped node (already known to have a title).
    fn from_jobish(node: &serde_json::Value) -> JsonLdJob {
        let title = node
            .get("title")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let description = node
            .get("description")
            .and_then(|s| s.as_str())
            .map(crate::scraping::http::strip_html)
            .filter(|s| !s.trim().is_empty());
        JsonLdJob {
            title,
            description,
            location: job_location(node),
        }
    }
    fn is_jobish(node: &serde_json::Value) -> bool {
        let has_title = node
            .get("title")
            .and_then(|s| s.as_str())
            .is_some_and(|s| !s.trim().is_empty());
        has_title
            && (node.get("description").is_some()
                || node.get("hiringOrganization").is_some()
                || node.get("jobLocation").is_some())
    }
    fn find(node: &serde_json::Value, depth: u8) -> Option<JsonLdJob> {
        // Prefer a real JobPosting; else accept a job-shaped node.
        if let Some(job) = job_from_node(node) {
            return Some(job);
        }
        if is_jobish(node) {
            return Some(from_jobish(node));
        }
        if depth >= 12 {
            return None;
        }
        match node {
            serde_json::Value::Array(arr) => arr.iter().find_map(|n| find(n, depth + 1)),
            serde_json::Value::Object(map) => map.values().find_map(|n| find(n, depth + 1)),
            _ => None,
        }
    }

    let doc = Html::parse_document(html);
    let sel = Selector::parse("script#__NEXT_DATA__").ok()?;
    let raw = doc.select(&sel).next()?.text().collect::<String>();
    let json = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    find(&json, 0)
}

/// Largest main-content text block as a last-resort description: pick the longest
/// rendered text among `main` / `[role="main"]` / `article`.
// ponytail: largest-block heuristic, not a Readability port; good enough for ATS
// detail pages. Upgrade path = a real content-extraction crate if it falls short.
fn main_content_text(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(r#"main, [role="main"], article"#).ok()?;
    doc.select(&sel)
        .map(|el| crate::scraping::http::html_to_text(&el.inner_html()))
        .filter(|t| !t.trim().is_empty())
        .max_by_key(|t| t.len())
}

fn parse_generic_html(html: &str) -> (String, Option<String>) {
    let doc = Html::parse_document(html);
    let title_sel = Selector::parse("title, h1").unwrap();
    let title = doc
        .select(&title_sel)
        .next()
        .map(|e| e.text().collect::<String>().trim().to_string())
        .unwrap_or_default();
    let meta_sel =
        Selector::parse("meta[name=\"description\"], meta[property=\"og:description\"]").unwrap();
    let description = doc
        .select(&meta_sel)
        .next()
        .and_then(|e| e.value().attr("content").map(str::to_string));
    (title, description)
}

/// Best-effort real employer name for the generic fallback. Tries JSON-LD
/// (`JobPosting.hiringOrganization.name`, incl. an `@graph` array), then
/// `og:site_name`. Returns `None` when neither is present so the caller can
/// fall back to the host.
fn parse_generic_company(html: &str) -> Option<String> {
    let doc = Html::parse_document(html);

    if let Ok(sel) = Selector::parse(r#"script[type="application/ld+json"]"#) {
        for node in doc.select(&sel) {
            let raw = node.text().collect::<String>();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(name) = json_ld_company(&json) {
                    let name = name.trim();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse(r#"meta[property="og:site_name"]"#) {
        if let Some(name) = doc
            .select(&sel)
            .next()
            .and_then(|e| e.value().attr("content"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(name.to_string());
        }
    }

    None
}

/// Pull `hiringOrganization.name` from a JSON-LD value at any depth, tolerating a
/// single object, a string org, an `@graph` array, and arbitrary nesting.
// ponytail: same depth-12 cap as `find_job` — cyclic/pathological-nesting guard.
fn json_ld_company(json: &serde_json::Value) -> Option<String> {
    fn org_name(node: &serde_json::Value, depth: u8) -> Option<String> {
        match node.get("hiringOrganization") {
            Some(serde_json::Value::String(s)) => return Some(s.clone()),
            Some(org @ serde_json::Value::Object(_)) => {
                if let Some(name) = org.get("name").and_then(|n| n.as_str()) {
                    return Some(name.to_string());
                }
            }
            _ => {}
        }
        if depth >= 12 {
            return None;
        }
        match node {
            serde_json::Value::Array(arr) => arr.iter().find_map(|n| org_name(n, depth + 1)),
            serde_json::Value::Object(map) => map.values().find_map(|n| org_name(n, depth + 1)),
            _ => None,
        }
    }
    org_name(json, 0)
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

    let client = crate::net::http::shared();
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
        .map(crate::scraping::http::strip_html);

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

    let client = crate::net::http::shared();
    let res = client.get(&api).send().await?;
    if !res.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = res.json().await?;

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
                .map(crate::scraping::http::strip_html)
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

async fn try_personio(url: &str) -> Result<Option<JobPosting>> {
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
    let xml = res.text().await?;

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

#[cfg(test)]
mod test;
