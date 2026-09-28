mod card_parse;

use super::client::LinkedInHttpClient;
use super::session::LinkedInSessionData;
use crate::observability::sanitize_reason;
use crate::scraping::types::JobPosting;
use anyhow::Result;
use std::collections::HashSet;

// The guest seeMoreJobPostings endpoint returns 10 cards per request; stepping
// `start` by 25 skipped jobs 10-24, 35-49, … Confirmed against the live endpoint.
const PAGE_SIZE: usize = 10;

/// Sleep for `dur`, aborting early if `signal` fires. Returns `true` if
/// cancellation interrupted the sleep (caller should stop), `false` if it
/// elapsed normally. Keeps the scrape responsive to cancel during backoff.
pub(crate) async fn cancellable_sleep(
    signal: Option<&tokio_util::sync::CancellationToken>,
    dur: std::time::Duration,
) -> bool {
    match signal {
        Some(sig) => tokio::select! {
            _ = sig.cancelled() => true,
            _ = tokio::time::sleep(dur) => false,
        },
        None => {
            tokio::time::sleep(dur).await;
            false
        }
    }
}

/// Structural discriminator for LinkedIn's guest `seeMoreJobPostings/search`
/// endpoint: whether a **page-0** 200 response that parsed to `card_count` job
/// cards indicates a soft-block rather than a genuine empty result.
///
/// Verified live 2026-07-11: the guest endpoint pads ANY real page-0 query with
/// talent-pool / "spontaneous application" cards (a real, non-block query never
/// returns an empty card list on page 0), and paging past the end returns HTTP
/// 400 (already surfaced as an error upstream). So a 200 page-0 body that yields
/// **zero** job cards is never "0 jobs found" — it is an anti-bot soft-block, a
/// login-wall interstitial, or a card-markup change. Surfacing it as a board
/// error (not the previous silent `Ok(vec![])`) is what makes LinkedIn honest.
///
/// Only page 0 is treated this way: a later page legitimately returns zero once
/// the harvest is exhausted, and by then real cards were already collected.
fn page0_is_soft_block(card_count: usize) -> bool {
    card_count == 0
}

#[derive(Debug, Clone)]
pub struct JobsSearchParams {
    pub keywords: String,
    pub location: Option<String>,
    pub start: usize,
    pub date_filter: Option<String>,
    pub job_type: Option<String>,
    pub work_type: Option<String>,
    pub experience_level: Option<String>,
    pub easy_apply: Option<bool>,
    pub actively_hiring: Option<bool>,
    pub verified: Option<bool>,
    pub sort_by: Option<String>,
    /// Precise LinkedIn geoId (resolved via typeahead) — far more reliable than
    /// the free-text `location` filter, which leaks results across countries (#49).
    pub geo_id: Option<String>,
    /// Search radius in km around the location (`distance` param, #40).
    pub distance: Option<u32>,
}

pub struct LinkedInJobsApiClient {
    client: LinkedInHttpClient,
}

impl LinkedInJobsApiClient {
    pub fn new(client: LinkedInHttpClient) -> Self {
        Self { client }
    }

    /// Search jobs using the guest API (no authentication required).
    pub async fn search_guest(
        &self,
        params: &JobsSearchParams,
        signal: Option<&tokio_util::sync::CancellationToken>,
    ) -> Result<Vec<JobPosting>> {
        let f_tpr = match params.date_filter.as_deref() {
            Some("15m") => "r900",
            Some("30m") => "r1800",
            Some("1h") => "r3600",
            Some("2h") => "r7200",
            Some("4h") => "r14400",
            Some("8h") => "r28800",
            Some("24h") => "r86400",
            Some("week") => "r604800",
            Some("month") => "r2592000",
            _ => "",
        };

        let mut url = format!(
            "https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search?keywords={}&start={}",
            urlencoding::encode(&params.keywords),
            params.start
        );

        if let Some(ref location) = params.location {
            url.push_str(&format!("&location={}", urlencoding::encode(location)));
        }

        // A resolved geoId pins the search to the exact place (country-correct);
        // distance widens it to a radius. Both are best-effort — absent them, the
        // free-text `location` filter above is used (current behavior).
        if let Some(ref geo_id) = params.geo_id {
            url.push_str(&format!("&geoId={}", urlencoding::encode(geo_id)));
        }
        if let Some(distance) = params.distance {
            url.push_str(&format!("&distance={distance}"));
        }

        if let Some(ref job_type) = params.job_type {
            url.push_str(&format!("&f_JT={}", urlencoding::encode(job_type)));
        }

        if !f_tpr.is_empty() {
            url.push_str(&format!("&f_TPR={}", f_tpr));
        }

        if let Some(ref work_type) = params.work_type {
            url.push_str(&format!("&f_WT={}", urlencoding::encode(work_type)));
        }

        if let Some(ref experience_level) = params.experience_level {
            url.push_str(&format!("&f_E={}", urlencoding::encode(experience_level)));
        }

        if params.easy_apply.unwrap_or(false) {
            url.push_str("&f_EA=true");
        }

        if params.actively_hiring.unwrap_or(false) {
            url.push_str("&f_AL=true");
        }

        if params.verified.unwrap_or(false) {
            url.push_str("&f_VJ=true");
        }

        if let Some(ref sort_by) = params.sort_by {
            url.push_str(&format!("&sortBy={}", urlencoding::encode(sort_by)));
        }

        let html = self.client.get_html(&url, signal).await?;
        Ok(card_parse::parse_cards(&html, signal))
    }

    /// Search jobs with pagination support.
    pub async fn search_paginated(
        &self,
        params: &JobsSearchParams,
        pages: usize,
        signal: Option<&tokio_util::sync::CancellationToken>,
        on_progress: Option<Box<dyn Fn(f32) + Send>>,
        on_item: Option<Box<dyn Fn(JobPosting) + Send>>,
    ) -> Result<Vec<JobPosting>> {
        let max_pages = pages.clamp(1, 10);
        let mut all_jobs = Vec::new();
        let mut seen = HashSet::new();

        // `effective` tracks the params we actually send to LinkedIn.  On page 0 we
        // start with the caller-supplied params (which may include a geoId).  If the
        // first response comes back empty while a geoId is set, LinkedIn is
        // soft-blocking the geo-filtered query; we strip geoId + distance from
        // `effective` and retry once so that subsequent pages also skip the geoId.
        let mut effective = params.clone();

        for page in 0..max_pages {
            if let Some(signal) = signal {
                if signal.is_cancelled() {
                    break;
                }
            }

            let start = page * PAGE_SIZE;
            let mut search_params = effective.clone();
            search_params.start = start;

            let mut jobs = match self.search_guest(&search_params, signal).await {
                Ok(jobs) => jobs,
                // First page failed → nothing collected → propagate as a real failure.
                Err(e) if all_jobs.is_empty() => return Err(e),
                // A later page failed → keep the pages we already have (and streamed).
                Err(e) => {
                    log::warn!(
                        "[linkedin] page {page} failed: {}; returning {} collected",
                        sanitize_reason(&e.to_string()),
                        all_jobs.len()
                    );
                    break;
                }
            };

            // Page-0 soft-block detection: LinkedIn returns an empty result set when
            // a geoId filter is applied to the guest endpoint.  Fall back to a
            // free-text location query (no geoId / no distance) and keep it for all
            // remaining pages by mutating `effective`.
            if page == 0 && jobs.is_empty() && effective.geo_id.is_some() {
                log::info!(
                    "[linkedin] geoId-filtered search returned 0 results; retrying with free-text location only"
                );
                effective.geo_id = None;
                effective.distance = None;

                // Jittered, cancellation-aware pause so the retry isn't fired back-to-back
                // with the soft-blocked request (avoids LinkedIn's anti-bot velocity boundary).
                if cancellable_sleep(
                    signal,
                    std::time::Duration::from_millis(300 + (rand::random::<u64>() % 300)),
                )
                .await
                {
                    break;
                }

                let mut retry_params = effective.clone();
                retry_params.start = start;

                jobs = match self.search_guest(&retry_params, signal).await {
                    Ok(jobs) => jobs,
                    Err(e) => return Err(e),
                };
            }

            // Honest block detection: a page-0 200 that yields zero job cards is a
            // soft-block / login-wall / markup drift, NOT a genuine empty result
            // (see `page0_is_soft_block`). Surface it as a board error instead of
            // the previous silent `Ok(vec![])`. Guarded on cancellation so a cancel
            // firing between fetch and here isn't mislabelled as a block.
            if page == 0
                && page0_is_soft_block(jobs.len())
                && !signal.is_some_and(|s| s.is_cancelled())
            {
                return Err(anyhow::anyhow!(
                    "LinkedIn returned no job cards — it may be rate-limiting guest traffic, \
                     require login, or have changed its page layout; results unavailable"
                ));
            }

            for job in &jobs {
                let job_id = job.external_id.clone().unwrap_or_else(|| job.id.clone());
                if !seen.contains(&job_id) {
                    seen.insert(job_id);
                    if let Some(ref on_item) = on_item {
                        on_item(job.clone());
                    }
                    all_jobs.push(job.clone());
                }
            }

            if jobs.is_empty() {
                break;
            }

            // Report incremental progress after each successful page.
            if let Some(ref on_progress) = on_progress {
                on_progress((page + 1) as f32 / max_pages as f32);
            }

            // Add delay between pages (cancellation-aware).
            if page < max_pages - 1
                && cancellable_sleep(
                    signal,
                    std::time::Duration::from_millis(500 + (rand::random::<u64>() % 500)),
                )
                .await
            {
                break;
            }
        }

        // Ensure progress reaches exactly 1.0 on completion.
        if let Some(on_progress) = on_progress {
            on_progress(1.0);
        }

        Ok(all_jobs)
    }

    pub fn update_session(&mut self, session_data: LinkedInSessionData) {
        self.client.update_session(session_data);
    }
}

#[cfg(test)]
mod test;
