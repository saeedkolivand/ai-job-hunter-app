#![allow(dead_code)]

/// Bundesagentur für Arbeit — official German federal employment agency
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, Scraper, ScraperMode, ScrapeContext};
use async_trait::async_trait;
use serde::Deserialize;

const API_BASE: &str = "https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v4";
const API_KEY: &str = "jobboerse-jobsuche";

#[derive(Debug, Deserialize)]
struct Arbeitsort {
    ort: Option<String>,
    region: Option<String>,
    land: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Stellenangebot {
    refnr: String,
    titel: Option<String>,
    beruf: Option<String>,
    arbeitgeber: Option<String>,
    arbeitsort: Option<Arbeitsort>,
    #[serde(rename = "aktuelleVeroeffentlichungsdatum")]
    aktuelle_veroeffentlichungsdatum: Option<String>,
    #[serde(rename = "eintrittsdatum")]
    eintrittsdatum: Option<String>,
    #[serde(rename = "externeUrl")]
    externe_url: Option<String>,
    #[serde(rename = "hashId")]
    hash_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListResp {
    stellenangebote: Option<Vec<Stellenangebot>>,
    #[serde(rename = "maxErgebnisse")]
    max_ergebnisse: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct Branche {
    bezeichnung: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DetailResp {
    refnr: String,
    stellenbeschreibung: Option<String>,
    arbeitgeberdarstellung: Option<String>,
    branche: Option<Branche>,
    arbeitgeber: Option<String>,
    titel: Option<String>,
}

pub struct ArbeitsagenturScraper;

impl ArbeitsagenturScraper {
    fn to_base64_url(&self, s: &str) -> String {
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        URL_SAFE_NO_PAD.encode(s.as_bytes())
    }
}

#[async_trait]
impl Scraper for ArbeitsagenturScraper {
    fn id(&self) -> &'static str {
        "arbeitsagentur"
    }

    fn display_name(&self) -> &'static str {
        "Arbeitsagentur"
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
        let loc = input.location.as_ref().map(|l| l.trim()).unwrap_or_default();
        let max_pages = input.pages.min(10).max(1);
        let size = 25;
        let mut out = vec![];
        let mut seen = std::collections::HashSet::new();
        let now = chrono::Utc::now().timestamp_millis();

        for page in 1..=max_pages {
            if ctx.signal.is_cancelled() {
                break;
            }

            let mut params = vec![
                ("was".to_string(), q.to_string()),
                ("page".to_string(), page.to_string()),
                ("size".to_string(), size.to_string()),
            ];

            if !loc.is_empty() {
                params.push(("wo".to_string(), loc.to_string()));
            }

            let query_string = params
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&");

            let list = fetch_json::<ListResp>(
                &format!("{}/jobs?{}", API_BASE, query_string),
                super::super::http::FetchOptions {
                    headers: Some(vec![
                        ("X-API-Key".to_string(), API_KEY.to_string()),
                        ("accept".to_string(), "application/json".to_string()),
                        ("accept-language".to_string(), "de-DE,de;q=0.9".to_string()),
                    ]),
                    ..Default::default()
                },
                ctx.signal.clone(),
            )
            .await?;

            let items = list.and_then(|l| l.stellenangebote).unwrap_or_default();

            if items.is_empty() {
                break;
            }

            for j in &items {
                if ctx.signal.is_cancelled() {
                    break;
                }

                if j.refnr.is_empty() || seen.contains(&j.refnr) {
                    continue;
                }

                seen.insert(j.refnr.clone());

                let hash = j.hash_id.clone().unwrap_or_else(|| self.to_base64_url(&j.refnr));

                let detail = fetch_json::<DetailResp>(
                    &format!("{}/jobdetails/{}", API_BASE, urlencoding::encode(&hash)),
                    super::super::http::FetchOptions {
                        headers: Some(vec![
                            ("X-API-Key".to_string(), API_KEY.to_string()),
                            ("accept".to_string(), "application/json".to_string()),
                        ]),
                        ..Default::default()
                    },
                    ctx.signal.clone(),
                )
                .await?;

                let description = detail.as_ref()
                    .and_then(|d| {
                        let desc = vec![
                            d.stellenbeschreibung.as_deref(),
                            d.arbeitgeberdarstellung.as_deref(),
                        ]
                        .into_iter()
                        .flatten()
                        .map(|s| strip_html(s))
                        .collect::<Vec<_>>()
                        .join("\n\n");
                        if desc.is_empty() { None } else { Some(desc) }
                    });

                let location = vec![
                    j.arbeitsort.as_ref().and_then(|a| a.ort.as_deref()),
                    j.arbeitsort.as_ref().and_then(|a| a.region.as_deref()),
                    j.arbeitsort.as_ref().and_then(|a| a.land.as_deref()),
                ]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ");

                let posted_at = j.aktuelle_veroeffentlichungsdatum
                    .as_ref()
                    .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                    .map(|dt| dt.timestamp_millis());

                let posting = JobPosting {
                    id: format!("{}:{}", self.id(), j.refnr),
                    external_id: Some(j.refnr.clone()),
                    title: j.titel.as_deref().or(j.beruf.as_deref()).unwrap_or("").trim().to_string(),
                    company: j.arbeitgeber.as_deref().unwrap_or("Unbekannt").trim().to_string(),
                    location: if location.is_empty() { None } else { Some(location) },
                    url: j.externe_url.clone().unwrap_or_else(|| {
                        format!(
                            "https://www.arbeitsagentur.de/jobsuche/jobdetail/{}",
                            urlencoding::encode(&hash)
                        )
                    }),
                    source: self.id().to_string(),
                    description,
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

            if let Some(ref on_progress) = ctx.on_progress {
                on_progress(page as f32 / max_pages as f32);
            }

            if items.len() < size as usize {
                break;
            }

            // Rate limiting delay
            tokio::time::sleep(std::time::Duration::from_millis(700 + (rand::random::<u64>() % 500))).await;
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arbeitsagentur_scraper_id() {
        let scraper = ArbeitsagenturScraper;
        assert_eq!(scraper.id(), "arbeitsagentur");
    }

    #[test]
    fn test_arbeitsagentur_scraper_display_name() {
        let scraper = ArbeitsagenturScraper;
        assert_eq!(scraper.display_name(), "Arbeitsagentur");
    }

    #[test]
    fn test_arbeitsagentur_scraper_mode() {
        let scraper = ArbeitsagenturScraper;
        assert_eq!(scraper.mode(), ScraperMode::Http);
    }

    #[test]
    fn test_to_base64_url() {
        let scraper = ArbeitsagenturScraper;
        let result = scraper.to_base64_url("test");
        assert!(!result.is_empty());
    }

    #[test]
    fn test_to_base64_url_empty() {
        let scraper = ArbeitsagenturScraper;
        let result = scraper.to_base64_url("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_to_base64_url_special_chars() {
        let scraper = ArbeitsagenturScraper;
        let result = scraper.to_base64_url("test-123_abc");
        assert!(!result.is_empty());
    }

    #[test]
    fn test_arbeitsagentur_scraper_mode_partial_eq() {
        let mode = ScraperMode::Http;
        assert_eq!(mode, ScraperMode::Http);
        assert_ne!(mode, ScraperMode::Browser);
    }

    #[test]
    fn test_arbeitsort_struct() {
        let ort = Arbeitsort {
            ort: Some("Berlin".to_string()),
            region: Some("Berlin".to_string()),
            land: Some("Germany".to_string()),
        };
        assert_eq!(ort.ort, Some("Berlin".to_string()));
    }

    #[test]
    fn test_arbeitsort_struct_defaults() {
        let ort = Arbeitsort {
            ort: None,
            region: None,
            land: None,
        };
        assert!(ort.ort.is_none());
    }

    #[test]
    fn test_stellenangebot_struct() {
        let offer = Stellenangebot {
            refnr: "123456".to_string(),
            titel: Some("Software Engineer".to_string()),
            beruf: None,
            arbeitgeber: Some("Test Corp".to_string()),
            arbeitsort: None,
            aktuelle_veroeffentlichungsdatum: None,
            eintrittsdatum: None,
            externe_url: None,
            hash_id: None,
        };
        assert_eq!(offer.refnr, "123456");
        assert_eq!(offer.titel, Some("Software Engineer".to_string()));
    }

    #[test]
    fn test_branche_struct() {
        let branche = Branche {
            bezeichnung: Some("IT".to_string()),
        };
        assert_eq!(branche.bezeichnung, Some("IT".to_string()));
    }

    #[test]
    fn test_list_resp_struct() {
        let resp = ListResp {
            stellenangebote: Some(vec![]),
            max_ergebnisse: Some(100),
        };
        assert!(resp.stellenangebote.as_ref().map(|v| v.is_empty()).unwrap_or(true));
    }
}
