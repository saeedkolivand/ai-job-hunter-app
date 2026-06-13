/// Xing — authenticated HTML scraping.
///
/// Requires a prior login via `board_login::open_login("xing", …)` so that
/// cookies are present in `<data_dir>/browser-state/xing/cookies.json`.
use crate::scraping::board_login;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;
use scraper::{Html, Selector};
use std::collections::HashSet;

// Xing job-card CSS selectors compiled once (Selector is Send + Sync).
static XING_CARD_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
    Selector::parse("article[data-testid=\"job-search-result\"], article.job-teaser").unwrap()
});
static XING_LINK_SEL: std::sync::LazyLock<Selector> =
    std::sync::LazyLock::new(|| Selector::parse("a[href*=\"/jobs/\"]").unwrap());
static XING_TITLE_SEL: std::sync::LazyLock<Selector> =
    std::sync::LazyLock::new(|| Selector::parse("h2, [data-testid=\"job-title\"]").unwrap());
static XING_COMPANY_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
    Selector::parse("[data-testid=\"job-company-name\"], .companyName, p.company").unwrap()
});
static XING_LOCATION_SEL: std::sync::LazyLock<Selector> = std::sync::LazyLock::new(|| {
    Selector::parse("[data-testid=\"job-location\"], .location, p.location").unwrap()
});

pub struct XingScraper;

#[async_trait]
impl Scraper for XingScraper {
    fn id(&self) -> &'static str {
        "xing"
    }

    fn display_name(&self) -> &'static str {
        "Xing"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Browser
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let data_dir = crate::platform::config::data_dir();
        let client = board_login::build_authed_client(&data_dir, "xing")?;

        let max_pages = input.pages.clamp(1, 5) as usize;
        let mut out = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        for page in 0..max_pages {
            if ctx.signal.is_cancelled() {
                break;
            }
            let url = format!(
                "https://www.xing.com/jobs/search?keywords={}&location={}&page={}",
                urlencoding::encode(input.query.trim()),
                urlencoding::encode(input.location.as_deref().unwrap_or("")),
                page + 1
            );

            let html = match async {
                let res = client.get(&url).send().await?;
                if !res.status().is_success() {
                    // Non-success status → stop paginating (not an error).
                    return anyhow::Ok(None);
                }
                board_login::touch_session(&data_dir, "xing");
                anyhow::Ok(Some(res.text().await?))
            }
            .await
            {
                Ok(Some(html)) => html,
                Ok(None) => break,
                Err(e) if out.is_empty() => return Err(e),
                Err(e) => {
                    log::warn!(
                        "[xing] page {page} failed: {e}; returning {} collected",
                        out.len()
                    );
                    break;
                }
            };

            // Sync parse — `scraper::Html` is !Send.
            let page_postings = parse_xing_page(&html, &mut seen);

            let page_count = page_postings.len();
            for posting in &page_postings {
                if let Some(ref on_item) = ctx.on_item {
                    on_item(posting.clone());
                }
            }
            out.extend(page_postings);

            if let Some(ref on_progress) = ctx.on_progress {
                on_progress((page + 1) as f32 / max_pages as f32);
            }

            if page_count == 0 {
                break;
            }

            tokio::time::sleep(std::time::Duration::from_millis(
                700 + (rand::random::<u64>() % 500),
            ))
            .await;
        }

        Ok(out)
    }
}

fn parse_xing_page(html: &str, seen: &mut HashSet<String>) -> Vec<JobPosting> {
    let doc = Html::parse_document(html);
    // Card/field selectors are compiled once at module level.

    let now = chrono::Utc::now().timestamp_millis();
    let mut out = Vec::new();

    for card in doc.select(&XING_CARD_SEL) {
        let link_el = match card.select(&XING_LINK_SEL).next() {
            Some(e) => e,
            None => continue,
        };
        let href = link_el.value().attr("href").unwrap_or("");
        let id = extract_id_from_href(href);
        if id.is_empty() || !seen.insert(id.clone()) {
            continue;
        }

        let title = card
            .select(&XING_TITLE_SEL)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let company = card
            .select(&XING_COMPANY_SEL)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let location = card
            .select(&XING_LOCATION_SEL)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string());

        let abs_url = if href.starts_with("http") {
            href.to_string()
        } else {
            format!("https://www.xing.com{href}")
        };

        out.push(JobPosting {
            id: format!("xing:{id}"),
            external_id: Some(id),
            title,
            company,
            location,
            url: abs_url,
            source: "xing".to_string(),
            description: None,
            requirements: None,
            posted_at: None,
            captured_at: now,
            extra: std::collections::HashMap::new(),
        });
    }
    out
}

/// Extract a stable job id from a Xing job URL like
/// `/jobs/software-engineer-abc123` → `abc123` (or the slug as fallback).
fn extract_id_from_href(href: &str) -> String {
    let path = href.split('?').next().unwrap_or(href);
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod test;
