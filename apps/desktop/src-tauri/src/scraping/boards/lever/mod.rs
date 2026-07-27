/// Lever — public JSON board API
///
/// Endpoint: `https://api.lever.co/v0/postings/{company}?mode=json`
/// No global keyword search — requires a company slug. The engine skips this
/// board with `"needs-company"` when `input.companies` is empty.
use super::super::http::{fetch_json, FetchOptions};
use super::super::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};
use super::common::{ats_all_fetches_failed, ats_failed_fetches_note};
use async_trait::async_trait;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Categories {
    location: Option<String>,
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    team: Option<String>,
    #[allow(dead_code)] // serde-deserialized; kept for completeness / future use
    commitment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LeverPosting {
    id: String,
    text: String,
    #[serde(rename = "hostedUrl")]
    hosted_url: String,
    categories: Option<Categories>,
    #[serde(rename = "descriptionPlain")]
    description_plain: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<i64>,
}

/// Map a Lever `createdAt` value (already epoch-milliseconds) to the
/// `posted_at` field.  Exposed for testing so the test exercises this exact
/// function rather than a local re-implementation that could drift.
#[inline]
pub(crate) fn map_posted_at(created_at: Option<i64>) -> Option<i64> {
    created_at // createdAt is already epoch-milliseconds — pass through verbatim
}

pub struct LeverScraper;

#[async_trait]
impl Scraper for LeverScraper {
    fn id(&self) -> &'static str {
        "lever"
    }

    fn display_name(&self) -> &'static str {
        "Lever"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    fn requires_company(&self) -> bool {
        true
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // Engine skips us when companies is empty; guard defensively anyway.
        if input.companies.is_empty() {
            return Ok(vec![]);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let mut out = vec![];
        let total = input.companies.len();

        let mut successful_fetches = 0usize;
        let mut failed_fetches = 0usize;
        let mut first_fetch_error: Option<String> = None;

        for (i, company) in input.companies.iter().enumerate() {
            if ctx.signal.is_cancelled() {
                break;
            }

            let company = company.trim();
            if company.is_empty() {
                continue;
            }

            let url = format!(
                "https://api.lever.co/v0/postings/{}?mode=json",
                urlencoding::encode(company)
            );

            let opts = FetchOptions {
                // One Lever call returns a company's WHOLE board, so a large
                // employer's payload can exceed the 8 MB default guard (observed:
                // veeva → "Response too large", i.e. a silently dropped company).
                // Raise the per-request cap only here.
                max_bytes: Some(16 * 1024 * 1024),
                ..Default::default()
            };

            let postings =
                match fetch_json::<Vec<LeverPosting>>(&url, opts, ctx.signal.clone()).await {
                    Ok(d) => d,
                    Err(e) => {
                        // A fetch that failed because the run was cancelled is not
                        // a real board-level error.
                        if ctx.signal.is_cancelled() {
                            break;
                        }
                        log::warn!("[lever] fetch failed for '{}': {e}", company);
                        failed_fetches += 1;
                        first_fetch_error.get_or_insert_with(|| e.to_string());
                        continue;
                    }
                };
            successful_fetches += 1;

            for p in postings {
                let posting = JobPosting {
                    id: format!("{}:{}", self.id(), p.id),
                    external_id: Some(p.id.clone()),
                    title: p.text,
                    company: company.to_string(),
                    location: p.categories.as_ref().and_then(|c| c.location.clone()),
                    url: p.hosted_url,
                    source: self.id().to_string(),
                    description: p.description_plain,
                    requirements: None,
                    posted_at: map_posted_at(p.created_at), // createdAt is already epoch-milliseconds
                    captured_at: now,
                    extra: std::collections::HashMap::new(),
                };

                if let Some(ref on_item) = ctx.on_item {
                    on_item(posting.clone());
                }

                out.push(posting);
            }

            if let Some(ref on_progress) = ctx.on_progress {
                on_progress((i + 1) as f32 / total as f32);
            }
        }

        // A PARTIAL run (some companies fetched, some failed) still returns Ok, so
        // the failures would otherwise be log-only — surface the count as ONE
        // informational note. Gated on non-cancellation: a benign interruption
        // reports nothing (mirrors `ats_finish_search`).
        if !ctx.signal.is_cancelled() {
            if let Some(note) = ats_failed_fetches_note(successful_fetches, failed_fetches) {
                ctx.report_note(note);
            }
        }

        // Distinguishes an all-slug 403/parse-drift run from a genuine zero result;
        // see `ats_all_fetches_failed` for the decision.
        if let Some(message) =
            ats_all_fetches_failed(self.id(), successful_fetches, &first_fetch_error)
        {
            return Err(anyhow::anyhow!(message));
        }

        Ok(out)
    }
}

#[cfg(test)]
mod test;
