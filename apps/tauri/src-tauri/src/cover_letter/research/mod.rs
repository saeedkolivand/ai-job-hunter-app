pub mod extractor;

use std::time::Duration;

use async_trait::async_trait;
use tauri::Manager;

use crate::pipeline::cache::KvCache;
use crate::pipeline::enrichment::{Enricher, EnrichmentResult};
use crate::pipeline::Completer;

const CACHE_NS: &str = "company_brief";
const TTL_SECS: i64 = 7 * 24 * 3600;
/// Hard cap on a single research call so generation never stalls on a slow or
/// hung provider search. Provider-agnostic — applied once here, around the
/// active provider's own `research()`.
const RESEARCH_TIMEOUT_SECS: u64 = 25;

/// Company-research enricher: extract company → cache check → the **active
/// provider's own** web search + brief synthesis (via [`Completer::research`]) →
/// cache store. Degrades gracefully — any missing key / unsupported provider /
/// failure / timeout yields an empty brief, never an error, so generation still
/// proceeds. The brief is reused by cover letters **and** application answers.
pub struct CompanyResearch;

#[async_trait]
impl Enricher for CompanyResearch {
    async fn enrich(&self, completer: &Completer, input: &str) -> EnrichmentResult {
        let meta = extractor::extract(input);
        let company = meta.company.clone();
        if company.is_empty() {
            tracing::debug!("research: could not extract company name from job ad");
            return EnrichmentResult::empty();
        }

        let app = completer.app();

        // Fast path: cached brief younger than the TTL. Company-scoped (not
        // provider-scoped) so a cover letter + application answers in one session
        // share a single paid search.
        if let Some(cache) = app.try_state::<KvCache>() {
            if let Some(brief) = cache.get(CACHE_NS, &company, TTL_SECS) {
                tracing::info!(
                    company = %company,
                    source = "cache",
                    chars = brief.len(),
                    "research: company brief\n{brief}"
                );
                return EnrichmentResult {
                    key: company,
                    content: brief,
                };
            }
        }

        // Provider-native research, bounded so generation never stalls. Any
        // failure / timeout / unconfigured provider yields an empty brief.
        let brief = match tokio::time::timeout(
            Duration::from_secs(RESEARCH_TIMEOUT_SECS),
            completer.research(&company, &meta.role),
        )
        .await
        {
            Ok(Ok(b)) => b,
            Ok(Err(e)) => {
                tracing::warn!("research: provider research failed for {company}: {e}");
                String::new()
            }
            Err(_) => {
                tracing::warn!("research: timed out for {company}");
                String::new()
            }
        };

        if brief.is_empty() {
            tracing::info!(
                company = %company,
                "research: no brief produced (provider can't search, isn't configured, or failed)"
            );
        } else {
            tracing::info!(
                company = %company,
                role = %meta.role,
                source = "provider",
                chars = brief.len(),
                "research: company brief\n{brief}"
            );
            if let Some(cache) = app.try_state::<KvCache>() {
                cache.set(CACHE_NS, &company, &brief);
            }
        }

        EnrichmentResult {
            key: company,
            content: brief,
        }
    }
}
