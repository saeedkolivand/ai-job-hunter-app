#![allow(dead_code)]

/// Glassdoor — browser-based scraper (requires JavaScript rendering)
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use scraper::{Html, Selector};
use std::time::Duration;

pub struct GlassdoorScraper;

#[async_trait]
impl Scraper for GlassdoorScraper {
    fn id(&self) -> &'static str {
        "glassdoor"
    }

    fn display_name(&self) -> &'static str {
        "Glassdoor"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Browser
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let max_pages = input.pages.clamp(1, 10);
        let query = urlencoding::encode(&input.query);

        let (mut browser, mut handler) = Browser::launch(
            BrowserConfig::builder()
                .window_size(1920, 1080)
                .build()
                .map_err(|e| anyhow::anyhow!("Browser config error: {e}"))?,
        )
        .await?;

        let handle = tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });

        let page = browser.new_page("about:blank").await?;
        let mut results = Vec::new();

        for p in 1..=max_pages {
            if ctx.signal.is_cancelled() {
                break;
            }

            let url = if p == 1 {
                format!(
                    "https://www.glassdoor.com/Job/jobs.htm?sc.keyword={}&locT=C&locId=&jobType=&fromAge=-1&minSalary=0&includeNoSalaryJobs=true&radius=0&cityId=-1&minRating=0.0&industryId=-1&sgocId=-1&seniorityType=&companyId=-1&employerSizes=0&applicationType=0&remoteWorkType=0",
                    query
                )
            } else {
                format!(
                    "https://www.glassdoor.com/Job/jobs.htm?sc.keyword={}&locT=C&locId=&jobType=&fromAge=-1&minSalary=0&includeNoSalaryJobs=true&radius=0&cityId=-1&minRating=0.0&industryId=-1&sgocId=-1&seniorityType=&companyId=-1&employerSizes=0&applicationType=0&remoteWorkType=0&p={}",
                    query, p
                )
            };

            let html = match async {
                page.goto(&url).await?;
                page.wait_for_navigation().await?;
                tokio::time::sleep(Duration::from_secs(3)).await;
                anyhow::Ok(page.content().await?)
            }
            .await
            {
                Ok(html) => html,
                // First page failed → nothing collected → propagate.
                Err(e) if results.is_empty() => return Err(e),
                // Later page failed → keep the pages we already have.
                Err(e) => {
                    log::warn!("[glassdoor] page {p} failed: {e}; returning {} collected", results.len());
                    break;
                }
            };
            
            // Parse and extract in a scope to drop doc before await
            {
                let doc = Html::parse_document(&html);

                // Glassdoor job card selectors
                let card_sel = Selector::parse("li[data-test='jobListing']").unwrap();
                let title_sel = Selector::parse("a[data-test='job-link']").unwrap();
                let company_sel = Selector::parse("div[data-test='employer-name']").unwrap();
                let location_sel = Selector::parse("div[data-test='emp-location']").unwrap();

                for card in doc.select(&card_sel) {
                    if ctx.signal.is_cancelled() {
                        break;
                    }

                    let title = card
                        .select(&title_sel)
                        .next()
                        .and_then(|e| e.text().next())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    let company = card
                        .select(&company_sel)
                        .next()
                        .and_then(|e| e.text().next())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    let loc = card
                        .select(&location_sel)
                        .next()
                        .and_then(|e| e.text().next())
                        .map(|s| s.trim().to_string());

                    let url = card
                        .select(&title_sel)
                        .next()
                        .and_then(|e| e.value().attr("href"))
                        .map(|href| {
                            if href.starts_with("http") {
                                href.to_string()
                            } else {
                                format!("https://www.glassdoor.com{}", href)
                            }
                        })
                        .unwrap_or_default();

                    if title.is_empty() || company.is_empty() || url.is_empty() {
                        continue;
                    }

                    // Extract job ID from URL
                    let external_id = url
                        .split("jobListingId=")
                        .nth(1)
                        .and_then(|s| s.split('&').next())
                        .unwrap_or("")
                        .to_string();

                    let posting = JobPosting {
                        id: format!("glassdoor-{}", external_id),
                        source: "glassdoor".to_string(),
                        external_id: Some(external_id),
                        url: url.clone(),
                        title,
                        company,
                        location: loc,
                        description: Some(String::new()),
                        posted_at: None,
                        captured_at: chrono::Utc::now().timestamp_millis(),
                        requirements: None,
                        extra: std::collections::HashMap::new(),
                    };

                    if let Some(ref cb) = ctx.on_item {
                        cb(posting.clone());
                    }
                    results.push(posting);
                }
            } // doc is dropped here

            if let Some(ref cb) = ctx.on_progress {
                cb(p as f32 / max_pages as f32);
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        browser.close().await?;
        let _ = handle.await;

        Ok(results)
    }

    async fn from_url(
        &self,
        _url: &str,
        _ctx: ScrapeContext,
    ) -> anyhow::Result<Option<JobPosting>> {
        // Glassdoor from_url would require browser automation to extract job details
        // For now, return None to indicate it's not supported
        Ok(None)
    }
}

#[cfg(test)]
mod test;
