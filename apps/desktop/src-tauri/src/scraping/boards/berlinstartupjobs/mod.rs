/// Berlin Startup Jobs — public WordPress RSS feed
use super::super::http::{fetch_text, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;

/// `"Job Title at Company"` splitter — compiled once and reused (hoisted out of the
/// per-entry loop so the regex is not recompiled on every feed item).
static AT_COMPANY_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r" at (.+)$").unwrap());

pub struct BerlinStartupJobsScraper;

#[async_trait]
impl Scraper for BerlinStartupJobsScraper {
    fn id(&self) -> &'static str {
        "berlinstartupjobs"
    }

    fn display_name(&self) -> &'static str {
        "Berlin Startup Jobs"
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
        let res = fetch_text(
            "https://berlinstartupjobs.com/feed/",
            Default::default(),
            ctx.signal,
        )
        .await?;

        if res.status_code != 200 {
            // Representable failure: a non-2xx feed response is a failed board, not
            // a silent zero — surface the status into `BoardScrapeSummary.error`.
            return Err(anyhow::anyhow!("HTTP {}", res.status_code));
        }

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        // Parse RSS XML
        let channel = feed_rs::parser::parse(res.text.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to parse RSS: {}", e))?;

        for entry in channel.entries {
            let title = entry.title.map(|t| t.content).unwrap_or_default();
            let link = entry
                .links
                .first()
                .map(|l| l.href.clone())
                .unwrap_or_default();
            let guid = if entry.id.is_empty() {
                link.clone()
            } else {
                entry.id.clone()
            };
            let description = entry
                .content
                .as_ref()
                .and_then(|c| c.body.as_ref())
                .map(|b| strip_html(b))
                .or_else(|| entry.summary.as_ref().map(|s| strip_html(&s.content)));
            let pub_date = entry.published.map(|dt| dt.timestamp_millis());

            let categories: Vec<String> = entry.categories.iter().map(|c| c.term.clone()).collect();

            // Split "Job Title at Company" → { title, company }
            let (clean_title, company) = if let Some(m) = AT_COMPANY_RE.captures(&title) {
                let company = m.get(1).map(|m| m.as_str().trim()).unwrap_or("Unknown");
                let clean_title = title.split(" at ").next().unwrap_or(&title).trim();
                (clean_title.to_string(), company.to_string())
            } else {
                (title.clone(), "Unknown".to_string())
            };

            let haystack = format!("{} {}", title, categories.join(" ")).to_lowercase();
            if !q.is_empty() && !haystack.contains(&q) {
                continue;
            }

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), guid),
                external_id: Some(guid),
                title: clean_title,
                company,
                location: Some("Berlin".to_string()),
                url: link,
                source: self.id().to_string(),
                description,
                requirements: if categories.is_empty() {
                    None
                } else {
                    Some(categories)
                },
                posted_at: pub_date,
                captured_at: now,
                extra: {
                    let mut map = std::collections::HashMap::new();
                    map.insert("language".to_string(), serde_json::json!("en"));
                    map
                },
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
