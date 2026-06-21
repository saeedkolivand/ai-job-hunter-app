/// Glassdoor — browser-based scraper (requires JavaScript rendering).
///
/// Requires login: Glassdoor bot-blocks anonymous sessions aggressively with
/// sign-in walls and CAPTCHAs. The scraper still runs best-effort even when
/// authenticated, as Glassdoor may restrict scraping regardless.
/// The engine short-circuits this board when no valid session exists.
use super::super::types::{
    AuthRequirement, BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode,
};
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

    fn auth(&self) -> AuthRequirement {
        AuthRequirement::Required
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let max_pages = input.pages.clamp(1, 10);
        let query = urlencoding::encode(&input.query);
        let loc = urlencoding::encode(input.location.as_deref().unwrap_or(""));

        // ponytail: use the persisted per-board profile so login cookies from
        // open_login are carried into the scrape. Best-effort — Cloudflare
        // fingerprinting may still block a headless session even when
        // authenticated; CDP stealth / TLS-impersonation is deliberately out of
        // scope. The profile is shared with the login window; they don't run
        // concurrently in normal use and browser scrapes are serialized by
        // browser_sem, so no profile-lock contention in practice.
        let data_dir = crate::platform::config::data_dir();
        let profile = crate::scraping::board_login::profile_dir(&data_dir, "glassdoor");
        tokio::fs::create_dir_all(&profile).await.ok();

        let mut builder = BrowserConfig::builder()
            .window_size(1920, 1080)
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--no-default-browser-check")
            .arg("--no-first-run");

        if let Some(chrome_path) = crate::platform::detect_system_chrome() {
            builder = builder.chrome_executable(chrome_path);
        }

        let (mut browser, mut handler) = Browser::launch(
            builder
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
                            "[glassdoor] page {p} returned no cards behind a sign-in/captcha wall; \
                             login session may have expired or Glassdoor is blocking the headless browser"
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

        let close_res = browser.close().await;
        let _ = handle.await;
        // The first-page navigation error is the root cause — always prefer it
        // over a teardown failure so callers see the real error, not a
        // secondary browser-close error that obscures what actually went wrong.
        if let Some(e) = first_page_err {
            return Err(e);
        }
        close_res?;

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
