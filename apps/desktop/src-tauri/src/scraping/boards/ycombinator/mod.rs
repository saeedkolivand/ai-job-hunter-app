/// Y Combinator Jobs — HN Firebase API (replaces Algolia whose public client keys rotated)
///
/// The workatastartup.com Algolia credentials are baked into the JS bundle and rotate.
/// The Hacker News Firebase API is the canonical public YC job feed:
///   /v0/jobstories.json  → list of item IDs (most recent ~30 jobs)
///   /v0/item/{id}.json   → individual job: type="job", title, url, text, by, time
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;
use serde::Deserialize;

const HN_BASE: &str = "https://hacker-news.firebaseio.com/v0";

#[derive(Debug, Deserialize)]
struct HnItem {
    id: i64,
    #[serde(rename = "type")]
    item_type: Option<String>,
    title: Option<String>,
    url: Option<String>,
    text: Option<String>,
    by: Option<String>,
    time: Option<i64>,
}

/// Derive the company name from an HN job title.
///
/// Titles look like `"Company (YC S24) Is Hiring …"`. If the ` (YC ` marker is
/// found, everything before it is the company name — but if that prefix is empty
/// (the marker appears at the very start), fall back to `by` instead.
/// When the marker is absent, return `by` directly.
pub(crate) fn parse_company(title: &str, by: &str) -> String {
    if let Some(pos) = title.find(" (YC ") {
        let prefix = title[..pos].trim();
        if prefix.is_empty() {
            by.to_string()
        } else {
            prefix.to_string()
        }
    } else {
        by.to_string()
    }
}

pub struct YCombinatorScraper;

#[async_trait]
impl Scraper for YCombinatorScraper {
    fn id(&self) -> &'static str {
        "ycombinator"
    }

    fn display_name(&self) -> &'static str {
        "Y Combinator"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let q = input.query.trim().to_lowercase();

        // Step 1: fetch the list of job story IDs
        let ids = fetch_json::<Vec<i64>>(
            &format!("{}/jobstories.json", HN_BASE),
            Default::default(),
            ctx.signal.clone(),
        )
        .await?
        .unwrap_or_default();

        if ids.is_empty() {
            return Ok(vec![]);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let limit = input.amount.clamp(1, 50) as usize;
        let mut out = vec![];

        // Step 2: fetch each item; stop early once we hit the limit
        for id in ids.iter().take(limit * 3) {
            // over-fetch to account for filter misses
            if ctx.signal.is_cancelled() {
                break;
            }
            if out.len() >= limit {
                break;
            }

            let item = match fetch_json::<HnItem>(
                &format!("{}/item/{}.json", HN_BASE, id),
                Default::default(),
                ctx.signal.clone(),
            )
            .await
            {
                Ok(Some(i)) => i,
                Ok(None) => continue,
                Err(e) => {
                    log::warn!("[ycombinator] item {id} failed: {e}; skipping");
                    continue;
                }
            };

            // Only include items of type "job"
            if item.item_type.as_deref() != Some("job") {
                continue;
            }

            let title = item.title.unwrap_or_default();
            if title.is_empty() {
                continue;
            }

            // Keyword filter
            let description_text = item.text.as_deref().unwrap_or("");
            let haystack = format!("{} {}", title, description_text).to_lowercase();
            if !q.is_empty() && !haystack.contains(&q) {
                continue;
            }

            // HN job titles often look like "Company (YC S24) Is Hiring …"
            // Only match the full " (YC " prefix — bare " (W" / " (S" are too loose
            // and truncate ordinary English parentheticals (e.g. "Engineer (Senior) at …").
            let by = item.by.clone().unwrap_or_else(|| "Unknown".to_string());
            let company = parse_company(&title, &by);

            let url = item
                .url
                .clone()
                .unwrap_or_else(|| format!("https://news.ycombinator.com/item?id={}", item.id));

            let description = item.text.as_deref().map(strip_html);

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), item.id),
                external_id: Some(item.id.to_string()),
                title,
                company,
                location: None,
                url,
                source: self.id().to_string(),
                description,
                requirements: None,
                posted_at: item.time.map(|t| t * 1000),
                captured_at: now,
                extra: std::collections::HashMap::new(),
            };

            if let Some(ref on_item) = ctx.on_item {
                on_item(posting.clone());
            }

            out.push(posting);
        }

        if let Some(ref on_progress) = ctx.on_progress {
            on_progress(1.0);
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;
