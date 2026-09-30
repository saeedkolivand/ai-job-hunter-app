/// Aggregator board — Adzuna (primary) → JSearch (paid fallback) → Jooble
/// (last-resort fallback).
///
/// Design:
/// * One `Scraper` in the registry (id = `"aggregator"`).
/// * Internally holds an ordered `JobProvider` registry: Adzuna → JSearch →
///   Jooble.
/// * Fallback semantics (enforced in `primary_chain`):
///   - Adzuna configured + `Ok(items)` (even empty) → use those, do NOT call JSearch/Jooble.
///   - Adzuna configured + `Err(_)` → log, try JSearch if configured.
///   - Adzuna not configured → try JSearch if configured.
///   - JSearch configured + `Ok(items)` (even empty) → use those, do NOT call Jooble.
///   - JSearch configured + `Err(_)`, or not configured → try Jooble if configured
///     (Jooble is a LAST-RESORT tier — it only fires once both Adzuna and JSearch
///     have failed to produce a decisive result, e.g. the unsupported-country case,
///     never on a routine "genuinely zero results" search, since its rate limit is
///     undocumented).
///   - Nothing left → `Ok(vec![])`.
/// * **Every provider here is key-backed**, so with no keys at all this board is
///   skipped with `needs-keys` rather than run. freehire used to sit under the
///   keyed tiers as an always-on keyless floor, which is what made that skip
///   unreachable; it is now its own catalog board
///   (`scraping::boards::freehire`), chosen explicitly, so the skip means what
///   it says again. Keys are never hardcoded, never logged.
/// * Keys are read from the OS keychain via `credentials::read_credential`,
///   under the `ai:` keyring namespace + the BARE slot names generated from the
///   cross-language source of truth in `ipc_contracts::provider_slots`
///   (`packages/shared/src/provider-slots.ts`):
///   - `ai:adzuna-app-id`   (`provider_slots::ADZUNA_APP_ID`)  — Adzuna application ID
///   - `ai:adzuna-app-key`  (`provider_slots::ADZUNA_APP_KEY`) — Adzuna application key
///   - `ai:jsearch-key`     (`provider_slots::JSEARCH_KEY`)    — RapidAPI key for JSearch
///   - `ai:jooble-key`      (`provider_slots::JOOBLE_KEY`)     — Jooble API key
///   - `ai:apify-token`     (`provider_slots::APIFY_TOKEN`)    — Apify Bearer token
///
/// Rate-limiting and cancellation are honoured: every network call flows
/// through `scraping::http::fetch_json` (which checks `ctx.signal` and calls
/// the per-host `rate_limiter`).
///
/// The `JobProvider` impls live in sibling files, split out to stay under the
/// R8 module-size cap: the PRIMARY tier (Adzuna) in `adzuna.rs` (the stateful
/// provider) + `adzuna_fetch.rs` (market/response-mapping/page-fetch
/// plumbing); the remaining tiers in `jsearch.rs` / `jooble.rs` /
/// `apify.rs` + `apify_settings.rs` (`serde_helpers.rs` is a small shared
/// serde helper the last two share). `budget.rs` holds the per-search spend
/// budget, `fallback.rs` the `JobProvider` trait + the Adzuna→JSearch→Jooble
/// chain, and `merge.rs` the cross-provider dedup + the additive
/// Apify-LinkedIn merge on top of it. This file holds the credential-state
/// helpers and the `Scraper` impl.
use async_trait::async_trait;

use crate::scraping::types::{
    AuthRequirement, BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode,
};

mod adzuna;
mod adzuna_fetch;
mod apify;
mod apify_settings;
mod budget;
mod fallback;
mod jooble;
mod jsearch;
mod merge;
mod serde_helpers;

use adzuna::*;
use apify::*;
use budget::*;
use fallback::*;
use jooble::*;
use jsearch::*;
use merge::*;
// `adzuna_fetch` / `apify_settings` items are consumed at the `aggregator`
// level only by `tests/*` (production code reaches them via each other
// sibling's own direct `super::adzuna_fetch::`/`super::apify_settings::`
// import) — the re-export is dead weight outside `#[cfg(test)]`.
#[cfg(test)]
use adzuna_fetch::*;
#[cfg(test)]
use apify_settings::*;

// ── Credential-state helpers (needs-keys skip vs. store-error) ──────────────────

/// Whether at least one aggregator provider is fully configured. Constructs the
/// providers fresh — the same keyring read the search path does — so a key added
/// in Settings clears any `needs-keys` skip on the next run. Apify counts only
/// when its opt-in toggle AND token are both present (its own `is_configured`).
///
/// Provider construction swallows a keyring READ FAILURE to an unconfigured
/// state; that fault is classified separately by [`aggregator_store_error`] so it
/// surfaces as a board error rather than a misleading `needs-keys` skip.
fn aggregator_has_configured_provider() -> bool {
    AdzunaProvider::new().is_configured()
        || JSearchProvider::new().is_configured()
        || JoobleProvider::new().is_configured()
        || ApifyLinkedInProvider::new().is_configured()
}

/// First keyring READ error across the aggregator's provider credential slots
/// (Adzuna id/key, JSearch key, Jooble key, and the Apify token), if any. Probes
/// the SAME slot set [`aggregator_has_configured_provider`] counts — including
/// Jooble and the Apify token — so a faulting slot (with the others merely
/// absent) is classified as a store error rather than a misleading `needs-keys`
/// skip. Distinguishes a credential-store FAULT (surfaced as a board error) from
/// mere key absence (surfaced as a `needs-keys` skip). Returns `None` when every
/// slot reads cleanly — whether the key is present or simply absent.
///
/// The error string is a keyring backend message + slot name only; it never
/// carries a credential value (see `credentials::read_credential`).
fn aggregator_store_error() -> Option<String> {
    use crate::ipc_contracts::provider_slots::{
        ADZUNA_APP_ID, ADZUNA_APP_KEY, APIFY_TOKEN, JOOBLE_KEY, JSEARCH_KEY,
    };
    for slot in [
        ADZUNA_APP_ID,
        ADZUNA_APP_KEY,
        JSEARCH_KEY,
        JOOBLE_KEY,
        APIFY_TOKEN,
    ] {
        if let Err(e) = crate::credentials::read_credential(&format!("ai:{slot}")) {
            return Some(e.to_string());
        }
    }
    None
}

// ── Scraper impl ──────────────────────────────────────────────────────────────

/// This board's `Scraper::id()` / `JobPosting.source` value. Exposed as a
/// crate-visible constant (rather than the bare `"aggregator"` literal
/// duplicated at each call site) so a caller that needs to recognise an
/// aggregator-sourced posting — e.g. `commands::autopilot`'s snippet-score
/// provisional-flag check — references this single source of truth instead of
/// a string that could silently drift out of lockstep with `id()`.
pub(crate) const AGGREGATOR_BOARD_ID: &str = "aggregator";

pub struct AggregatorScraper;

#[async_trait]
impl Scraper for AggregatorScraper {
    fn id(&self) -> &'static str {
        AGGREGATOR_BOARD_ID
    }

    fn display_name(&self) -> &'static str {
        "Aggregated Jobs"
    }

    fn mode(&self) -> ScraperMode {
        ScraperMode::Http
    }

    fn auth(&self) -> AuthRequirement {
        // Keys are optional config, not a login requirement.
        AuthRequirement::Guest
    }

    fn requires_company(&self) -> bool {
        false
    }

    fn needs_keys(&self) -> bool {
        // Skip with "needs-keys" only when the store reads cleanly but has no
        // usable provider keys. A store READ FAILURE is NOT a skip — `search`
        // surfaces it as a board error instead — so short-circuit on that case.
        aggregator_store_error().is_none() && !aggregator_has_configured_provider()
    }

    fn supports_location(&self) -> bool {
        // Adzuna/JSearch consume the location server-side: `country_code` routes the
        // market directly and the free-text `location` is the `where`/query param.
        true
    }

    async fn search(
        &self,
        input: BoardSearchInput,
        ctx: ScrapeContext,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // Surface a credential-store FAILURE (keyring unavailable) as a real board
        // error rather than the silent keyless-empty a missing key produces. Mere
        // key absence is handled upstream by the engine's `needs-keys` skip, so by
        // the time `search` runs the only credential fault left to report is a
        // store read error.
        if let Some(msg) = aggregator_store_error() {
            return Err(anyhow::anyhow!(
                "aggregator: credential store unavailable ({msg})"
            ));
        }

        let query = input.query.trim();
        let location = input.location.as_deref().unwrap_or("").trim();
        // Whether the caller supplied a real `country_code`, vs us GUESSING "de"
        // below. `primary_chain`'s guessed-market guard uses this to distinguish
        // a genuine German search from a scrape that never had a country to
        // begin with (e.g. an autopilot target saved without a geocode pick).
        let country_guessed = input.country_code.is_none();
        let country = input
            .country_code
            .as_deref()
            .map(str::to_lowercase)
            .unwrap_or_else(|| "de".to_string());

        // Construct providers fresh per call so that key changes made in Settings
        // take effect immediately without requiring an app restart. Adzuna carries
        // the location-policy note sink so its guessed-market / broadening decisions
        // surface as `BoardScrapeSummary.note` (see `AdzunaProvider::report_note`).
        let providers: Vec<Box<dyn JobProvider>> = vec![
            Box::new(AdzunaProvider::new().with_note_sink(ctx.on_note.clone())),
            Box::new(JSearchProvider::new()),
            // Last-resort fallback (see `primary_chain`'s doc comment) — only
            // reached once both Adzuna and JSearch fail to produce a result.
            Box::new(JoobleProvider::new()),
            // Additive, opt-in, paid: only runs when the toggle is ON and a token
            // is present (gated in `ApifyLinkedInProvider::is_configured`).
            Box::new(ApifyLinkedInProvider::new()),
        ];
        // `amount` caps the OUTPUT; `provider_amount` is the only thing that buys
        // upstream calls — the free tiers' page budgets AND the paid Apify tier
        // (`apify_cap`) both read it, and only it. A scheduled run leaves it
        // `None`, so the run costs exactly one Adzuna request and zero paid
        // Apify runs, regardless of the 100 it passes as `amount`.
        // The mapping lives in `from_input` so it is directly testable.
        let budget = SearchBudget::from_input(&input);
        let amount = budget.amount;
        let items = search_with_providers(
            &providers,
            query,
            location,
            &country,
            country_guessed,
            input.date_filter.as_deref(),
            budget,
            ctx.signal.clone(),
        )
        .await?;
        let mut out = Vec::new();

        for posting in items.into_iter().take(amount) {
            if ctx.signal.is_cancelled() {
                break;
            }
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
mod tests;
