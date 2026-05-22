/// We Work Remotely — public RSS feed
use super::super::http::{fetch_text, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;

pub struct WeWorkRemotelyScraper;

#[async_trait]
impl Scraper for WeWorkRemotelyScraper {
    fn id(&self) -> &'static str {
        "wwr"
    }

    fn display_name(&self) -> &'static str {
        "We Work Remotely"
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
        let res = fetch_text("https://weworkremotely.com/remote-jobs.rss", Default::default(), ctx.signal).await?;

        if res.status_code != 200 {
            return Ok(vec![]);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        // Parse RSS XML
        let channel = feed_rs::parser::parse(res.text.as_bytes()).map_err(|e| anyhow::anyhow!("Failed to parse RSS: {}", e))?;

        for entry in channel.entries {
            let title = entry.title.map(|t| t.content).unwrap_or_default();
            let link = entry.links.first().map(|l| l.href.clone()).unwrap_or_default();
            let guid = if entry.id.is_empty() { link.clone() } else { entry.id.clone() };
            let description = entry.content.as_ref()
                .and_then(|c| c.body.as_ref())
                .map(|b| strip_html(b))
                .or_else(|| {
                    entry.summary.as_ref()
                        .map(|s| strip_html(&s.content))
                });
            let pub_date = entry.published.map(|dt| dt.timestamp_millis());

            // WWR titles often look like "Company: Senior Engineer"
            let split: Vec<&str> = title.splitn(2, ": ").collect();
            let (company, clean_title) = if split.len() > 1 {
                (split[0].trim(), split[1].trim())
            } else {
                ("Unknown", title.as_str())
            };

            if !q.is_empty() && !title.to_lowercase().contains(&q) {
                continue;
            }

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), guid),
                external_id: Some(guid),
                title: clean_title.to_string(),
                company: company.to_string(),
                location: None,
                url: link,
                source: self.id().to_string(),
                description,
                requirements: None,
                posted_at: pub_date,
                captured_at: now,
                extra: {
                    let mut map = std::collections::HashMap::new();
                    map.insert("remote".to_string(), serde_json::json!(true));
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
