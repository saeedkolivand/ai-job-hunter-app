#![allow(dead_code)]

/// SmartRecruiters — public per-company postings API
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Location {
    city: Option<String>,
    country: Option<String>,
    remote: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Posting {
    id: String,
    uuid: Option<String>,
    name: String,
    location: Option<Location>,
    #[serde(rename = "releasedDate")]
    released_date: Option<String>,
    #[serde(rename = "ref")]
    ref_field: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListResp {
    content: Vec<Posting>,
}

#[derive(Debug, Deserialize)]
struct Section {
    title: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JobAd {
    sections: Option<std::collections::HashMap<String, Section>>,
}

#[derive(Debug, Deserialize)]
struct DetailResp {
    #[serde(rename = "jobAd")]
    job_ad: Option<JobAd>,
    #[serde(rename = "ref")]
    ref_field: Option<String>,
}

pub struct SmartRecruitersScraper;

#[async_trait]
impl Scraper for SmartRecruitersScraper {
    fn id(&self) -> &'static str {
        "smartrecruiters"
    }

    fn display_name(&self) -> &'static str {
        "SmartRecruiters"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let company = input.query.trim();
        if company.is_empty() {
            return Ok(vec![]);
        }

        let list_url = format!(
            "https://api.smartrecruiters.com/v1/companies/{}/postings?limit=100",
            urlencoding::encode(company)
        );

        let list = fetch_json::<ListResp>(&list_url, Default::default(), ctx.signal.clone()).await?;

        let postings = match list {
            Some(l) => l.content,
            None => return Ok(vec![]),
        };

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];
        let total = postings.len();

        for (i, p) in postings.into_iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            let detail_url = format!(
                "https://api.smartrecruiters.com/v1/companies/{}/postings/{}",
                urlencoding::encode(company),
                p.id
            );

            let detail = fetch_json::<DetailResp>(&detail_url, Default::default(), ctx.signal.clone()).await?;

            let sections = detail
                .and_then(|d| d.job_ad)
                .and_then(|ja| ja.sections)
                .unwrap_or_default();

            let description = sections
                .values()
                .map(|s| {
                    format!(
                        "{}\n{}",
                        s.title.as_deref().unwrap_or(""),
                        strip_html(s.text.as_deref().unwrap_or(""))
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n")
                .trim()
                .to_string();

            let location = vec![
                p.location.as_ref().and_then(|l| l.city.clone()),
                p.location.as_ref().and_then(|l| l.country.clone()),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(", ");

            let posted_at = p.released_date.and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok()).map(|dt| dt.timestamp_millis());

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), p.id),
                external_id: Some(p.id.clone()),
                title: p.name,
                company: company.to_string(),
                location: if location.is_empty() { None } else { Some(location) },
                url: format!(
                    "https://jobs.smartrecruiters.com/{}/{}",
                    urlencoding::encode(company),
                    p.id
                ),
                source: self.id().to_string(),
                description: if description.is_empty() { None } else { Some(description) },
                requirements: None,
                posted_at,
                captured_at: now,
                extra: {
                    let mut map = std::collections::HashMap::new();
                    if let Some(remote) = p.location.as_ref().and_then(|l| l.remote) {
                        map.insert("remote".to_string(), serde_json::json!(remote));
                    }
                    map
                },
            };

            if let Some(ref on_item) = ctx.on_item {
                on_item(posting.clone());
            }

            out.push(posting);

            if let Some(ref on_progress) = ctx.on_progress {
                on_progress((i + 1) as f32 / total as f32);
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smartrecruiters_scraper_id() {
        let scraper = SmartRecruitersScraper;
        assert_eq!(scraper.id(), "smartrecruiters");
    }

    #[test]
    fn test_smartrecruiters_scraper_display_name() {
        let scraper = SmartRecruitersScraper;
        assert_eq!(scraper.display_name(), "SmartRecruiters");
    }

    #[test]
    fn test_smartrecruiters_scraper_mode() {
        let scraper = SmartRecruitersScraper;
        assert_eq!(scraper.mode(), ScraperMode::Http);
    }

    #[test]
    fn test_location_struct_fields() {
        let location = Location {
            city: Some("Berlin".to_string()),
            country: Some("Germany".to_string()),
            remote: Some(true),
        };
        assert_eq!(location.city, Some("Berlin".to_string()));
        assert_eq!(location.remote, Some(true));
    }

    #[test]
    fn test_location_struct_defaults() {
        let location = Location {
            city: None,
            country: None,
            remote: None,
        };
        assert!(location.city.is_none());
        assert!(location.remote.is_none());
    }

    #[test]
    fn test_posting_struct_fields() {
        let posting = Posting {
            id: "123".to_string(),
            uuid: Some("abc".to_string()),
            name: "Software Engineer".to_string(),
            location: None,
            released_date: None,
            ref_field: None,
        };
        assert_eq!(posting.id, "123");
        assert_eq!(posting.name, "Software Engineer");
    }

    #[test]
    fn test_smartrecruiters_scraper_mode_partial_eq() {
        let mode = ScraperMode::Http;
        assert_eq!(mode, ScraperMode::Http);
        assert_ne!(mode, ScraperMode::Browser);
    }
}
