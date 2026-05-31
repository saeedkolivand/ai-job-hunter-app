pub mod brief;
pub mod extractor;
pub mod search;

use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;
use tauri::Manager;

use crate::pipeline::cache::KvCache;
use crate::pipeline::enrichment::{Enricher, EnrichmentResult};
use crate::pipeline::Completer;

const CACHE_NS: &str = "company_brief";
const TTL_SECS: i64 = 7 * 24 * 3600;

/// Company-research enricher: extract company → cache check → Brave search →
/// brief synthesis (via the centralized provider) → cache store. Degrades
/// gracefully — any missing key/failure yields an empty brief, never an error,
/// so generation still proceeds.
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

        // Fast path: cached brief younger than the TTL.
        if let Some(cache) = app.try_state::<KvCache>() {
            if let Some(brief) = cache.get(CACHE_NS, &company, TTL_SECS) {
                tracing::debug!(company = %company, "research: cache hit");
                return EnrichmentResult {
                    key: company,
                    content: brief,
                };
            }
        }

        // Need a Brave key to search.
        let brave_key = app
            .try_state::<Mutex<crate::credentials::CredentialStore>>()
            .and_then(|store| store.lock().get_decrypted("ai:brave").map(|(_, k)| k));
        let key = match brave_key {
            Some(k) if !k.is_empty() => k,
            _ => {
                tracing::debug!(company = %company, "research: no brave key, skipping search");
                return EnrichmentResult {
                    key: company,
                    content: String::new(),
                };
            }
        };

        let http = match crate::net::http::build_client(crate::net::http::ClientConfig {
            timeout: Some(Duration::from_secs(10)),
            ..Default::default()
        }) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("research: http client build failed: {e}");
                return EnrichmentResult {
                    key: company,
                    content: String::new(),
                };
            }
        };

        let query = format!(
            "{} company overview site:linkedin.com OR crunchbase.com OR bloomberg.com",
            company
        );
        let results = match search::brave_search(&http, &key, &query, 5).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("research: search failed for {company}: {e}");
                return EnrichmentResult {
                    key: company,
                    content: String::new(),
                };
            }
        };

        let brief = match brief::synthesise(completer, &company, &meta.role, &results).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("research: brief synthesis failed for {company}: {e}");
                return EnrichmentResult {
                    key: company,
                    content: String::new(),
                };
            }
        };

        if !brief.is_empty() {
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
