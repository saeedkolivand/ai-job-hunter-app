/// Indeed — authenticated HTML scraping.
///
/// Requires a prior login via `board_login::open_login("indeed", …)` so that
/// cookies are present in `<data_dir>/browser-state/indeed/cookies.json`.
use crate::scraping::board_login;
use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;
use scraper::{Html, Selector};
use std::collections::HashSet;

/// Indeed has one locale per TLD. Default to .com.
const INDEED_DOMAINS: &[(&str, &str)] = &[
    ("us", "www.indeed.com"),
    ("de", "de.indeed.com"),
    ("uk", "uk.indeed.com"),
    ("fr", "fr.indeed.com"),
    ("at", "at.indeed.com"),
    ("ch", "ch.indeed.com"),
    ("au", "au.indeed.com"),
    ("ca", "ca.indeed.com"),
    ("nl", "nl.indeed.com"),
    ("be", "be.indeed.com"),
    ("es", "es.indeed.com"),
    ("it", "it.indeed.com"),
    ("pl", "pl.indeed.com"),
    ("br", "br.indeed.com"),
    ("in", "in.indeed.com"),
    ("sg", "sg.indeed.com"),
    ("jp", "jp.indeed.com"),
];

pub struct IndeedScraper;

#[async_trait]
impl Scraper for IndeedScraper {
    fn id(&self) -> &'static str {
        "indeed"
    }

    fn display_name(&self) -> &'static str {
        "Indeed"
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
        let client = board_login::build_authed_client(&data_dir, "indeed")?;

        let domain = INDEED_DOMAINS
            .iter()
            .find(|(locale, _)| *locale == input.locale.as_deref().unwrap_or("us"))
            .map(|(_, d)| *d)
            .unwrap_or("www.indeed.com");

        let max_pages = input.pages.clamp(1, 5) as usize;
        let mut out = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        for page in 0..max_pages {
            if ctx.signal.is_cancelled() {
                break;
            }
            let url = format!(
                "https://{domain}/jobs?q={}&l={}&start={}",
                urlencoding::encode(input.query.trim()),
                urlencoding::encode(input.location.as_deref().unwrap_or("")),
                page * 10
            );

            let html = match async {
                let res = client.get(&url).send().await?;
                if !res.status().is_success() {
                    // Non-success status → stop paginating (not an error).
                    return anyhow::Ok(None);
                }
                // Successful authenticated response → refresh session timestamp.
                board_login::touch_session(&data_dir, "indeed");
                anyhow::Ok(Some(res.text().await?))
            }
            .await
            {
                Ok(Some(html)) => html,
                Ok(None) => break,
                // First page failed → nothing collected → propagate.
                Err(e) if out.is_empty() => return Err(e),
                // Later page failed → keep the pages we already have (and streamed).
                Err(e) => {
                    log::warn!(
                        "[indeed] page {page} failed: {e}; returning {} collected",
                        out.len()
                    );
                    break;
                }
            };

            // Parse synchronously — `scraper::Html` is !Send so it must not
            // live across an await point.
            let page_postings = parse_indeed_page(&html, domain, &mut seen);

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

            // Light pacing — Indeed throttles aggressively.
            tokio::time::sleep(std::time::Duration::from_millis(
                800 + (rand::random::<u64>() % 600),
            ))
            .await;
        }

        Ok(out)
    }
}

fn parse_indeed_page(html: &str, domain: &str, seen: &mut HashSet<String>) -> Vec<JobPosting> {
    let doc = Html::parse_document(html);
    let card_sel = Selector::parse("div.job_seen_beacon, td.resultContent").unwrap();
    let title_sel = Selector::parse("h2.jobTitle span[title], h2.jobTitle a span").unwrap();
    let link_sel = Selector::parse("h2.jobTitle a").unwrap();
    let company_sel = Selector::parse("[data-testid=\"company-name\"], span.companyName").unwrap();
    let location_sel =
        Selector::parse("[data-testid=\"text-location\"], div.companyLocation").unwrap();
    let snippet_sel = Selector::parse("div.job-snippet, div[data-testid=\"job-snippet\"]").unwrap();

    let now = chrono::Utc::now().timestamp_millis();
    let mut out = Vec::new();

    for card in doc.select(&card_sel) {
        let link_el = match card.select(&link_sel).next() {
            Some(e) => e,
            None => continue,
        };
        let href = link_el.value().attr("href").unwrap_or("");
        let jk = link_el
            .value()
            .attr("data-jk")
            .or_else(|| extract_jk_from_href(href))
            .unwrap_or("");
        if jk.is_empty() || !seen.insert(jk.to_string()) {
            continue;
        }

        let title = card
            .select(&title_sel)
            .next()
            .map(|e| {
                e.value()
                    .attr("title")
                    .map(str::to_string)
                    .unwrap_or_else(|| e.text().collect::<String>())
            })
            .unwrap_or_default()
            .trim()
            .to_string();

        let company = card
            .select(&company_sel)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let location = card
            .select(&location_sel)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string());

        let description = card
            .select(&snippet_sel)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string());

        let url = if href.starts_with("http") {
            href.to_string()
        } else {
            format!("https://{domain}{href}")
        };

        out.push(JobPosting {
            id: format!("indeed:{jk}"),
            external_id: Some(jk.to_string()),
            title,
            company,
            location,
            url,
            source: "indeed".to_string(),
            description,
            requirements: None,
            posted_at: None,
            captured_at: now,
            extra: std::collections::HashMap::new(),
        });
    }
    out
}

fn extract_jk_from_href(href: &str) -> Option<&str> {
    // /viewjob?jk=abc123&from=…  or  /rc/clk?jk=abc123…
    let jk_start = href.find("jk=")?;
    let after = &href[jk_start + 3..];
    let end = after.find('&').unwrap_or(after.len());
    Some(&after[..end])
}

#[cfg(test)]
mod test;
