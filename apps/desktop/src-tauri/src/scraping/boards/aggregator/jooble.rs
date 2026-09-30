//! The Jooble `JobProvider` (last-resort fallback tier) — split out of
//! `providers.rs` (R8 module-size guard).
//!
//! Visibility: items here are `pub(super)` (visible to `aggregator` and its
//! descendants, including `tests`) rather than fully private, purely to
//! preserve this behavior-preserving move.
use async_trait::async_trait;
use serde::Deserialize;

use crate::observability::sanitize_reason;
use crate::scraping::http::{fetch_json, html_to_markdown, FetchOptions};
use crate::scraping::types::JobPosting;

use super::serde_helpers::de_opt_string_or_number;
use super::JobProvider;

// ── Jooble provider (last-resort fallback, after Adzuna + JSearch) ──────────────

#[derive(Debug, Deserialize)]
pub(super) struct JoobleJob {
    #[serde(default, deserialize_with = "de_opt_string_or_number")]
    pub(super) id: Option<String>,
    pub(super) title: Option<String>,
    pub(super) company: Option<String>,
    pub(super) location: Option<String>,
    /// TRUNCATED description — same caveat as Adzuna's `description`: never
    /// treat this as full job text.
    pub(super) snippet: Option<String>,
    /// Free-text salary string (e.g. "$50,000 - $70,000"); not parsed into a
    /// number — stashed verbatim in `extra.salaryText` rather than risking a
    /// wrong numeric parse of an undocumented format.
    pub(super) salary: Option<String>,
    pub(super) link: Option<String>,
    /// ISO-8601.
    pub(super) updated: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct JoobleResp {
    pub(super) jobs: Vec<JoobleJob>,
}

/// Production Jooble host. Factored into a constant (rather than inlined in
/// [`fetch_jooble`]) purely so `JoobleProvider::search` reads as "the real
/// host" at a glance; tests pass a local `wiremock` base instead.
pub(super) const JOOBLE_BASE_URL: &str = "https://jooble.org";

/// Build the Jooble search endpoint for a given base + key. Factored out of
/// [`fetch_jooble`] so the URL shape (key in the PATH, not a header/query) is
/// independently testable.
pub(super) fn jooble_endpoint(base_url: &str, api_key: &str) -> String {
    format!("{base_url}/api/{}", urlencoding::encode(api_key))
}

/// Map one Jooble result to a [`JobPosting`]. Drops items missing a usable
/// title or link (mirrors JSearch's `filter_map` policy — such an item can't
/// be shown or opened). Pulled out of [`fetch_jooble`] so the mapping is
/// unit-testable without a network call.
pub(super) fn map_jooble_job(j: JoobleJob, now: i64) -> Option<JobPosting> {
    let title = j
        .title
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let url = j
        .link
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;
    let id_part =
        j.id.map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| url.clone());

    let mut extra = std::collections::HashMap::new();
    if let Some(salary) = j
        .salary
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        extra.insert("salaryText".to_string(), serde_json::json!(salary));
    }

    Some(JobPosting {
        id: format!("aggregator:jooble-{id_part}"),
        external_id: Some(format!("jooble-{id_part}")),
        title,
        company: j.company.unwrap_or_default(),
        location: j
            .location
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        url,
        source: "aggregator".to_string(),
        description: j.snippet.map(|d| html_to_markdown(&d)),
        requirements: None,
        // Jooble's real `updated` value has NO timezone offset (e.g.
        // "2026-05-15T00:00:00.0000000") despite looking ISO-8601-ish, so plain
        // RFC3339 parsing fails for every live job — `posted_at` silently stayed
        // `None` for all of them. Try RFC3339 first (in case an entry ever does
        // carry an offset), then fall back to a naive datetime — tolerant of
        // Jooble's fractional-seconds-no-tz form — assumed UTC.
        posted_at: j.updated.as_deref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp_millis())
                .or_else(|| {
                    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
                        .ok()
                        .map(|ndt| ndt.and_utc().timestamp_millis())
                })
        }),
        captured_at: now,
        extra,
    })
}

/// Build + fetch + parse the Jooble search response for a given base URL.
/// Factored out of `JoobleProvider::search` (mirrors `fetch_adzuna_page`'s
/// testable-base pattern) so the non-2xx / mapping contract is unit-testable
/// against a local mock server without a real key or hitting the live host.
pub(super) async fn fetch_jooble(
    base_url: &str,
    api_key: &str,
    query: &str,
    location: &str,
    amount: Option<u32>,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    // Jooble's contract puts the key in the URL PATH, not a header or query
    // param: `POST /api/{apiKey}`. `redact_path: true` below keeps it out of
    // `fetch_json`'s non-2xx / schema-drift log lines (the shared query-only
    // redaction doesn't cover a path-embedded secret) — including on the exact
    // "bad key" 403 that would otherwise echo it straight back.
    let url = jooble_endpoint(base_url, api_key);

    let body = match amount {
        Some(n) => {
            serde_json::json!({ "keywords": query, "location": location, "ResultOnPage": n })
        }
        None => serde_json::json!({ "keywords": query, "location": location }),
    };

    // A non-2xx or schema-drift response propagates as `Err` from `fetch_json`
    // (carrying the HTTP status); `?` surfaces it as a provider failure. The
    // "jooble:" prefix is required — the aggregator board fronts multiple
    // providers, so an unattributed "HTTP 403" in BoardScrapeSummary.error
    // wouldn't say which one failed.
    let resp = fetch_json::<JoobleResp>(
        &url,
        FetchOptions {
            method: Some(reqwest::Method::POST),
            body: Some(body.to_string()),
            headers: Some(vec![(
                "content-type".to_string(),
                "application/json".to_string(),
            )]),
            redact_path: true,
            ..FetchOptions::default()
        },
        signal,
    )
    .await
    .map_err(|e| anyhow::anyhow!("jooble: {e}"))?;

    let now = chrono::Utc::now().timestamp_millis();
    Ok(resp
        .jobs
        .into_iter()
        .filter_map(|j| map_jooble_job(j, now))
        .collect())
}

pub(crate) struct JoobleProvider {
    pub(super) api_key: Option<String>,
}

impl JoobleProvider {
    pub(super) fn new() -> Self {
        use crate::ipc_contracts::provider_slots::JOOBLE_KEY;
        Self {
            api_key: crate::credentials::read_credential(&format!("ai:{JOOBLE_KEY}"))
                .unwrap_or_else(|e| {
                    log::warn!(
                        "[aggregator] {JOOBLE_KEY} keyring error: {}",
                        sanitize_reason(&e.to_string())
                    );
                    None
                }),
        }
    }
}

#[async_trait]
impl JobProvider for JoobleProvider {
    fn provider_id(&self) -> &'static str {
        "jooble"
    }

    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }

    async fn search(
        &self,
        query: &str,
        location: &str,
        _country: &str,
        _country_guessed: bool,
        _date_filter: Option<&str>,
        amount: Option<u32>,
        signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        if !self.is_configured() {
            return Err(anyhow::anyhow!("jooble: not configured"));
        }
        let api_key = self.api_key.as_deref().unwrap_or("");
        fetch_jooble(JOOBLE_BASE_URL, api_key, query, location, amount, signal).await
    }
}
