use crate::scraping::linkedin::api_client::{JobsSearchParams, LinkedInJobsApiClient};
use crate::scraping::linkedin::client::LinkedInHttpClient;
use crate::scraping::types::BoardSearchInput;
use crate::scraping::types::{JobPosting, ScrapeContext, Scraper};
use anyhow::Result;
use async_trait::async_trait;

pub struct LinkedInScraper;

#[async_trait]
impl Scraper for LinkedInScraper {
    fn id(&self) -> &'static str {
        "linkedin"
    }

    fn display_name(&self) -> &'static str {
        "LinkedIn"
    }

    fn mode(&self) -> crate::scraping::types::ScraperMode {
        crate::scraping::types::ScraperMode::Http
    }

    async fn search(&self, input: BoardSearchInput, ctx: ScrapeContext) -> Result<Vec<JobPosting>> {
        // Try to load session data from disk
        let session_data = self.load_session_data().await;
        if session_data.is_some() {
            // Touch the auth-status heartbeat so freshness countdown restarts.
            crate::scraping::board_login::touch_session(
                &crate::platform::config::data_dir(),
                "linkedin",
            );
        }

        // Create HTTP client with session
        let http_client = LinkedInHttpClient::new(session_data);
        let api_client = LinkedInJobsApiClient::new(http_client);

        let params = JobsSearchParams {
            keywords: input.query.clone(),
            location: input.location.clone(),
            start: 0,
            date_filter: input.date_filter.clone(),
            job_type: input.job_type.clone(),
            work_type: input.work_type.clone(),
            experience_level: input.experience_level.clone(),
            easy_apply: input.easy_apply,
            actively_hiring: input.actively_hiring,
            verified: input.verified,
            sort_by: input.sort_by.clone(),
        };

        let max_pages = input.pages.clamp(1, 10) as usize;
        let signal = Some(&ctx.signal);

        api_client
            .search_paginated(&params, max_pages, signal, ctx.on_progress, ctx.on_item)
            .await
    }
}

impl LinkedInScraper {
    /// Load LinkedIn session by reading the cookies.json exported by the
    /// board login flow (`board_login::open_login`). Returns None if the user
    /// has not logged in or the cookies don't contain `li_at`.
    async fn load_session_data(
        &self,
    ) -> Option<crate::scraping::linkedin::session::LinkedInSessionData> {
        use crate::scraping::board_login;
        use crate::scraping::linkedin::session::{Cookie, LinkedInSessionData};

        let data_dir = crate::platform::config::data_dir();
        if board_login::session_is_stale(&data_dir, "linkedin") {
            log::warn!("[linkedin] session is stale — falling back to guest mode");
            return None;
        }
        let cookies = board_login::load_cookies(&data_dir, "linkedin");
        if cookies.is_empty() {
            return None;
        }

        let li_at = cookies
            .iter()
            .find(|c| c.name == "li_at" && c.domain.contains("linkedin.com"))?
            .value
            .clone();

        let jsession_id = cookies
            .iter()
            .find(|c| c.name == "JSESSIONID" && c.domain.contains("linkedin.com"))
            .map(|c| c.value.trim_matches('"').to_string());

        // LinkedIn embeds the CSRF token in JSESSIONID as `ajax:<token>`.
        let csrf_token = jsession_id
            .as_ref()
            .and_then(|v| v.strip_prefix("ajax:").map(str::to_string));

        let mapped_cookies = cookies
            .iter()
            .map(|c| Cookie {
                name: c.name.clone(),
                value: c.value.clone(),
                domain: Some(c.domain.clone()),
                path: Some(c.path.clone()),
                expires: c.expires,
            })
            .collect();

        Some(LinkedInSessionData {
            cookies: mapped_cookies,
            li_at,
            jsession_id,
            csrf_token,
            last_updated: chrono::Utc::now().timestamp_millis() as u64,
        })
    }
}

#[cfg(test)]
mod test;
