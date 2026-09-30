//! Cross-provider dedup (`dedupe`, `canonical_url`, `dedupe_by_url`) and the
//! additive Apify-LinkedIn merge on top of the fallback chain
//! (`search_with_providers`) — split out of `mod.rs` (R8 module-size guard).

use crate::scraping::types::JobPosting;

use super::budget::{apify_cap, SearchBudget};
use super::fallback::{primary_chain, JobProvider};

/// Deduplicate by `external_id`, preserving first-seen order.
pub(super) fn dedupe(items: Vec<JobPosting>) -> Vec<JobPosting> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|p| {
            let key = p.external_id.clone().unwrap_or_else(|| p.url.clone());
            seen.insert(key)
        })
        .collect()
}

/// Normalise a URL for deduplication.
///
/// For `linkedin.com` hosts, strip the query string so tracking-only variants
/// (`?trk=…`, `?refId=…`) of the same job URL are treated as identical.
/// For every other host, keep the query string intact: some boards encode the
/// job id in query params, so stripping would merge distinct jobs.
pub(super) fn canonical_url(url: &str) -> String {
    let trimmed = url.trim();
    // `Url::parse` normalises scheme and host to lowercase (URL standard) while
    // preserving the original-case path and query — so host comparison below
    // is already case-insensitive without lowercasing the entire URL string.
    if let Ok(mut parsed) = reqwest::Url::parse(trimmed) {
        if parsed
            .host_str()
            .is_some_and(|h| h == "linkedin.com" || h.ends_with(".linkedin.com"))
        {
            parsed.set_query(None);
            return parsed.to_string();
        }
        // Non-LinkedIn: return the parsed URL (host normalised to lowercase,
        // path + query preserved in original case — case-significant on boards
        // that encode the job id in query params or case-sensitive path segments).
        return parsed.to_string();
    }
    trimmed.to_string()
}

/// Deduplicate the cross-provider merge by URL, preserving first-seen order.
///
/// `dedupe` keys on `external_id`, which is provider-prefixed (`adzuna-…` vs
/// `linkedin-…`) and so never collides across providers even for the SAME job.
/// The additive merge therefore keys on the canonical URL instead, so a posting
/// surfaced by both the primary chain and the LinkedIn provider appears once
/// (primary first, since it is extended onto the front).
///
/// LinkedIn tracking params (`?trk=…`, `?refId=…`) are stripped by
/// [`canonical_url`] before keying so the same logical job dedupes regardless
/// of which tracking variant was captured.
pub(super) fn dedupe_by_url(items: Vec<JobPosting>) -> Vec<JobPosting> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|p| seen.insert(canonical_url(&p.url)))
        .collect()
}

/// Top-level provider orchestration.
///
/// 1. **Primary result** — the Adzuna → JSearch → Jooble fallback chain
///    ([`primary_chain`]), with its existing semantics fully preserved.
/// 2. **Additive LinkedIn (Apify)** — runs IN ADDITION to (never as a fallback of)
///    the primary result, and ONLY when `apify_linkedin` is configured (the toggle
///    is ON and a token is present) AND the search carries upstream spend to give
///    it ([`apify_cap`] > 0 — i.e. never on a scheduled run). Its results are
///    merged onto the primary, deterministically (primary first) and deduped by URL.
///
/// When the LinkedIn provider is absent or not configured — the default, and what
/// the Adzuna/JSearch tests exercise — this returns the primary result byte-for-byte,
/// so all existing fallback + keyless-empty semantics are unchanged.
pub(super) async fn search_with_providers(
    providers: &[Box<dyn JobProvider>],
    query: &str,
    location: &str,
    country: &str,
    country_guessed: bool,
    date_filter: Option<&str>,
    budget: SearchBudget,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    let primary = primary_chain(
        providers,
        query,
        location,
        country,
        country_guessed,
        date_filter,
        // The UPSTREAM budget, never `amount` — see `SearchBudget`.
        budget.provider_amount,
        signal.clone(),
    )
    .await;

    let linkedin = providers
        .iter()
        .find(|p| p.provider_id() == "apify_linkedin");
    let li_configured = linkedin.map(|p| p.is_configured()).unwrap_or(false);

    // Not opted in → identical to the legacy Adzuna→JSearch path (Err and all).
    if !li_configured {
        return primary;
    }

    // Cost gate — see [`apify_cap`]. `0` means "don't buy a run at all", which
    // covers BOTH the no-upstream-budget case (every scheduled run) and a primary
    // result that already satisfies the budget.
    let cap = apify_cap(budget, primary.as_ref().map(|v| v.len()).unwrap_or(0));
    if cap == 0 {
        return primary;
    }

    // Don't fire a paid Apify run after cancellation.
    let li_items = if signal.is_cancelled() {
        Vec::new()
    } else {
        match linkedin
            .expect("li_configured implies the provider is present")
            .search(
                query,
                location,
                country,
                country_guessed,
                date_filter,
                Some(cap),
                signal,
            )
            .await
        {
            Ok(items) => items,
            Err(e) => {
                // Tolerate one provider erroring — log and merge what we have.
                log::warn!("[aggregator] apify_linkedin error (additive, ignored): {e}");
                Vec::new()
            }
        }
    };

    match primary {
        Ok(mut items) => {
            items.extend(li_items);
            Ok(dedupe_by_url(items))
        }
        // Primary failed (e.g. Adzuna unsupported-country diagnostic + no JSearch).
        // Surface the diagnostic only when LinkedIn also produced nothing; if it
        // returned results, prefer showing them over hiding them behind the error.
        Err(e) => {
            if li_items.is_empty() {
                Err(e)
            } else {
                Ok(dedupe_by_url(li_items))
            }
        }
    }
}
