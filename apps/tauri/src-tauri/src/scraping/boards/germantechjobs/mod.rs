/// GermanTechJobs — public RSS feed (migrated from Next.js __NEXT_DATA__ in 2025)
use super::super::http::{fetch_text, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;

// Matches a trailing salary bracket like " [46.000 - 86.000 €]" or "[100k]".
// Using a regex so we only strip when an actual bracket is present — a char-set
// trim would incorrectly eat suffixes like "L3", "II", or "- Berlin".
static SALARY_BRACKET_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"\s*\[\d[^\]]*\]$").unwrap());

pub struct GermanTechJobsScraper;

#[async_trait]
impl Scraper for GermanTechJobsScraper {
    fn id(&self) -> &'static str {
        "germantechjobs"
    }

    fn display_name(&self) -> &'static str {
        "German Tech Jobs"
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
        let loc = input
            .location
            .as_ref()
            .map(|l| l.trim().to_lowercase())
            .unwrap_or_default();

        let res = fetch_text(
            "https://germantechjobs.de/rss",
            super::super::http::FetchOptions {
                headers: Some(vec![(
                    "accept-language".to_string(),
                    "en-US,en;q=0.9".to_string(),
                )]),
                // GTJ RSS feed can reach ~10 MB; raise the per-request cap only here.
                max_bytes: Some(16 * 1024 * 1024),
                ..Default::default()
            },
            ctx.signal,
        )
        .await?;

        if res.status_code != 200 {
            log::warn!("[germantechjobs] RSS fetch returned {}", res.status_code);
            return Ok(vec![]);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

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

            // GTJ titles: "Job Title @ Company [salary range]"
            // Split on " @ " to extract company; strip trailing salary bracket from title.
            let (clean_title, company) = if let Some(at_pos) = title.find(" @ ") {
                let job_part = title[..at_pos].trim();
                let company_part = title[at_pos + 3..].trim();
                (job_part.to_string(), company_part.to_string())
            } else {
                (title.clone(), "Unknown".to_string())
            };

            // Strip trailing salary bracket like " [46.000 - 86.000 €]" from the job part.
            // Regex matches only when a bracket is actually present, so "L3" / "II" / "- Berlin"
            // are not eaten by a greedy char-set trim.
            let clean_title = SALARY_BRACKET_RE
                .replace(&clean_title, "")
                .trim()
                .to_string();
            let clean_title = if clean_title.is_empty() {
                title.clone()
            } else {
                clean_title
            };

            let description = entry
                .content
                .as_ref()
                .and_then(|c| c.body.as_ref())
                .map(|b| strip_html(b))
                .or_else(|| entry.summary.as_ref().map(|s| strip_html(&s.content)));

            let pub_date = entry.published.map(|dt| dt.timestamp_millis());

            // Client-side keyword filter — include description so skill-based queries
            // (e.g. "rust") match jobs whose description mentions the keyword.
            let haystack = format!(
                "{} {} {}",
                title,
                company,
                description.as_deref().unwrap_or("")
            )
            .to_lowercase();
            if !q.is_empty() && !haystack.contains(&q) {
                continue;
            }

            // Location filter — GTJ is Germany-focused; location not in RSS so filter on loc
            // against `haystack` (which already includes the description).
            if !loc.is_empty() && !haystack.contains(&loc) {
                continue;
            }

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), guid),
                external_id: Some(guid),
                title: clean_title,
                company,
                location: None, // not in RSS feed
                url: link,
                source: self.id().to_string(),
                description,
                requirements: None,
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
