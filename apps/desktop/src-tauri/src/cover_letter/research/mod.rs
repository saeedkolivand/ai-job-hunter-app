pub mod extractor;

use std::time::Duration;

use tauri::Manager;

use crate::pipeline::cache::KvCache;
use crate::pipeline::enrichment::EnrichmentResult;
use crate::pipeline::Completer;

const CACHE_NS: &str = "company_brief";
const TTL_SECS: i64 = 7 * 24 * 3600;

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
    ///
    /// `deadline` bounds the whole pass (search + synthesis) and is INJECTED by
    /// the L3 caller — which derives it from the request's reasoning effort via
    /// `timeouts::research_deadline` — rather than resolved here, for the same
    /// reason `SalaryResearch::enrich` takes its `cache`: this module must not
    /// reach up into `commands` (R7), and an injected bound is directly
    /// testable. A FLAT bound was the bug: synthesis is a model call, so its
    /// cost scales with the model's reasoning budget, and a reasoning model's
    /// research never finished inside the old fixed 25s.
    pub async fn enrich_with(
        &self,
        completer: &Completer,
        job_ad: &str,
        company_override: Option<&str>,
        role_override: Option<&str>,
        deadline: Duration,
    ) -> EnrichmentResult {
        let meta = extractor::extract(job_ad);
        let company = pick(company_override, &meta.company);
        // Same precedence as `company`, and for the same reason: the heuristic's
        // last resort is "the ad's first short line", which on a scraped page is
        // routinely an apply button or a nav link. A reported session searched
        // for role="Jetzt bewerben" and role="[← Alle offenen Stellen](/karriere)".
        let role = pick(role_override, &meta.role);
        if company.is_empty() {
            tracing::debug!("research: no company name available (override + extraction empty)");
            return EnrichmentResult::empty();
        }

        let app = completer.app();
        // ADR-017 key discipline: the brief is the OUTPUT of the active
        // provider's own model doing web search + synthesis
        // (`Completer::research` → either the provider's native search or
        // `searched_research`'s `provider.complete(app, model, …)`), so
        // provider + model are cache-key terms — the same reasoning
        // `pipeline::resume::cache::StageIdentity` documents. Without them, a
        // user who switches models kept getting the OLD model's cached brief
        // for the full 7-day TTL. Still company-scoped WITHIN a (provider,
        // model) pair (not per-request-scoped), so a cover letter +
        // application answers in one session still share a single paid search.
        let key = cache_key(
            completer.provider_id().as_str(),
            completer.model(),
            &company,
        );

        // Fast path: cached brief younger than the TTL.
        if let Some(cache) = app.try_state::<KvCache>() {
            if let Some(brief) = cache.get(CACHE_NS, &key, TTL_SECS) {
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
        let brief = match tokio::time::timeout(deadline, completer.research(&company, &role)).await
        {
            Ok(Ok(b)) => b,
            Ok(Err(e)) => {
                tracing::warn!("research: provider research failed for {company}: {e}");
                String::new()
            }
            Err(_) => {
                tracing::warn!(
                    company = %company,
                    deadline_secs = deadline.as_secs(),
                    "research: timed out"
                );
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
            role = %role,
            source = "provider",
            chars = brief.len(),
            "research: company brief\n{brief}"
        );
        if let Some(cache) = app.try_state::<KvCache>() {
            cache.set(CACHE_NS, &key, &brief);
        }

        EnrichmentResult {
            key: company,
            content: brief,
        }
    }
}

/// The AI-extracted value when it is non-empty, else the heuristic one. The
/// AI extraction sees the whole ad and is simply better; the heuristic exists
/// for the paths that have no extraction to offer.
fn pick(override_value: Option<&str>, heuristic: &str) -> String {
    override_value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(heuristic)
        .to_string()
}

/// Build the `company_brief` cache key. Provider + model are cache-key terms
/// (not just `company`) because the cached value is the OUTPUT of that
/// provider's own model — see the ADR-017 note at the `cache_key` call site
/// in [`CompanyResearch::enrich_with`]. Not case-folded: `company` is used
/// verbatim elsewhere in this module, and folding it here would be an
/// unrelated behavior change. Pure + unit-tested.
fn cache_key(provider: &str, model: &str, company: &str) -> String {
    format!("{provider}|{model}|{company}")
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

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::{cache_key, is_no_info, CACHE_NS, TTL_SECS};
    use crate::pipeline::cache::KvCache;

    // ── cache_key (ADR-017: provider + model are cache-key terms) ───────────

    #[test]
    fn cache_key_differs_by_model_so_a_model_switch_never_hits_the_old_brief() {
        let ollama_a = cache_key("ollama", "llama3", "Acme");
        let ollama_b = cache_key("ollama", "gpt-oss:20b", "Acme");
        assert_ne!(
            ollama_a, ollama_b,
            "same provider + company but a different model must be a different key"
        );
    }

    #[test]
    fn cache_key_differs_by_provider_for_the_same_model_name() {
        let openai = cache_key("openai", "gpt-4o", "Acme");
        let compatible = cache_key("openai-compatible", "gpt-4o", "Acme");
        assert_ne!(openai, compatible);
    }

    #[test]
    fn cache_key_is_identical_for_the_same_provider_model_and_company() {
        assert_eq!(
            cache_key("ollama", "llama3", "Acme"),
            cache_key("ollama", "llama3", "Acme")
        );
    }

    // ── KvCache round-trip: proves the fix at the storage layer, not just the
    // key-builder in isolation — this is what `enrich_with` actually does. ───

    #[test]
    fn switching_models_misses_the_other_models_cached_brief() {
        let dir = TempDir::new().expect("tempdir");
        let cache = KvCache::open(dir.path()).expect("open cache");
        let old_key = cache_key("ollama", "llama3", "Acme");
        cache.set(CACHE_NS, &old_key, "Acme is a fintech (llama3's brief).");

        // The defect this fixes: before, both models shared the SAME row
        // (keyed on company alone), so switching models kept serving the old
        // model's brief for the whole 7-day TTL instead of recomputing.
        let new_key = cache_key("ollama", "gpt-oss:20b", "Acme");
        assert_eq!(
            cache.get(CACHE_NS, &new_key, TTL_SECS),
            None,
            "a different model must MISS the other model's cached brief, forcing a fresh compute"
        );
    }

    #[test]
    fn the_same_model_still_hits_its_own_cached_brief() {
        let dir = TempDir::new().expect("tempdir");
        let cache = KvCache::open(dir.path()).expect("open cache");
        let key = cache_key("ollama", "llama3", "Acme");
        cache.set(CACHE_NS, &key, "Acme is a fintech (llama3's brief).");

        assert_eq!(
            cache.get(CACHE_NS, &key, TTL_SECS),
            Some("Acme is a fintech (llama3's brief).".to_string())
        );
    }

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
