/// Aggregator board — Adzuna (primary) with JSearch (paid fallback).
///
/// Design:
/// * One `Scraper` in the registry (id = `"aggregator"`).
/// * Internally holds an ordered `JobProvider` registry: Adzuna → JSearch.
/// * Fallback semantics (enforced in `search_with_providers`):
///   - Adzuna configured + `Ok(items)` (even empty) → use those, do NOT call JSearch.
///   - Adzuna configured + `Err(_)` → log, try JSearch if configured.
///   - Adzuna not configured → try JSearch if configured.
///   - Neither configured → `Ok(vec![])` (keyless-empty, never an error).
/// * Keys are optional: absent = keyless-empty.  Never hardcoded, never logged.
/// * Keys are read from the OS keychain via `credentials::read_credential`,
///   under the `ai:` keyring namespace + the BARE slot names generated from the
///   cross-language source of truth in `ipc_contracts::provider_slots`
///   (`packages/shared/src/provider-slots.ts`):
///   - `ai:adzuna-app-id`   (`provider_slots::ADZUNA_APP_ID`)  — Adzuna application ID
///   - `ai:adzuna-app-key`  (`provider_slots::ADZUNA_APP_KEY`) — Adzuna application key
///   - `ai:jsearch-key`     (`provider_slots::JSEARCH_KEY`)    — RapidAPI key for JSearch
///   - `ai:apify-token`     (`provider_slots::APIFY_TOKEN`)    — Apify Bearer token
///
/// Rate-limiting and cancellation are honoured: every network call flows
/// through `scraping::http::fetch_json` (which checks `ctx.signal` and calls
/// the per-host `rate_limiter`).
///
/// The three `JobProvider` impls (Adzuna/JSearch/Apify) live in `providers.rs`
/// (split out to stay under the R8 module-size cap); this file holds the shared
/// `JobProvider` trait, the fallback orchestration (`primary_chain` /
/// `search_with_providers`), the credential-state helpers, and the `Scraper` impl.
use async_trait::async_trait;

use crate::scraping::types::{
    AuthRequirement, BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode,
};

mod providers;
use providers::*;

/// Below this many results from a supported market, a non-empty `where` retries
/// once country-wide (see `AdzunaProvider::search` in `providers.rs`). A city
/// geocode can be sparse even when the country has plenty; broadening recovers
/// the full market page. Also gates `primary_chain`'s guessed-market fallback
/// below — shared between both, hence it stays at this level rather than moving
/// into `providers.rs` with the provider implementations.
const ADZUNA_BROADEN_FLOOR: usize = 3;

// ── Provider trait ────────────────────────────────────────────────────────────

/// A single search-API backend.  Object-safe so the scraper can hold a
/// `Vec<Box<dyn JobProvider>>` without generics leaking into the `Scraper` trait.
#[async_trait]
pub(crate) trait JobProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    /// True when the necessary API keys are present in the credential store.
    fn is_configured(&self) -> bool;
    /// Run a search.  Non-2xx or network errors are returned as `Err`.
    ///
    /// `amount` is a provider-specific result cap that callers may pass to
    /// cost-bounded providers (currently Apify only).  Providers that have no
    /// concept of a cap ignore it (`_amount`).
    ///
    /// `country_guessed` is true when the caller supplied no explicit
    /// `country_code` (see `AggregatorScraper::search`). Providers that don't
    /// need to distinguish a real target from a guessed default ignore it
    /// (`_country_guessed`) — currently only `AdzunaProvider` reads it, to keep
    /// its near-empty broaden retry from ever firing on a guessed market (that
    /// would defeat the guessed-market → JSearch fallback in `primary_chain`).
    async fn search(
        &self,
        query: &str,
        location: &str,
        country: &str,
        country_guessed: bool,
        date_filter: Option<&str>,
        amount: Option<u32>,
        signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>>;
}

// ── Fallback logic ────────────────────────────────────────────────────────────

/// Run the provider chain: Adzuna primary, JSearch fallback.
///
/// Fallback rule (spec):
/// - Adzuna configured, `Ok(items)` (even empty) → return those; skip JSearch.
/// - Adzuna configured, `Err(_)`                 → log; try JSearch if configured.
/// - Adzuna not configured                        → try JSearch if configured.
/// - Neither configured                           → `Ok(vec![])` (keyless-empty).
/// - Adzuna configured + `Err(_)`, JSearch absent → `Err(diagnostic)` so the
///   engine surfaces it as a board error rather than a silent zero-result run.
///
/// **Guessed-market exception**: when the caller supplied no `country_code`
/// (`country_guessed`), `AggregatorScraper::search` defaulted `country` to `"de"`
/// as a GUESS, not a real target — a common autopilot shape (a prefilled/typed
/// location with no geocode pick). If that guess comes back `Ok(empty)` for a
/// real (non-empty) `location`, the location is very likely NOT in Germany, so
/// trusting the sparse guess would silently zero out (or under-fill) an otherwise-
/// findable search (the autopilot aggregator zero-jobs bug). Treat it like an
/// Adzuna error instead: fall through to JSearch (global, free-text location).
///
/// - When a fallback IS configured it wins (the sparse Adzuna hits are dropped).
/// - When NO fallback is configured (or it also fails), any real (non-empty)
///   sparse hits are RETURNED as a last resort — a user with only Adzuna keys
///   keeps their few legit results instead of losing them to a diagnostic error;
///   an EMPTY guessed-market result still surfaces the diagnostic (nothing to
///   salvage, and a silent zero is the bug we guard).
///
/// A guessed country with NO location (the keyless/no-location default) is
/// unaffected — `Ok(empty)` there returns as before.
///
/// Items from each provider are keyed by their `external_id` to deduplicate.
async fn primary_chain(
    providers: &[Box<dyn JobProvider>],
    query: &str,
    location: &str,
    country: &str,
    country_guessed: bool,
    date_filter: Option<&str>,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    if signal.is_cancelled() {
        return Ok(vec![]);
    }

    // Locate primary (Adzuna) and fallback (JSearch) by id.
    let primary = providers.iter().find(|p| p.provider_id() == "adzuna");
    let fallback = providers.iter().find(|p| p.provider_id() == "jsearch");

    // Track whether Adzuna was configured-but-failed so we can distinguish
    // "keys present, request failed" from "no keys at all" at the end.
    let mut adzuna_configured_failed: Option<anyhow::Error> = None;

    // Real (non-empty) but SPARSE items from a GUESSED market. We distrust them
    // enough to prefer a fallback, but if there is no working fallback they beat
    // discarding legit hits for a zero-results error — so we retain and return them
    // as a last resort. An EMPTY guessed-market result is NOT salvaged here (there
    // is nothing to return, and a silent zero is the exact autopilot bug the
    // diagnostic guards) — it keeps the diagnostic-Err path via `adzuna_configured_failed`.
    let mut sparse_guessed_items: Option<Vec<JobPosting>> = None;

    // Run primary if configured.
    if let Some(p) = primary {
        if p.is_configured() {
            match p
                .search(
                    query,
                    location,
                    country,
                    country_guessed,
                    date_filter,
                    None,
                    signal.clone(),
                )
                .await
            {
                Ok(items)
                    if items.len() < ADZUNA_BROADEN_FLOOR
                        && country_guessed
                        && !location.is_empty() =>
                {
                    // Guessed-market guard (see doc comment above): a SPARSE result —
                    // fewer than the broaden floor — from a GUESSED country with a real
                    // location is untrustworthy. "London" defaulting to "de" returns
                    // either nothing or a handful of stray German hits; neither should
                    // be trusted as authoritative. Treat it like an Adzuna error and
                    // fall through below (to JSearch, which is global + free-text)
                    // instead of returning those few results as the whole answer.
                    //
                    // PRIVACY: never log/persist the raw user-entered `location` —
                    // it's free-text PII, not something this repo puts in logs or
                    // diagnostics. `country` (the guessed market code) is fine.
                    log::warn!(
                        "[aggregator] adzuna guessed market '{country}' returned too few \
                         results ({}) for the supplied location (no country_code supplied); \
                         attempting jsearch fallback",
                        items.len()
                    );
                    adzuna_configured_failed = Some(anyhow::anyhow!(
                        "adzuna: guessed market '{country}' returned too few results ({}) for \
                         the supplied location (no country was supplied)",
                        items.len()
                    ));
                    // Retain the sparse-but-real hits so that, absent a working
                    // fallback, we return them rather than discarding legit results
                    // for a zero-results error. Empty stays unsalvaged (see the
                    // `sparse_guessed_items` doc) and keeps the diagnostic-Err path.
                    if !items.is_empty() {
                        sparse_guessed_items = Some(items);
                    }
                    // Fall through to JSearch/diagnostic below.
                }
                Ok(items) => {
                    // Real country (or no location to doubt the guess with), even
                    // empty → use result as-is; do NOT fall through to JSearch.
                    return Ok(dedupe(items));
                }
                Err(e) => {
                    log::warn!("[aggregator] adzuna error, attempting jsearch fallback: {e}");
                    adzuna_configured_failed = Some(e);
                    // Fall through to JSearch below.
                }
            }
        }
    }

    // Guard: don't fire a paid JSearch call after cancellation.
    if signal.is_cancelled() {
        return Ok(vec![]);
    }

    // Try fallback.
    if let Some(f) = fallback {
        if f.is_configured() {
            match f
                .search(
                    query,
                    location,
                    country,
                    country_guessed,
                    date_filter,
                    None,
                    signal,
                )
                .await
            {
                Ok(items) => return Ok(dedupe(items)),
                Err(e) => {
                    // Fallback itself failed. If we retained sparse-but-real Adzuna
                    // items from a guessed market, return those (better than nothing)
                    // rather than surfacing the fallback error and losing legit hits.
                    if let Some(items) = sparse_guessed_items.take() {
                        log::warn!(
                            "[aggregator] jsearch fallback failed ({e}); returning {} \
                             sparse guessed-market adzuna result(s) instead",
                            items.len()
                        );
                        return Ok(dedupe(items));
                    }
                    return Err(e);
                }
            }
        }
    }

    // No working fallback fired. If we retained sparse guessed-market items, return
    // them rather than the diagnostic — a user with only Adzuna keys keeps their few
    // legit hits (better than nothing). The uncertainty was already logged above.
    //
    // NOTE (deliberately under-surfaced, PR D): this salvage path — and the sibling
    // one at the `sparse_guessed_items.take()` above — return the sparse guessed
    // hits WITHOUT a `BoardScrapeSummary.note`, because only `primary_chain` (not
    // `AdzunaProvider`, which holds the note sink) knows the guess was salvaged
    // rather than authoritative or replaced. Revisit when PR F threads location
    // context through this path.
    if let Some(items) = sparse_guessed_items {
        return Ok(dedupe(items));
    }

    // Adzuna had keys but failed (e.g. unsupported country, or an EMPTY guessed
    // market) AND JSearch is not configured → surface a diagnostic error instead
    // of a silent empty result. The engine records this in BoardScrapeSummary.error,
    // which the Jobs page renders as a partial-failure warning and autopilot logs
    // as a skipped board.
    if let Some(e) = adzuna_configured_failed {
        return Err(anyhow::anyhow!(
            "{e}; add a JSearch key in Settings → API Keys for global coverage"
        ));
    }

    // Neither provider configured → keyless-empty (intended, never an error).
    Ok(vec![])
}

/// Deduplicate by `external_id`, preserving first-seen order.
fn dedupe(items: Vec<JobPosting>) -> Vec<JobPosting> {
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
fn canonical_url(url: &str) -> String {
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
fn dedupe_by_url(items: Vec<JobPosting>) -> Vec<JobPosting> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|p| seen.insert(canonical_url(&p.url)))
        .collect()
}

/// Top-level provider orchestration.
///
/// 1. **Primary result** — the Adzuna → JSearch fallback chain ([`primary_chain`]),
///    with its existing semantics fully preserved.
/// 2. **Additive LinkedIn (Apify)** — runs IN ADDITION to (never as a fallback of)
///    the primary result, and ONLY when `apify_linkedin` is configured (the toggle
///    is ON and a token is present). Its results are merged onto the primary,
///    deterministically (primary first) and deduped by URL.
///
/// When the LinkedIn provider is absent or not configured — the default, and what
/// the Adzuna/JSearch tests exercise — this returns the primary result byte-for-byte,
/// so all existing fallback + keyless-empty semantics are unchanged.
async fn search_with_providers(
    providers: &[Box<dyn JobProvider>],
    query: &str,
    location: &str,
    country: &str,
    country_guessed: bool,
    date_filter: Option<&str>,
    amount: usize,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    let primary = primary_chain(
        providers,
        query,
        location,
        country,
        country_guessed,
        date_filter,
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

    // Cost gate: skip the paid Apify call when the primary result already
    // satisfies the requested amount — LinkedIn is a fill for UNMET capacity,
    // not unconditional.
    if let Ok(ref items) = primary {
        if items.len() >= amount {
            return primary;
        }
    }

    // Don't fire a paid Apify run after cancellation.
    // Cap: only fetch as many results as still needed; never exceed APIFY_MAX_ITEMS.
    let primary_len = primary.as_ref().map(|v| v.len()).unwrap_or(0);
    let remaining = amount.saturating_sub(primary_len);
    let apify_cap = remaining.min(APIFY_MAX_ITEMS as usize) as u32;

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
                Some(apify_cap),
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
        || ApifyLinkedInProvider::new().is_configured()
}

/// First keyring READ error across the aggregator's provider credential slots
/// (Adzuna id/key, JSearch key, and the Apify token), if any. Probes the SAME slot
/// set [`aggregator_has_configured_provider`] counts — including the Apify token —
/// so a faulting Apify slot (with Adzuna/JSearch merely absent) is classified as a
/// store error rather than a misleading `needs-keys` skip. Distinguishes a
/// credential-store FAULT (surfaced as a board error) from mere key absence
/// (surfaced as a `needs-keys` skip). Returns `None` when every slot reads
/// cleanly — whether the key is present or simply absent.
///
/// The error string is a keyring backend message + slot name only; it never
/// carries a credential value (see `credentials::read_credential`).
fn aggregator_store_error() -> Option<String> {
    use crate::ipc_contracts::provider_slots::{
        ADZUNA_APP_ID, ADZUNA_APP_KEY, APIFY_TOKEN, JSEARCH_KEY,
    };
    for slot in [ADZUNA_APP_ID, ADZUNA_APP_KEY, JSEARCH_KEY, APIFY_TOKEN] {
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
            // Additive, opt-in, paid: only runs when the toggle is ON and a token
            // is present (gated in `ApifyLinkedInProvider::is_configured`).
            Box::new(ApifyLinkedInProvider::new()),
        ];
        let amount = input.amount as usize;
        let items = search_with_providers(
            &providers,
            query,
            location,
            &country,
            country_guessed,
            input.date_filter.as_deref(),
            amount,
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
mod test;
