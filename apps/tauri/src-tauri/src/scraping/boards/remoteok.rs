/// RemoteOK — public JSON feed
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RemoteOkItem {
    #[serde(rename_all = "camelCase")]
    Job {
        id: Option<serde_json::Value>,
        slug: Option<String>,
        position: Option<String>,
        company: Option<String>,
        location: Option<String>,
        tags: Option<Vec<String>>,
        description: Option<String>,
        url: Option<String>,
        #[serde(rename = "apply_url")]
        apply_url: Option<String>,
        date: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Legend {
        #[serde(rename = "slug")]
        _slug: String,
    },
}

pub struct RemoteOkScraper;

#[async_trait]
impl Scraper for RemoteOkScraper {
    fn id(&self) -> &'static str {
        "remoteok"
    }

    fn display_name(&self) -> &'static str {
        "RemoteOK"
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
        let items = fetch_json::<Vec<RemoteOkItem>>("https://remoteok.com/api", Default::default(), ctx.signal).await?;

        let items = match items {
            Some(i) => i,
            None => return Ok(vec![]),
        };

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        for it in items {
            let (id, slug, position, company, location, tags, description, url, apply_url, date) = match it {
                RemoteOkItem::Job { id, slug, position, company, location, tags, description, url, apply_url, date } => {
                    (id, slug, position, company, location, tags, description, url, apply_url, date)
                }
                RemoteOkItem::Legend { .. } => continue, // skip legend entry
            };

            let id_str = id.map(|i| i.to_string()).unwrap_or_else(|| "".to_string());
            if id_str.is_empty() || position.is_none() {
                continue;
            }

            let haystack = format!(
                "{} {} {}",
                position.as_ref().unwrap_or(&"".to_string()),
                company.as_ref().unwrap_or(&"".to_string()),
                tags.as_ref().map(|t| t.join(" ")).unwrap_or_else(|| "".to_string())
            ).to_lowercase();

            if !q.is_empty() && !haystack.contains(&q) {
                continue;
            }

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), id_str),
                external_id: Some(id_str.clone()),
                title: position.unwrap_or_default(),
                company: company.unwrap_or_else(|| "Unknown".to_string()),
                location,
                url: url.or(apply_url).unwrap_or_else(|| {
                    format!("https://remoteok.com/remote-jobs/{}", slug.unwrap_or_else(|| id_str.clone()))
                }),
                source: self.id().to_string(),
                description: description.map(|d| strip_html(&d)),
                requirements: tags,
                posted_at: date.and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok()).map(|dt| dt.timestamp_millis()),
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
