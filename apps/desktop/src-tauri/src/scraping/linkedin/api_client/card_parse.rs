//! LinkedIn guest job-card HTML parsing: compiled CSS selectors, the
//! `<time>`/relative-text date parsers, and [`parse_cards`] — the pure
//! DOM-to-`JobPosting` extraction `super::search_guest` fetches HTML for.

use std::collections::HashSet;

use scraper::Html;

use crate::scraping::types::JobPosting;

// LinkedIn guest job-card CSS selectors compiled once (Selector is Send + Sync).
static LI_CARD_SEL: std::sync::LazyLock<scraper::Selector> =
    std::sync::LazyLock::new(|| scraper::Selector::parse("li").unwrap());
static LI_LINK_SEL: std::sync::LazyLock<scraper::Selector> = std::sync::LazyLock::new(|| {
    scraper::Selector::parse("a.base-card__full-link, a.base-search-card__link").unwrap()
});
static LI_URN_SEL: std::sync::LazyLock<scraper::Selector> =
    std::sync::LazyLock::new(|| scraper::Selector::parse("[data-entity-urn]").unwrap());
static LI_TITLE_SEL: std::sync::LazyLock<scraper::Selector> = std::sync::LazyLock::new(|| {
    scraper::Selector::parse(".base-search-card__title, .job-card-container__title").unwrap()
});
static LI_COMPANY_SEL: std::sync::LazyLock<scraper::Selector> = std::sync::LazyLock::new(|| {
    scraper::Selector::parse(".base-search-card__subtitle, .job-card-container__subtitle").unwrap()
});
static LI_LOCATION_SEL: std::sync::LazyLock<scraper::Selector> = std::sync::LazyLock::new(|| {
    scraper::Selector::parse(".job-search-card__location, .job-card-container__location").unwrap()
});
static LI_TIME_SEL: std::sync::LazyLock<scraper::Selector> =
    std::sync::LazyLock::new(|| scraper::Selector::parse("time").unwrap());

/// Parse LinkedIn's `<time datetime="…">` attribute into a `DateTime`.
///
/// This is the accurate posting date. It is usually a bare ISO date
/// (`YYYY-MM-DD`); full RFC 3339 timestamps are accepted too. Preferred over the
/// element's visible text, which reflects the repost/refresh time.
pub(super) fn parse_iso_date(value: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let value = value.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(dt);
    }
    let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
    Some(date.and_hms_opt(0, 0, 0)?.and_utc().into())
}

/// Parse relative time strings like "1 hour ago", "30 minutes ago", "2 weeks ago".
///
/// Fallback only — used when the `<time>` element has no `datetime` attribute.
/// Stems are checked most-specific-first so "minute" is not swallowed by the "m"
/// in "month".
pub(super) fn parse_relative_time(text: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let now = chrono::Utc::now();
    let text = text.to_lowercase();

    let num: i64 = text.split_whitespace().next()?.parse().ok()?;

    let duration = if text.contains("minute") || text.contains("min") {
        chrono::Duration::minutes(num)
    } else if text.contains("hour") || text.contains("hr") {
        chrono::Duration::hours(num)
    } else if text.contains("day") {
        chrono::Duration::days(num)
    } else if text.contains("week") {
        chrono::Duration::weeks(num)
    } else if text.contains("month") {
        chrono::Duration::days(num * 30)
    } else if text.contains("year") {
        chrono::Duration::days(num * 365)
    } else {
        return None;
    };

    Some((now - duration).into())
}

/// Parse every LinkedIn guest job card in `html` into a [`JobPosting`],
/// deduplicating by entity-urn id within the page. Pure — the network fetch
/// happens in `super::search_guest`, which is the only caller.
pub(super) fn parse_cards(
    html: &str,
    signal: Option<&tokio_util::sync::CancellationToken>,
) -> Vec<JobPosting> {
    let document = Html::parse_document(html);

    let mut seen = HashSet::new();
    let mut jobs = Vec::new();

    for element in document.select(&LI_CARD_SEL) {
        if let Some(signal) = signal {
            if signal.is_cancelled() {
                break;
            }
        }

        let link = element
            .select(&LI_LINK_SEL)
            .next()
            .and_then(|el| el.value().attr("href"))
            .unwrap_or("");

        let entity_urn = element
            .select(&LI_URN_SEL)
            .next()
            .and_then(|el| el.value().attr("data-entity-urn"));

        let id = entity_urn.and_then(|urn| urn.split(':').next_back());

        let id = match id {
            Some(id_str) => {
                if seen.contains(id_str) {
                    continue;
                }
                seen.insert(id_str.to_string());
                id_str
            }
            None => continue,
        };

        let title = element
            .select(&LI_TITLE_SEL)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();

        let company = element
            .select(&LI_COMPANY_SEL)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();

        let location = element
            .select(&LI_LOCATION_SEL)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .to_string();

        let posted_at = element.select(&LI_TIME_SEL).next().and_then(|el| {
            // Prefer the ISO date in the `datetime` attribute. LinkedIn's
            // visible text ("1 hour ago") reflects when the listing was last
            // reposted/refreshed, not when it was originally posted — so an
            // old job that was recently refreshed shows "1h ago". Fall back to
            // the relative text only when the attribute is missing.
            el.value()
                .attr("datetime")
                .and_then(parse_iso_date)
                .or_else(|| {
                    let text = el.text().collect::<String>().trim().to_lowercase();
                    parse_relative_time(&text)
                })
        });

        let job = JobPosting {
            id: format!("linkedin:{}", id),
            source: "linkedin".to_string(),
            external_id: Some(id.to_string()),
            url: link.split('?').next().unwrap_or("").to_string(),
            title: title.clone(),
            company: company.clone(),
            location: Some(location.clone()),
            // Blank at search time — an autopilot run backfills this after
            // `record_run`, best-effort, via
            // `commands::autopilot::linkedin_enrich` (issue #1114). A manual
            // scrape/import instead resolves it inline through
            // `scraping::scrape_url::resolve`.
            description: Some(String::new()),
            captured_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
            posted_at: posted_at.map(|dt| dt.timestamp_millis()),
            requirements: None,
            extra: std::collections::HashMap::new(),
        };

        jobs.push(job);
    }

    jobs
}
