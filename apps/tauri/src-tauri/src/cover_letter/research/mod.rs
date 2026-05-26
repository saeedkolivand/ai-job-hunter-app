pub mod brief;
pub mod extractor;
pub mod search;

use super::cache::CompanyBriefCache;
use super::llm::LlmProvider;

/// Full research pipeline: extract → cache-check → search → brief → cache-store.
///
/// Returns `(company_name, brief_text)`. The brief is empty when research is
/// skipped (no Brave key, company name could not be extracted, or search fails).
/// The caller treats an empty brief as graceful degradation — generation still
/// proceeds without the extra context.
pub async fn run(
    llm: &dyn LlmProvider,
    job_ad: &str,
    cache: &CompanyBriefCache,
    brave_key: Option<&str>,
) -> (String, String) {
    let meta = extractor::extract(job_ad);
    let company = meta.company.clone();

    if company.is_empty() {
        tracing::debug!("research: could not extract company name from job ad");
        return (String::new(), String::new());
    }

    // Fast path: return cached brief if available
    if let Some(cached) = cache.get(&company) {
        tracing::debug!(company = %company, "research: cache hit");
        return (company, cached);
    }

    // Need a Brave key to search
    let key = match brave_key {
        Some(k) if !k.is_empty() => k,
        _ => {
            tracing::debug!(company = %company, "research: no brave key, skipping search");
            return (company, String::new());
        }
    };

    let http = match reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("research: http client build failed: {e}");
            return (company, String::new());
        }
    };

    let query = format!("{} company overview site:linkedin.com OR crunchbase.com OR {}",
        company,
        // Fallback to a broad query if domain unknown
        "bloomberg.com");

    let results = match search::brave_search(&http, key, &query, 5).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("research: search failed for {company}: {e}");
            return (company, String::new());
        }
    };

    let brief = match brief::synthesise(llm, &company, &meta.role, &results).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("research: brief synthesis failed for {company}: {e}");
            return (company, String::new());
        }
    };

    if !brief.is_empty() {
        cache.set(&company, &brief);
    }

    (company, brief)
}
