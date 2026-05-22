#![allow(dead_code)]
use super::client::LinkedInHttpClient;
use super::session::LinkedInSessionData;
use crate::scraping::types::JobPosting;
use anyhow::Result;
use scraper::Html;
use std::collections::HashSet;

const PAGE_SIZE: usize = 25;

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
        eprintln!("[LinkedIn API] Searching with keywords: '{}', location: {:?}, start: {}", 
            params.keywords, params.location, params.start);
        let f_tpr = match params.date_filter.as_deref() {
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

        if let Some(ref job_type) = params.job_type {
            url.push_str(&format!("&f_JT={}", job_type));
        }

        if !f_tpr.is_empty() {
            url.push_str(&format!("&f_TPR={}", f_tpr));
        }

        if let Some(ref work_type) = params.work_type {
            url.push_str(&format!("&f_WT={}", work_type));
        }

        if let Some(ref experience_level) = params.experience_level {
            url.push_str(&format!("&f_E={}", experience_level));
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
            url.push_str(&format!("&sortBy={}", sort_by));
        }

        eprintln!("[LinkedIn API] Request URL: {}", url);
        
        let html = self.client.get_html(&url, signal).await?;
        
        eprintln!("[LinkedIn API] Parsing HTML response...");
        let document = Html::parse_document(&html);
        let selector = scraper::Selector::parse("li").unwrap();
        let link_selector = scraper::Selector::parse("a.base-card__full-link, a.base-search-card__link").unwrap();
        let urn_selector = scraper::Selector::parse("[data-entity-urn]").unwrap();
        let title_selector = scraper::Selector::parse(".base-search-card__title, .job-card-container__title").unwrap();
        let company_selector = scraper::Selector::parse(".base-search-card__subtitle, .job-card-container__subtitle").unwrap();
        let location_selector = scraper::Selector::parse(".job-search-card__location, .job-card-container__location").unwrap();
        let time_selector = scraper::Selector::parse("time").unwrap();

        let mut seen = HashSet::new();
        let mut jobs = Vec::new();
        let _now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        for element in document.select(&selector) {
            if let Some(signal) = signal {
                if signal.is_cancelled() {
                    break;
                }
            }

            let link = element
                .select(&link_selector)
                .next()
                .and_then(|el| el.value().attr("href"))
                .unwrap_or("");

            let entity_urn = element
                .select(&urn_selector)
                .next()
                .and_then(|el| el.value().attr("data-entity-urn"));

            let id = entity_urn.and_then(|urn| urn.split(':').last());

            let id = match id {
                Some(id_str) => {
                    if seen.contains(id_str) {
                        continue;
                    }
                    seen.insert(id_str.to_string());
                    id_str
                }
                None => continue,
            };

            let title = element
                .select(&title_selector)
                .next()
                .map(|el| el.text().collect::<String>())
                .unwrap_or_default()
                .trim()
                .to_string();

            let company = element
                .select(&company_selector)
                .next()
                .map(|el| el.text().collect::<String>())
                .unwrap_or_default()
                .trim()
                .to_string();

            let location = element
                .select(&location_selector)
                .next()
                .map(|el| el.text().collect::<String>())
                .unwrap_or_default()
                .trim()
                .to_string();

            let date_attr = element
                .select(&time_selector)
                .next()
                .and_then(|el| el.value().attr("datetime"));

            let posted_at = date_attr.and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok());

            let job = JobPosting {
                id: format!("linkedin:{}", id),
                source: "linkedin".to_string(),
                external_id: Some(id.to_string()),
                url: link.split('?').next().unwrap_or("").to_string(),
                title,
                company,
                location: Some(location),
                description: Some(String::new()), // Will be filled in background
                captured_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64,
                posted_at: posted_at.map(|dt| dt.timestamp_millis()),
                requirements: None,
                extra: std::collections::HashMap::new(),
            };

            jobs.push(job);
        }

        eprintln!("[LinkedIn API] Found {} jobs on this page", jobs.len());
        Ok(jobs)
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
        let max_pages = pages.min(10).max(1);
        let mut all_jobs = Vec::new();
        let mut seen = HashSet::new();

        for page in 0..max_pages {
            if let Some(signal) = signal {
                if signal.is_cancelled() {
                    break;
                }
            }

            let start = page * PAGE_SIZE;
            let mut search_params = params.clone();
            search_params.start = start;

            let jobs = self.search_guest(&search_params, signal).await?;

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

            // Add delay between pages
            if page < max_pages - 1 {
                tokio::time::sleep(tokio::time::Duration::from_millis(500 + (rand::random::<u64>() % 500)))
                    .await;
            }
        }

        if let Some(on_progress) = on_progress {
            on_progress(1.0);
        }

        Ok(all_jobs)
    }

    pub fn update_session(&mut self, session_data: LinkedInSessionData) {
        self.client.update_session(session_data);
    }
}
