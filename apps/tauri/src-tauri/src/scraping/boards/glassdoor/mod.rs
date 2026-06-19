/// Glassdoor — browser-based scraper (requires JavaScript rendering).
///
/// BEST-EFFORT ANONYMOUS: launches a browser with no cookies or auth.
/// Glassdoor frequently bot-blocks anonymous sessions with sign-in or
/// captcha walls. Full login-wiring is out of scope for this scraper.
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use scraper::{Html, Selector};
use std::time::Duration;

// Glassdoor job-card CSS selectors compiled once (Selector is Send + Sync).
static GD_CARD_SEL: std::sync::LazyLock<Selector> =
    std::sync::LazyLock::new(|| Selector::parse("li[data-test='jobListing']").unwrap());
static GD_TITLE_SEL: std::sync::LazyLock<Selector> =
    std::sync::LazyLock::new(|| Selector::parse("a[data-test='job-link']").unwrap());
static GD_COMPANY_SEL: std::sync::LazyLock<Selector> =
    std::sync::LazyLock::new(|| Selector::parse("div[data-test='employer-name']").unwrap());
static GD_LOCATION_SEL: std::sync::LazyLock<Selector> =
    std::sync::LazyLock::new(|| Selector::parse("div[data-test='emp-location']").unwrap());

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
        let loc = urlencoding::encode(input.location.as_deref().unwrap_or(""));

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

        let page = match browser.new_page("about:blank").await {
            Ok(p) => p,
            Err(e) => {
                let _ = browser.close().await;
                let _ = handle.await;
                return Err(e.into());
            }
        };
        let mut results = Vec::new();
        // Tracks the first-page navigation error so we can propagate it after
        // closing the browser (browser.close() must run on all exit paths).
        let mut first_page_err: Option<anyhow::Error> = None;

        'pages: for p in 1..=max_pages {
            if ctx.signal.is_cancelled() {
                break;
            }

            let url = if p == 1 {
                format!(
                    "https://www.glassdoor.com/Job/jobs.htm?sc.keyword={}&locT=C&locId=&locKeyword={}&jobType=&fromAge=-1&minSalary=0&includeNoSalaryJobs=true&radius=0&cityId=-1&minRating=0.0&industryId=-1&sgocId=-1&seniorityType=&companyId=-1&employerSizes=0&applicationType=0&remoteWorkType=0",
                    query, loc
                )
            } else {
                format!(
                    "https://www.glassdoor.com/Job/jobs.htm?sc.keyword={}&locT=C&locId=&locKeyword={}&jobType=&fromAge=-1&minSalary=0&includeNoSalaryJobs=true&radius=0&cityId=-1&minRating=0.0&industryId=-1&sgocId=-1&seniorityType=&companyId=-1&employerSizes=0&applicationType=0&remoteWorkType=0&p={}",
                    query, loc, p
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
                // First page failed → nothing collected → record error and break so
                // the browser is still closed before we propagate.
                Err(e) if results.is_empty() => {
                    first_page_err = Some(e);
                    break 'pages;
                }
                // Later page failed → keep the pages we already have.
                Err(e) => {
                    log::warn!(
                        "[glassdoor] page {p} failed: {e}; returning {} collected",
                        results.len()
                    );
                    break;
                }
            };

            // Parse and extract in a scope to drop doc before await
            {
                let doc = Html::parse_document(&html);

                // Glassdoor job card selectors (compiled once at module level)
                let mut page_card_count = 0usize;

                for card in doc.select(&GD_CARD_SEL) {
                    if ctx.signal.is_cancelled() {
                        break;
                    }

                    let title = card
                        .select(&GD_TITLE_SEL)
                        .next()
                        .and_then(|e| e.text().next())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    let company = card
                        .select(&GD_COMPANY_SEL)
                        .next()
                        .and_then(|e| e.text().next())
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    let loc = card
                        .select(&GD_LOCATION_SEL)
                        .next()
                        .and_then(|e| e.text().next())
                        .map(|s| s.trim().to_string());

                    let url = card
                        .select(&GD_TITLE_SEL)
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

                    // Stable external id: prefer jobListingId from the URL; if
                    // absent, fall back to a hash of the job URL so cards don't
                    // collide on a bare "glassdoor-" id. Skip if we still can't
                    // derive anything stable.
                    let external_id = url
                        .split("jobListingId=")
                        .nth(1)
                        .and_then(|s| s.split('&').next())
                        .map(|s| s.to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| {
                            use std::hash::{Hash, Hasher};
                            let mut h = std::collections::hash_map::DefaultHasher::new();
                            url.hash(&mut h);
                            format!("u{:x}", h.finish())
                        });
                    if external_id.is_empty() {
                        continue;
                    }

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
                    page_card_count += 1;
                }

                if page_card_count == 0 {
                    let lower = html.to_lowercase();
                    let walled = lower.contains("captcha")
                        || lower.contains("verify you are human")
                        || lower.contains("sign in")
                        || lower.contains("log in to continue");
                    if walled {
                        log::warn!(
                            "[glassdoor] page {p} returned no cards behind a sign-in/captcha wall;                              this board is best-effort-anonymous and is likely bot-blocked"
                        );
                    } else {
                        log::warn!(
                            "[glassdoor] page {p} parsed zero job cards (selectors may have changed)"
                        );
                    }
                    break;
                }
            } // doc is dropped here

            if let Some(ref cb) = ctx.on_progress {
                cb(p as f32 / max_pages as f32);
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        browser.close().await?;
        let _ = handle.await;

        if let Some(e) = first_page_err {
            return Err(e);
        }

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
