/// Bundesagentur für Arbeit — official German federal employment agency
///
/// Detail endpoint note: `/jobdetails/{hash}` is opportunistic (404s seen from
/// non-browser clients). Jobs are emitted from list data alone; description is
/// populated only when the detail fetch succeeds. This degrades gracefully — list data has title/company/
/// location/url which is sufficient for search results.
///
use super::super::http::{fetch_json, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use super::common::should_propagate_page_error;
use crate::error::AppError;
use async_trait::async_trait;
use serde::Deserialize;

/// List endpoint. v4 now answers 403 (#1494); v6 is the live list API.
const LIST_BASE: &str = "https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v6";
/// Details stay on v4: v6 `/jobdetails/{hash}` answers 403 while v4 answers 200
/// (checked live 2026-11), keyed by base64url(referenznummer).
const DETAIL_BASE: &str = "https://rest.arbeitsagentur.de/jobboerse/jobsuche-service/pc/v4";
const API_KEY: &str = "jobboerse-jobsuche";

mod model;
use model::*;

#[derive(Debug, Deserialize)]
struct DetailResp {
    #[serde(rename = "stellenangebotsBeschreibung")]
    stellenangebots_beschreibung: Option<String>,
    arbeitgeberdarstellung: Option<String>,
}

pub struct ArbeitsagenturScraper;

impl ArbeitsagenturScraper {
    fn to_base64_url(&self, s: &str) -> String {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
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

    fn supports_location(&self) -> bool {
        // The free-text location is sent server-side as the `wo` (where) param.
        true
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
        let max_pages = input.pages.clamp(1, 10);
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

            let list_resp = match fetch_json::<ListResp>(
                &format!("{}/jobs?{}", LIST_BASE, query_string),
                super::super::http::FetchOptions {
                    headers: Some(vec![
                        ("X-API-Key".to_string(), API_KEY.to_string()),
                        ("accept-language".to_string(), "de-DE,de;q=0.9".to_string()),
                    ]),
                    ..Default::default()
                },
                ctx.signal.clone(),
            )
            .await
            {
                Ok(r) => r,
                // A cancel firing mid-fetch is a clean stop, not a failure —
                // even on page 0 with nothing collected yet, this must return
                // `Ok(out)` (empty), not bubble as an error.
                Err(AppError::Cancelled) => break,
                Err(e) if should_propagate_page_error(out.len()) => return Err(e.into()),
                Err(e) => {
                    log::warn!(
                        "[arbeitsagentur] page {page} failed: {e}; returning {} collected",
                        out.len()
                    );
                    ctx.report_truncation(format!("page {page} of {max_pages} failed: {e}"));
                    break;
                }
            };

            // Page 1 with neither a list nor a zero total is a changed response
            // shape (the v4 -> v6 rename was silent): fail loudly so board health
            // records it instead of reading as "found nothing".
            if page == 1 {
                list_resp.check_first_page()?;
            }
            let items = list_resp.ergebnisliste.unwrap_or_default();

            if items.is_empty() {
                break;
            }

            for j in &items {
                if ctx.signal.is_cancelled() {
                    break;
                }

                let refnr = &j.referenznummer;
                if refnr.is_empty() || seen.contains(refnr) {
                    continue;
                }

                seen.insert(refnr.clone());

                // Attempt detail fetch opportunistically — 404s are common from non-browser
                // clients (bot filter). On failure/404 we still emit the job from list data.
                let hash = self.to_base64_url(refnr);

                // Detail is opportunistic — 404s are common from non-browser clients
                // (bot filter). On any failure we still emit the job from list data,
                // so a failed detail fetch collapses to `None`, not a board error.
                let detail = fetch_json::<DetailResp>(
                    &format!("{}/jobdetails/{}", DETAIL_BASE, urlencoding::encode(&hash)),
                    super::super::http::FetchOptions {
                        headers: Some(vec![("X-API-Key".to_string(), API_KEY.to_string())]),
                        ..Default::default()
                    },
                    ctx.signal.clone(),
                )
                .await
                .ok();

                let description = detail.as_ref().and_then(|d| {
                    let desc = vec![
                        d.stellenangebots_beschreibung.as_deref(),
                        d.arbeitgeberdarstellung.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    .map(strip_html)
                    .collect::<Vec<_>>()
                    .join("\n\n");
                    if desc.is_empty() {
                        None
                    } else {
                        Some(desc)
                    }
                });

                let adresse = j
                    .stellenlokationen
                    .as_ref()
                    .and_then(|l| l.first())
                    .and_then(|l| l.adresse.as_ref());
                let location = format_location(adresse);

                let posted_at = j.posted_at_ms();

                let posting = JobPosting {
                    id: format!("{}:{}", self.id(), refnr),
                    external_id: Some(refnr.clone()),
                    title: j
                        .titel
                        .as_deref()
                        .or(j.hauptberuf.as_deref())
                        .unwrap_or("")
                        .trim()
                        .to_string(),
                    company: j.firma.as_deref().unwrap_or("Unbekannt").trim().to_string(),
                    location: if location.is_empty() {
                        None
                    } else {
                        Some(location)
                    },
                    url: j.externe_url.clone().unwrap_or_else(|| {
                        // The public jobdetail deep-link uses the hashId (base64url of refnr),
                        // NOT the raw refnr — the raw form returns 404 on the website.
                        // `hash` was already computed above for the API detail fetch.
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
            tokio::time::sleep(std::time::Duration::from_millis(
                700 + (rand::random::<u64>() % 500),
            ))
            .await;
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests;
