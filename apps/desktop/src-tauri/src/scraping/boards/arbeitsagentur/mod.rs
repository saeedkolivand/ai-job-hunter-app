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

#[derive(Debug, Deserialize)]
struct Adresse {
    ort: Option<String>,
    region: Option<String>,
    land: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Stellenlokation {
    adresse: Option<Adresse>,
}

#[derive(Debug, Deserialize)]
struct Zeitraum {
    von: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Stellenangebot {
    referenznummer: String,
    #[serde(rename = "stellenangebotsTitel")]
    titel: Option<String>,
    hauptberuf: Option<String>,
    firma: Option<String>,
    stellenlokationen: Option<Vec<Stellenlokation>>,
    /// Current publication window; `von` is the (re)publication date.
    veroeffentlichungszeitraum: Option<Zeitraum>,
    /// `YYYY-MM-DD`; the FIRST publication, so old for republished listings.
    #[serde(rename = "datumErsteVeroeffentlichung")]
    erste_veroeffentlichung: Option<String>,
    #[serde(rename = "externeURL")]
    externe_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListResp {
    /// Absent (not empty) both for a genuine zero-hit search and for a changed
    /// response shape; `maxErgebnisse` tells the two apart.
    ergebnisliste: Option<Vec<Stellenangebot>>,
    #[serde(rename = "maxErgebnisse")]
    max_ergebnisse: Option<i64>,
}

impl ListResp {
    fn is_shape_drift(&self) -> bool {
        self.ergebnisliste.is_none() && self.max_ergebnisse != Some(0)
    }

    /// Page-1 gate: a changed response shape is an error, not "found nothing".
    fn check_first_page(&self) -> Result<(), AppError> {
        if self.is_shape_drift() {
            return Err(AppError::Parse(
                "arbeitsagentur list response has no `ergebnisliste`".into(),
            ));
        }
        Ok(())
    }
}

impl Stellenangebot {
    /// Prefer the current publication date over the first one (republished
    /// listings would otherwise look months old and fail recency filters).
    fn posted_at_ms(&self) -> Option<i64> {
        let d = self
            .veroeffentlichungszeitraum
            .as_ref()
            .and_then(|z| z.von.as_deref())
            .or(self.erste_veroeffentlichung.as_deref())?;
        let d = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()?;
        Some(d.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis())
    }
}

/// v6 writes region/country upper-case ("BERLIN", "DEUTSCHLAND"); show them
/// title-cased. Keeps the country text so the location post-filter (which
/// tokenizes the whole location string, case-insensitively) still sees it.
fn title_case(s: &str) -> String {
    let s = s.replace('_', "-");
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for c in s.chars() {
        if start {
            out.extend(c.to_uppercase());
        } else {
            out.extend(c.to_lowercase());
        }
        start = !c.is_alphanumeric();
    }
    // v6 transliterates ("BADEN_WUERTTEMBERG"); restore the two spellings that
    // occur in region names. The location filter folds ue/ü either way.
    out.replace("Wuerttemberg", "Württemberg")
        .replace("Thueringen", "Thüringen")
}

/// "Berlin, Berlin, DEUTSCHLAND" -> "Berlin, Deutschland": region is dropped
/// when it repeats the city (ignoring case).
fn format_location(a: Option<&Adresse>) -> String {
    let Some(a) = a else { return String::new() };
    let ort = a.ort.as_deref().filter(|s| !s.is_empty());
    let region = a
        .region
        .as_deref()
        .filter(|r| !r.is_empty() && ort.is_none_or(|o| !o.eq_ignore_ascii_case(r)));
    let land = a.land.as_deref().filter(|s| !s.is_empty());
    [
        ort.map(str::to_string),
        region.map(title_case),
        land.map(title_case),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ")
}

#[derive(Debug, Deserialize)]
struct Branche {
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    bezeichnung: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DetailResp {
    // Live API (2026-06): fields renamed; all Option so future renames degrade gracefully.
    #[allow(dead_code)]
    #[serde(rename = "referenznummer")]
    referenznummer: Option<String>,
    #[serde(rename = "stellenangebotsBeschreibung")]
    stellenangebots_beschreibung: Option<String>,
    arbeitgeberdarstellung: Option<String>,
    #[allow(dead_code)]
    branche: Option<Branche>,
    // live API uses "firma" for company name in detail response
    #[allow(dead_code)]
    firma: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "stellenangebotsTitel")]
    stellenangebots_titel: Option<String>,
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
