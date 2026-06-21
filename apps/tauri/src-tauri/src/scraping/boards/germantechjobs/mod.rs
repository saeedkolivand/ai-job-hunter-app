/// GermanTechJobs — custom XML feed (<jobs><job>…</job></jobs>)
///
/// Endpoint: https://germantechjobs.de/job_feed.xml
/// The old /rss endpoint returned HTTP 403 (dead as of 2026-06).
/// The new feed is a non-RSS custom XML schema that feed_rs cannot parse;
/// we use the same regex-per-block approach as the Personio scraper.
use super::super::http::{fetch_text, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;

// ── module-level compiled regexes (mirrors Personio pattern) ─────────────────
// One compile per process; never inside the per-job loop.

static JOB_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    // `\b` prevents a hypothetical future <job-type> tag from opening a bogus block.
    regex::Regex::new(r"(?s)<job\b[^>]*>(.*?)</job>").unwrap()
});
static ID_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<id>(.*?)</id>").unwrap());
static TITLE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<title>(.*?)</title>").unwrap());
static NAME_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<name>(.*?)</name>").unwrap());
static COMPANY_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<company>(.*?)</company>").unwrap());
static COMPANY_NAME_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?s)<company-name>(.*?)</company-name>").unwrap()
});
static LOCATION_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<location>(.*?)</location>").unwrap());
static CITY_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<city>(.*?)</city>").unwrap());
static REGION_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<region>(.*?)</region>").unwrap());
static URL_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<url>(.*?)</url>").unwrap());
static LINK_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<link>(.*?)</link>").unwrap());
static APPLY_URL_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<apply_url>(.*?)</apply_url>").unwrap());
static PUBDATE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?s)<pubdate>(.*?)</pubdate>").unwrap());
static DESCRIPTION_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?s)<description>(.*?)</description>").unwrap()
});

// ── helpers ───────────────────────────────────────────────────────────────────

/// Extract the text content of the first match of `re` from `block`.
/// Unwraps CDATA and trims. Returns an empty string when absent.
fn cap(re: &regex::Regex, block: &str) -> String {
    re.captures(block)
        .and_then(|c| c.get(1).map(|m| unwrap_cdata(m.as_str().trim())))
        .unwrap_or_default()
}

/// Strip the CDATA wrapper if present; otherwise return the input as-is.
fn unwrap_cdata(s: &str) -> String {
    if let Some(inner) = s
        .strip_prefix("<![CDATA[")
        .and_then(|t| t.strip_suffix("]]>"))
    {
        inner.trim().to_string()
    } else {
        s.to_string()
    }
}

// ── shared parse logic ────────────────────────────────────────────────────────

/// Parse a GermanTechJobs XML feed into job postings.
///
/// Extracted from `search()` so tests can drive the parser directly without
/// replicating the loop — a verbatim copy in tests would silently diverge.
pub(crate) fn parse_feed(xml: &str, scraper_id: &str, now: i64) -> Vec<JobPosting> {
    let mut out = Vec::new();

    for job_cap in JOB_RE.captures_iter(xml) {
        let Some(job_match) = job_cap.get(1) else {
            continue;
        };
        let block = job_match.as_str();

        // ── field extraction ──────────────────────────────────────────────────

        // id: prefer <id>, fall back to <link>
        let raw_id = {
            let v = cap(&ID_RE, block);
            if v.is_empty() {
                cap(&LINK_RE, block)
            } else {
                v
            }
        };
        if raw_id.is_empty() {
            continue; // no usable id — skip
        }

        // title: prefer <title>, fall back to <name>
        let title = {
            let v = cap(&TITLE_RE, block);
            if v.is_empty() {
                cap(&NAME_RE, block)
            } else {
                v
            }
        };
        if title.is_empty() {
            continue;
        }

        // company: <company> → <company-name> → "Unknown"
        let company = {
            let v = cap(&COMPANY_RE, block);
            if v.is_empty() {
                let v2 = cap(&COMPANY_NAME_RE, block);
                if v2.is_empty() {
                    "Unknown".to_string()
                } else {
                    v2
                }
            } else {
                v
            }
        };

        // location: <location> (full address) or "<city>, <region>" fallback
        let location = {
            let v = cap(&LOCATION_RE, block);
            if !v.is_empty() {
                Some(v)
            } else {
                let city = cap(&CITY_RE, block);
                let region = cap(&REGION_RE, block);
                match (city.is_empty(), region.is_empty()) {
                    (false, false) => Some(format!("{city}, {region}")),
                    (false, true) => Some(city),
                    (true, false) => Some(region),
                    (true, true) => None,
                }
            }
        };

        // url: <url> → <link> → <apply_url>
        let url = {
            let v = cap(&URL_RE, block);
            if !v.is_empty() {
                v
            } else {
                let v2 = cap(&LINK_RE, block);
                if !v2.is_empty() {
                    v2
                } else {
                    cap(&APPLY_URL_RE, block)
                }
            }
        };
        if url.is_empty() {
            continue;
        }

        // posted_at: <pubdate> in DD.MM.YYYY format
        let posted_at = {
            let s = cap(&PUBDATE_RE, block);
            if s.is_empty() {
                None
            } else {
                chrono::NaiveDate::parse_from_str(&s, "%d.%m.%Y")
                    .ok()
                    .map(|d| {
                        d.and_hms_opt(0, 0, 0)
                            .expect("midnight is always valid")
                            .and_utc()
                            .timestamp_millis()
                    })
            }
        };

        // description: strip HTML from CDATA content
        let description = {
            let v = cap(&DESCRIPTION_RE, block);
            if v.is_empty() {
                None
            } else {
                Some(strip_html(&v))
            }
        };

        out.push(JobPosting {
            id: format!("{scraper_id}:{raw_id}"),
            external_id: Some(raw_id),
            title,
            company,
            location,
            url,
            source: scraper_id.to_string(),
            description,
            requirements: None,
            posted_at,
            captured_at: now,
            extra: {
                let mut map = std::collections::HashMap::new();
                // Feed content is German; reflect that in the language hint.
                map.insert("language".to_string(), serde_json::json!("de"));
                map
            },
        });
    }

    out
}

// ── scraper ──────────────────────────────────────────────────────────────────

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
            "https://germantechjobs.de/job_feed.xml",
            super::super::http::FetchOptions {
                // fetch_text already injects `accept-language: en-US,en;q=0.9,de;q=0.8`.
                // Signal a German-first preference that better suits this feed.
                headers: Some(vec![(
                    "accept-language".to_string(),
                    "de,en;q=0.9".to_string(),
                )]),
                // GTJ feed can reach ~10 MB; raise the per-request cap only here.
                max_bytes: Some(16 * 1024 * 1024),
                ..Default::default()
            },
            ctx.signal,
        )
        .await?;

        if res.status_code != 200 {
            log::warn!("[germantechjobs] feed fetch returned {}", res.status_code);
            return Ok(vec![]);
        }

        let now = chrono::Utc::now().timestamp_millis();

        let mut out = parse_feed(&res.text, self.id(), now);

        // ── client-side keyword + location filter ─────────────────────────────
        out.retain(|posting| {
            let haystack = format!(
                "{} {} {} {}",
                posting.title,
                posting.company,
                posting.location.as_deref().unwrap_or(""),
                posting.description.as_deref().unwrap_or("")
            )
            .to_lowercase();

            if !q.is_empty() && !haystack.contains(&q) {
                return false;
            }
            if !loc.is_empty() && !haystack.contains(&loc) {
                return false;
            }
            true
        });

        for posting in &out {
            if let Some(ref on_item) = ctx.on_item {
                on_item(posting.clone());
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
