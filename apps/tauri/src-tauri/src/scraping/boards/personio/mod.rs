/// Personio — public XML feed per company
use super::super::http::{fetch_text, strip_html};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use async_trait::async_trait;

const HOSTS: &[&str] = &["jobs.personio.de", "jobs.personio.com"];

// Personio XML feed parsing (shared). The public feed is a flat <position>
// list; the regex set + capture loop is identical for the board scraper and the
// single-URL resolver (scrape_url::try_personio), so parsing lives here once.
// Each caller still builds its own JobPosting (different id/url/posted_at shape).
static POSITION_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<position>(.*?)</position>").unwrap());
static ID_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<id>(.*?)</id>").unwrap());
static NAME_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<name>(.*?)</name>").unwrap());
static OFFICE_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<office>(.*?)</office>").unwrap());
static DESC_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"<jobDescription>\s*<value>(.*?)</value>\s*</jobDescription>").unwrap()
});
static CREATED_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"<createdAt>(.*?)</createdAt>").unwrap());

/// One parsed Personio position. Description is already run through strip_html.
pub(crate) struct PersonioPosition {
    pub id: String,
    pub title: String,
    pub office: String,
    pub description: String,
    pub created: String,
}

/// Parse a Personio XML feed into its positions, skipping empty-id entries.
pub(crate) fn parse_xml_feed(xml: &str) -> Vec<PersonioPosition> {
    let mut out = Vec::new();
    for position_cap in POSITION_RE.captures_iter(xml) {
        let Some(position_content) = position_cap.get(1) else {
            continue;
        };
        let position_str = position_content.as_str();
        let cap = |re: &regex::Regex| {
            re.captures(position_str)
                .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
                .unwrap_or_default()
        };
        let id = cap(&ID_RE);
        if id.is_empty() {
            continue;
        }
        let description = DESC_RE
            .captures(position_str)
            .and_then(|c| c.get(1).map(|m| strip_html(m.as_str().trim())))
            .unwrap_or_default();
        out.push(PersonioPosition {
            id,
            title: cap(&NAME_RE),
            office: cap(&OFFICE_RE),
            description,
            created: cap(&CREATED_RE),
        });
    }
    out
}

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

        for pos in parse_xml_feed(&xml) {
            let posted_at = if pos.created.is_empty() {
                None
            } else {
                chrono::DateTime::parse_from_rfc3339(&pos.created)
                    .ok()
                    .map(|dt| dt.timestamp_millis())
            };

            let posting = JobPosting {
                id: format!("{}:{}", self.id(), pos.id),
                external_id: Some(pos.id.clone()),
                title: pos.title,
                company: company.clone(),
                location: if pos.office.is_empty() {
                    None
                } else {
                    Some(pos.office)
                },
                url: format!("https://{}.{}/job/{}", company, HOSTS[0], pos.id),
                source: self.id().to_string(),
                description: if pos.description.is_empty() {
                    None
                } else {
                    Some(pos.description)
                },
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

        if let Some(ref on_progress) = ctx.on_progress {
            on_progress(1.0);
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;
