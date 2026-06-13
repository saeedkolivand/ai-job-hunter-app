/// StepStone (Germany) — public listing pages, parsed with regex/JSON
use super::super::http::{fetch_text, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;
use serde::Deserialize;

// Compiled once and reused across the per-page loop (hoisted to avoid recompiling
// the regex on every iteration).
static LD_JSON_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r#"<script type="application/ld\+json">(.*?)</script>"#).unwrap()
});
static STEP_ID_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"[?&]ID=([^&]+)").unwrap());
static STEP_DIGITS_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(\d{6,})").unwrap());

#[derive(Debug, Deserialize)]
struct JobLocation {
    address: Option<Address>,
}

#[derive(Debug, Deserialize)]
struct Address {
    #[serde(rename = "addressLocality")]
    address_locality: Option<String>,
    #[serde(rename = "addressCountry")]
    address_country: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HiringOrganization {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LdJobPosting {
    #[serde(rename = "@type")]
    at_type: Option<String>,
    url: Option<String>,
    title: Option<String>,
    #[serde(rename = "hiringOrganization")]
    hiring_organization: Option<HiringOrganization>,
    #[serde(rename = "jobLocation")]
    job_location: Option<JobLocation>,
    description: Option<String>,
    #[serde(rename = "datePosted")]
    date_posted: Option<String>,
}

pub struct StepStoneScraper;

#[async_trait]
impl Scraper for StepStoneScraper {
    fn id(&self) -> &'static str {
        "stepstone"
    }

    fn display_name(&self) -> &'static str {
        "StepStone"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let q = input.query.trim();
        let loc = input
            .location
            .as_ref()
            .map(|l| l.trim())
            .unwrap_or_default();
        let max_pages = input.pages.clamp(1, 5);
        let mut out = vec![];
        let mut seen = std::collections::HashSet::new();
        let now = chrono::Utc::now().timestamp_millis();

        for p in 1..=max_pages {
            if ctx.signal.is_cancelled() {
                break;
            }

            let url = if loc.is_empty() {
                format!(
                    "https://www.stepstone.de/jobs/{}?page={}",
                    urlencoding::encode(q),
                    p
                )
            } else {
                format!(
                    "https://www.stepstone.de/jobs/{}/in-{}?page={}",
                    urlencoding::encode(q),
                    urlencoding::encode(loc),
                    p
                )
            };

            let res = match fetch_text(
                &url,
                super::super::http::FetchOptions {
                    headers: Some(vec![(
                        "accept-language".to_string(),
                        "de-DE,de;q=0.9,en;q=0.7".to_string(),
                    )]),
                    ..Default::default()
                },
                ctx.signal.clone(),
            )
            .await
            {
                Ok(r) => r,
                Err(e) if out.is_empty() => return Err(e.into()),
                Err(e) => {
                    log::warn!(
                        "[stepstone] page {p} failed: {e}; returning {} collected",
                        out.len()
                    );
                    break;
                }
            };

            if res.status_code != 200 {
                break;
            }

            // Extract ld+json blocks
            let re = &*LD_JSON_RE;
            let mut found_any = false;

            for cap in re.captures_iter(&res.text) {
                if let Some(json_str) = cap.get(1) {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(json_str.as_str()) {
                        let items = if data.is_array() {
                            data.as_array().unwrap().clone()
                        } else {
                            vec![data]
                        };

                        for item in items {
                            if let Ok(job) = serde_json::from_value::<LdJobPosting>(item) {
                                if job.at_type.as_deref() != Some("JobPosting") {
                                    continue;
                                }

                                let url = job.url.unwrap_or_default();
                                let id = STEP_ID_RE
                                    .captures(&url)
                                    .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
                                    .or_else(|| {
                                        STEP_DIGITS_RE
                                            .captures(&url)
                                            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
                                    })
                                    .unwrap_or_else(|| url.clone());

                                if id.is_empty() || seen.contains(&id) {
                                    continue;
                                }

                                seen.insert(id.clone());
                                found_any = true;

                                let location = vec![
                                    job.job_location
                                        .as_ref()
                                        .and_then(|j| j.address.as_ref())
                                        .and_then(|a| a.address_locality.clone()),
                                    job.job_location
                                        .as_ref()
                                        .and_then(|j| j.address.as_ref())
                                        .and_then(|a| a.address_country.clone()),
                                ]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(", ");

                                let posted_at = job
                                    .date_posted
                                    .and_then(|d| chrono::DateTime::parse_from_rfc3339(&d).ok())
                                    .map(|dt| dt.timestamp_millis());

                                let posting = JobPosting {
                                    id: format!("{}:{}", self.id(), id),
                                    external_id: Some(id),
                                    title: job.title.unwrap_or_default().trim().to_string(),
                                    company: job
                                        .hiring_organization
                                        .and_then(|h| h.name)
                                        .unwrap_or_else(|| "Unknown".to_string())
                                        .trim()
                                        .to_string(),
                                    location: if location.is_empty() {
                                        None
                                    } else {
                                        Some(location)
                                    },
                                    url,
                                    source: self.id().to_string(),
                                    description: job.description.map(|d| strip_html(&d)),
                                    requirements: None,
                                    posted_at,
                                    captured_at: now,
                                    extra: {
                                        let mut map = std::collections::HashMap::new();
                                        map.insert("language".to_string(), serde_json::json!("de"));
                                        map
                                    },
                                };

                                if let Some(ref on_item) = ctx.on_item {
                                    on_item(posting.clone());
                                }

                                out.push(posting);
                            }
                        }
                    }
                }
            }

            if !found_any {
                break;
            }

            if let Some(ref on_progress) = ctx.on_progress {
                on_progress(p as f32 / max_pages as f32);
            }

            // Rate limiting delay
            tokio::time::sleep(std::time::Duration::from_millis(
                900 + (rand::random::<u64>() % 600),
            ))
            .await;
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;
