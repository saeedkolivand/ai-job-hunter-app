//! Comeet — credentialed job-positions API (single company)
//!
//! Endpoint: `https://www.comeet.co/careers-api/2.0/company/{uid}/positions?token={token}`
//! Requires a company UID + API token, both read from the OS keyring at
//! scrape time — same `ai:`-namespaced credential-slot convention as the
//! Apify LinkedIn provider (`scraping/boards/aggregator/mod.rs`). Unlike the
//! company-scoped ATS boards (Greenhouse, Breezy, …), `requires_company()`
//! stays `false`: "company" here is a fixed per-user credential (one Comeet
//! tenant per app install), not a per-search `companies[]` input.
//!
//! ponytail: the live endpoint 400s without real credentials, so the response
//! shape below is built from the career-ops (MIT) field spec, not a captured
//! live payload — `name`, `url_comeet_hosted_page`/`url_active_page`,
//! `location.{name,city,country}`, `uid`, `time_updated`. Needs
//! live-verification with a real company UID + token via the Settings UI
//! (Pass 3). `CmPosition::company_name` below is a further speculative guess
//! not in that field spec at all — see its own doc comment.
use super::super::http::fetch_json;
use super::super::types::{
    AuthRequirement, BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode,
};
use super::common::matches_filters;
use async_trait::async_trait;
use serde::Deserialize;

const BOARD_ID: &str = "comeet";

#[derive(Debug, Deserialize)]
struct CmLocation {
    name: Option<String>,
    city: Option<String>,
    country: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CmPosition {
    name: Option<String>,
    uid: Option<String>,
    url_comeet_hosted_page: Option<String>,
    url_active_page: Option<String>,
    location: Option<CmLocation>,
    time_updated: Option<String>,
    // NOT in the career-ops field spec handed to this pass — a speculative,
    // "if present" company-name guess (the caller falls back to the
    // configured company uid when this is absent, which — until live
    // verification — it always will be).
    #[serde(default, alias = "company")]
    company_name: Option<String>,
}

/// Deserialize each position row independently, dropping (with a debug log)
/// any row that fails to type-check. The live shape is unconfirmed (see
/// module doc comment), so a single drifted/unexpected row must not fail the
/// whole batch — mirrors Rippling's/Workable's `rows_to_jobs` resilience.
pub(crate) fn rows_to_jobs(values: Vec<serde_json::Value>) -> Vec<CmPosition> {
    let total = values.len();
    let jobs: Vec<CmPosition> = values
        .into_iter()
        .filter_map(|v| match serde_json::from_value::<CmPosition>(v) {
            Ok(job) => Some(job),
            Err(e) => {
                log::debug!("[comeet] skipping malformed row: {e}");
                None
            }
        })
        .collect();
    let skipped = total - jobs.len();
    if skipped > 0 {
        log::warn!("[comeet] skipped {skipped}/{total} malformed rows");
    }
    jobs
}

/// Validate that a job URL from the response is `https://comeet.co/…` (or a
/// `*.comeet.co` subdomain, reusing the same label-boundary-anchored suffix
/// match the trust module's ATS allowlist uses). A drifting or hostile
/// response could inject arbitrary URLs into `JobPosting.url`; constrain it
/// to Comeet's own host.
fn is_valid_comeet_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .map(|u| {
            u.scheme() == "https"
                && u.host_str()
                    .is_some_and(|h| crate::scraping::trust::matches_domain_list(h, &["comeet.co"]))
        })
        .unwrap_or(false)
}

/// `time_updated` is documented (career-ops spec) as a Unix-seconds epoch;
/// tolerate an RFC3339 string too in case the live shape differs (unconfirmed).
fn parse_comeet_time(s: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    s.trim().parse::<i64>().ok().map(|n| n.saturating_mul(1000))
}

/// Map parsed Comeet positions into postings for the configured company.
/// Standalone (no `&self`) so it is unit-testable against a career-ops-shaped
/// fixture, without needing real credentials or a network round-trip.
pub(crate) fn parse_comeet_response(
    positions: Vec<CmPosition>,
    company_uid: &str,
    now: i64,
) -> Vec<JobPosting> {
    let mut out = Vec::new();

    for p in positions {
        let title = p.name.unwrap_or_default().trim().to_string();
        if title.is_empty() {
            continue;
        }

        let uid = match p.uid.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(u) => u.to_string(),
            None => continue,
        };

        let url = match p
            .url_comeet_hosted_page
            .as_deref()
            .or(p.url_active_page.as_deref())
            .map(str::trim)
        {
            Some(u) if is_valid_comeet_url(u) => u.to_string(),
            _ => continue,
        };

        let location = p.location.and_then(|l| {
            if let Some(name) = l.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                return Some(name.to_string());
            }
            let parts: Vec<String> = [l.city, l.country]
                .into_iter()
                .flatten()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            (!parts.is_empty()).then(|| parts.join(", "))
        });

        let company = p
            .company_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| company_uid.to_string());

        let posted_at = p.time_updated.as_deref().and_then(parse_comeet_time);

        out.push(JobPosting {
            id: format!("{BOARD_ID}:{uid}"),
            external_id: Some(uid),
            title,
            company,
            location,
            url,
            source: BOARD_ID.to_string(),
            description: None,
            requirements: None,
            posted_at,
            captured_at: now,
            extra: std::collections::HashMap::new(),
        });
    }

    out
}

pub struct ComeetScraper;

#[async_trait]
impl Scraper for ComeetScraper {
    fn id(&self) -> &'static str {
        BOARD_ID
    }

    fn display_name(&self) -> &'static str {
        "Comeet"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    // Explicit override (matches the trait default) — mirrors the Aggregator
    // board's own explicit `Guest` override: credentials are read internally
    // at scrape time, not surfaced through the board-login
    // `AuthRequirement::Required` connect flow.
    fn auth(&self) -> AuthRequirement {
        AuthRequirement::Guest
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        use crate::ipc_contracts::provider_slots::{COMEET_API_TOKEN, COMEET_COMPANY_UID};

        let company_uid = crate::credentials::read_credential(&format!("ai:{COMEET_COMPANY_UID}"))
            .unwrap_or_else(|e| {
                log::warn!("[comeet] {COMEET_COMPANY_UID} keyring error: {e}");
                None
            })
            .filter(|s| !s.trim().is_empty());
        let token = crate::credentials::read_credential(&format!("ai:{COMEET_API_TOKEN}"))
            .unwrap_or_else(|e| {
                log::warn!("[comeet] {COMEET_API_TOKEN} keyring error: {e}");
                None
            })
            .filter(|s| !s.trim().is_empty());

        // Not configured → keyless-empty, same contract as the aggregator's
        // providers: never an error just because the user hasn't added keys.
        let (company_uid, token) = match (company_uid, token) {
            (Some(u), Some(t)) => (u, t),
            _ => return Ok(vec![]),
        };

        if ctx.signal.is_cancelled() {
            return Ok(vec![]);
        }

        let url = format!(
            "https://www.comeet.co/careers-api/2.0/company/{}/positions?token={}",
            urlencoding::encode(company_uid.trim()),
            urlencoding::encode(token.trim())
        );

        let raw = match fetch_json::<Vec<serde_json::Value>>(
            &url,
            Default::default(),
            ctx.signal.clone(),
        )
        .await
        {
            Ok(d) => d,
            // A cancel firing mid-fetch is a clean stop, not a board error.
            Err(_) if ctx.signal.is_cancelled() => return Ok(vec![]),
            // A non-2xx / schema-drift response now surfaces as a board error
            // instead of a silent empty result.
            Err(e) => return Err(e.into()),
        };

        let now = chrono::Utc::now().timestamp_millis();
        let positions = rows_to_jobs(raw);
        let mut out = Vec::new();

        for posting in parse_comeet_response(positions, company_uid.trim(), now) {
            if !matches_filters(
                &posting,
                &input.query,
                input.location.as_deref().unwrap_or(""),
            ) {
                continue;
            }
            if let Some(ref on_item) = ctx.on_item {
                on_item(posting.clone());
            }
            out.push(posting);
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;
