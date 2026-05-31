/// Personio — public XML feed per company
use super::super::http::{fetch_text, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;

const HOSTS: &[&str] = &["jobs.personio.de", "jobs.personio.com"];

pub struct PersonioScraper;

#[async_trait]
impl Scraper for PersonioScraper {
    fn id(&self) -> &'static str {
        "personio"
    }

    fn display_name(&self) -> &'static str {
        "Personio"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let company = input.query.trim().to_lowercase();
        if company.is_empty() {
            return Ok(vec![]);
        }

        let mut xml = None;
        for host in HOSTS {
            let url = format!("https://{}.{}", company, host);
            let res = fetch_text(&url, Default::default(), ctx.signal.clone()).await?;

            if res.status_code == 200 && res.text.contains("<position") {
                xml = Some(res.text);
                break;
            }
        }

        let xml = match xml {
            Some(x) => x,
            None => return Ok(vec![]),
        };

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        // Parse XML manually since feed-rs may not handle this format
        let position_re = regex::Regex::new(r"<position>(.*?)</position>").unwrap();
        let id_re = regex::Regex::new(r"<id>(.*?)</id>").unwrap();
        let name_re = regex::Regex::new(r"<name>(.*?)</name>").unwrap();
        let office_re = regex::Regex::new(r"<office>(.*?)</office>").unwrap();
        let desc_re =
            regex::Regex::new(r"<jobDescription>\s*<value>(.*?)</value>\s*</jobDescription>")
                .unwrap();
        let created_re = regex::Regex::new(r"<createdAt>(.*?)</createdAt>").unwrap();

        for position_cap in position_re.captures_iter(&xml) {
            if let Some(position_content) = position_cap.get(1) {
                let position_str = position_content.as_str();

                let id = id_re
                    .captures(position_str)
                    .and_then(|c| c.get(1).map(|m| m.as_str().trim()))
                    .unwrap_or("");

                if id.is_empty() {
                    continue;
                }

                let title = name_re
                    .captures(position_str)
                    .and_then(|c| c.get(1).map(|m| m.as_str().trim()))
                    .unwrap_or("");

                let office = office_re
                    .captures(position_str)
                    .and_then(|c| c.get(1).map(|m| m.as_str().trim()))
                    .unwrap_or("");

                let desc = desc_re
                    .captures(position_str)
                    .and_then(|c| c.get(1).map(|m| strip_html(m.as_str().trim())))
                    .unwrap_or_default();

                let created = created_re
                    .captures(position_str)
                    .and_then(|c| c.get(1).map(|m| m.as_str().trim()))
                    .unwrap_or("");

                let posted_at = if !created.is_empty() {
                    chrono::DateTime::parse_from_rfc3339(created)
                        .ok()
                        .map(|dt| dt.timestamp_millis())
                } else {
                    None
                };

                let posting = JobPosting {
                    id: format!("{}:{}", self.id(), id),
                    external_id: Some(id.to_string()),
                    title: title.to_string(),
                    company: company.clone(),
                    location: if office.is_empty() {
                        None
                    } else {
                        Some(office.to_string())
                    },
                    url: format!("https://{}.{}/job/{}", company, HOSTS[0], id),
                    source: self.id().to_string(),
                    description: if desc.is_empty() { None } else { Some(desc) },
                    requirements: None,
                    posted_at,
                    captured_at: now,
                    extra: std::collections::HashMap::new(),
                };

                if let Some(ref on_item) = ctx.on_item {
                    on_item(posting.clone());
                }

                out.push(posting);
            }
        }

        if let Some(ref on_progress) = ctx.on_progress {
            on_progress(1.0);
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;
