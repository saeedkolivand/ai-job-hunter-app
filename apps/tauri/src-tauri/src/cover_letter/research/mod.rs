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

/// Company-research enricher: resolve company → cache check → the **active
/// provider's own** web search + brief synthesis (via [`Completer::research`]) →
/// cache store. Degrades gracefully — any missing key / unsupported provider /
/// failure / timeout / "no information" result yields an empty brief, never an
/// error, so generation still proceeds. The brief is reused by cover letters
/// **and** application answers.
pub struct CompanyResearch;

impl CompanyResearch {
    /// Research a company. `company_override` — the accurate company name the
    /// generation flow already AI-extracted — takes precedence over the heuristic
    /// job-ad extraction, which frequently grabs a tagline ("…platform built for
    /// the era of agentic commerce") rather than the company. Falls back to the
    /// heuristic extraction only when the override is absent/empty.
    pub async fn enrich_with(
        &self,
        completer: &Completer,
        job_ad: &str,
        company_override: Option<&str>,
    ) -> EnrichmentResult {
        let meta = extractor::extract(job_ad);
        let company = company_override
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| meta.company.clone());
        if company.is_empty() {
            tracing::debug!("research: no company name available (override + extraction empty)");
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

        // Drop unhelpful "no information" / too-short responses so they neither
        // pollute the cover letter nor get cached (a bad miss must not stick for
        // the 7-day TTL).
        if is_no_info(&brief) {
            tracing::info!(
                company = %company,
                chars = brief.len(),
                "research: no usable brief (provider found nothing)\n{brief}"
            );
            return EnrichmentResult {
                key: company,
                content: String::new(),
            };
        }

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

        EnrichmentResult {
            key: company,
            content: brief,
        }
    }
}

/// A brief the model couldn't actually fill: too short to be a real ~150-word
/// brief, or an explicit "no information" disclaimer. Treated as empty. Pure +
/// unit-tested.
fn is_no_info(brief: &str) -> bool {
    let b = brief.trim().to_lowercase();
    b.len() < 60
        || b.contains("no information")
        || b.contains("not available")
        || b.contains("couldn't find")
        || b.contains("could not find")
        || b.contains("unable to find")
        || b.contains("no relevant")
}

#[async_trait]
impl Enricher for CompanyResearch {
    /// Heuristic-only path (no AI-extracted company available, e.g. the Rust
    /// cover-letter pipeline). Prefer [`enrich_with`](Self::enrich_with) when the
    /// caller already knows the company.
    async fn enrich(&self, completer: &Completer, input: &str) -> EnrichmentResult {
        self.enrich_with(completer, input, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::is_no_info;

    #[test]
    fn is_no_info_flags_empty_short_and_disclaimers() {
        assert!(is_no_info(""));
        assert!(is_no_info("No information available."));
        assert!(is_no_info("  Unable to find details about this company.  "));
        assert!(is_no_info("I could not find any relevant information."));
    }

    #[test]
    fn is_no_info_accepts_a_real_brief() {
        let brief = "Acme is a Series B fintech (≈200 employees) building payment \
            infrastructure for marketplaces. Its core product processes split \
            payouts for platforms; notable customers include several large \
            gig-economy apps. Recently raised funding to expand into Europe, which \
            is relevant for a backend engineer joining the payments team.";
        assert!(!is_no_info(brief));
    }
}
