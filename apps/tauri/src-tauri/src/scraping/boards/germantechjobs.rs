/// GermanTechJobs — Next.js powered board for English-speaking tech roles in DE
use super::super::http::{fetch_text, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct NextJob {
    #[serde(rename = "_id")]
    _id: Option<String>,
    id: Option<String>,
    slug: Option<String>,
    title: Option<String>,
    #[serde(rename = "companyName")]
    company_name: Option<String>,
    description: Option<String>,
    location: Option<serde_json::Value>,
    remote: Option<bool>,
    tags: Option<Vec<String>>,
    skills: Option<Vec<String>>,
    #[serde(rename = "createdAt")]
    created_at: Option<String>,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PageProps {
    jobs: Option<Vec<NextJob>>,
    #[serde(rename = "jobsList")]
    jobs_list: Option<Vec<NextJob>>,
}

#[derive(Debug, Deserialize)]
struct Props {
    #[serde(rename = "pageProps")]
    page_props: Option<PageProps>,
}

#[derive(Debug, Deserialize)]
struct NextData {
    props: Option<Props>,
}

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
        let loc = input.location.as_ref().map(|l| l.trim().to_lowercase()).unwrap_or_default();
        
        let res = fetch_text(
            "https://germantechjobs.de/",
            super::super::http::FetchOptions {
                headers: Some(vec![("accept-language".to_string(), "en-US,en;q=0.9".to_string())]),
                ..Default::default()
            },
            ctx.signal,
        ).await?;

        if res.status_code != 200 {
            return Ok(vec![]);
        }

        // Extract __NEXT_DATA__ JSON from HTML
        let re = regex::Regex::new(r#"<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).unwrap();
        let raw = match re.captures(&res.text) {
            Some(c) => c.get(1).map(|m| m.as_str()).unwrap_or(""),
            None => return Ok(vec![]),
        };

        if raw.is_empty() {
            return Ok(vec![]);
        }

        let data: NextData = serde_json::from_str(raw).map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?;
        let jobs = data.props
            .and_then(|p| p.page_props)
            .and_then(|pp| pp.jobs.or(pp.jobs_list))
            .unwrap_or_default();

        if jobs.is_empty() {
            return Ok(vec![]);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];

        for j in jobs {
            let external_id = j._id.as_ref().or(j.id.as_ref()).or(j.slug.as_ref()).map(|s| s.as_str()).unwrap_or("");
            if external_id.is_empty() {
                continue;
            }

            let skills = j.tags.as_ref().or(j.skills.as_ref()).cloned().unwrap_or_default();
            let location = match j.location {
                Some(serde_json::Value::Array(arr)) => {
                    arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")
                }
                Some(serde_json::Value::String(s)) => s,
                _ => "".to_string(),
            };

            let haystack = format!(
                "{} {} {}",
                j.title.as_deref().unwrap_or(""),
                j.company_name.as_deref().unwrap_or(""),
                skills.join(" ")
            ).to_lowercase();

            if !q.is_empty() && !haystack.contains(&q) {
                continue;
            }

            if !loc.is_empty() && !location.to_lowercase().contains(&loc) {
                continue;
            }

            let url = j.url.or_else(|| {
                j.slug.as_ref().map(|slug| {
                    format!("https://germantechjobs.de/job/{}", slug)
                })
            }).unwrap_or_else(|| {
                format!("https://germantechjobs.de/job/{}", external_id)
            });

            let posted_at = j.published_at
                .as_ref()
                .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                .map(|dt| dt.timestamp_millis())
                .or_else(|| {
                    j.created_at.as_ref()
                        .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                        .map(|dt| dt.timestamp_millis())
                });

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), external_id),
                external_id: Some(external_id.to_string()),
                title: j.title.unwrap_or_default().trim().to_string(),
                company: j.company_name.unwrap_or_else(|| "Unknown".to_string()).trim().to_string(),
                location: if location.is_empty() { None } else { Some(location) },
                url,
                source: self.id().to_string(),
                description: j.description.map(|d| strip_html(&d)),
                requirements: if skills.is_empty() { None } else { Some(skills) },
                posted_at,
                captured_at: now,
                extra: {
                    let mut map = std::collections::HashMap::new();
                    map.insert("language".to_string(), serde_json::json!("en"));
                    if let Some(remote) = j.remote {
                        map.insert("remote".to_string(), serde_json::json!(remote));
                    }
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
mod tests {
    use super::*;

    #[test]
    fn test_german_tech_jobs_scraper_id() {
        let scraper = GermanTechJobsScraper;
        assert_eq!(scraper.id(), "germantechjobs");
    }

    #[test]
    fn test_german_tech_jobs_scraper_display_name() {
        let scraper = GermanTechJobsScraper;
        assert_eq!(scraper.display_name(), "German Tech Jobs");
    }

    #[test]
    fn test_german_tech_jobs_scraper_mode() {
        let scraper = GermanTechJobsScraper;
        assert_eq!(scraper.mode(), ScraperMode::Http);
    }

    #[test]
    fn test_german_tech_jobs_scraper_mode_partial_eq() {
        let mode = ScraperMode::Http;
        assert_eq!(mode, ScraperMode::Http);
        assert_ne!(mode, ScraperMode::Browser);
    }

    #[test]
    fn test_next_job_struct_fields() {
        let job = NextJob {
            _id: Some("123".to_string()),
            id: None,
            slug: None,
            title: Some("Software Engineer".to_string()),
            company_name: Some("Test Corp".to_string()),
            description: None,
            location: None,
            remote: None,
            tags: None,
            skills: None,
            created_at: None,
            published_at: None,
            url: None,
        };
        assert_eq!(job._id, Some("123".to_string()));
        assert_eq!(job.title, Some("Software Engineer".to_string()));
    }

    #[test]
    fn test_next_job_struct_defaults() {
        let job = NextJob {
            _id: None,
            id: None,
            slug: None,
            title: None,
            company_name: None,
            description: None,
            location: None,
            remote: None,
            tags: None,
            skills: None,
            created_at: None,
            published_at: None,
            url: None,
        };
        assert!(job.title.is_none());
        assert!(job.remote.is_none());
    }

    #[test]
    fn test_next_data_regex() {
        let re = regex::Regex::new(r#"<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).unwrap();
        let html = r#"<script id="__NEXT_DATA__" type="application/json">{"test": true}</script>"#;
        let caps = re.captures(html);
        assert!(caps.is_some());
    }

    #[test]
    fn test_next_data_regex_no_match() {
        let re = regex::Regex::new(r#"<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).unwrap();
        let html = "<div>No script here</div>";
        let caps = re.captures(html);
        assert!(caps.is_none());
    }
}
