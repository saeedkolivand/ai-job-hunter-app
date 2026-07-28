/// Aggregator board — Adzuna (primary) → JSearch (paid fallback) → Jooble
/// (last-resort fallback).
///
/// Design:
/// * One `Scraper` in the registry (id = `"aggregator"`).
/// * Internally holds an ordered `JobProvider` registry: Adzuna → JSearch → Jooble.
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
///   - None of the three configured → `Ok(vec![])` (keyless-empty, never an error).
/// * Keys are optional: absent = keyless-empty.  Never hardcoded, never logged.
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
/// The `JobProvider` impls live next door, split out to stay under the R8
/// module-size cap: the PRIMARY tier (Adzuna — market allowlist, page loop,
/// broaden retry) in `adzuna.rs`, the remaining tiers (JSearch/Jooble/Apify) in
/// `providers.rs`. This file holds the shared `JobProvider` trait, the fallback
/// orchestration (`primary_chain` / `search_with_providers`), the
/// credential-state helpers, and the `Scraper` impl.
use async_trait::async_trait;

use crate::scraping::types::{
    AuthRequirement, BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode,
};

mod adzuna;
mod providers;
use adzuna::*;
use providers::*;

/// Below this many results from a supported market, a non-empty `where` retries
/// once country-wide (see `AdzunaProvider::search` in `providers.rs`). A city
/// geocode can be sparse even when the country has plenty; broadening recovers
/// the full market page. Also gates `primary_chain`'s guessed-market fallback
/// below — shared between both, hence it stays at this level rather than moving
/// into `providers.rs` with the provider implementations.
const ADZUNA_BROADEN_FLOOR: usize = 3;

// ── Search budgets ────────────────────────────────────────────────────────────

/// The two INDEPENDENT budgets one aggregator search carries. They are separate
/// fields (not one number) because conflating them is a live cost bug: `amount`
/// is a sentinel ceiling for callers with no item-count intent, so spending
/// upstream calls against it silently multiplies every scheduled run's bill.
///
/// Passed as one struct rather than two parameters so the provider-orchestration
/// fns stay within the crate's 8-argument clippy ceiling (`clippy.toml`).
#[derive(Clone, Copy)]
pub(crate) struct SearchBudget {
    /// OUTPUT cap — how many postings the caller will keep, and NOTHING else.
    /// It buys no upstream call on any tier (paid ones included): it is a
    /// "don't cap me" sentinel on the scheduled path, so spending against it is
    /// precisely the cost bug this type exists to prevent.
    amount: usize,
    /// UPSTREAM SPEND target — see `BoardSearchInput::provider_amount`. The ONLY
    /// field that buys upstream calls. `None` (the default, and every scheduled
    /// run) keeps each metered provider at its cheapest single-request form and
    /// skips the paid LinkedIn tier entirely (see [`apify_cap`]).
    provider_amount: Option<u32>,
}

impl SearchBudget {
    /// The ONLY production translation from a request to a spend budget, and the
    /// single line where the cost bug this type exists to prevent could reappear.
    /// Deliberately not inlined at the call site: hand-built `SearchBudget`s in
    /// tests cannot observe this hop, so a guard that constructs one directly
    /// would keep passing even if this mapping were rewritten to spend `amount`.
    /// `budget_from_autopilot_shaped_input_has_no_provider_spend` pins it.
    ///
    /// `amount` becomes the output cap verbatim; the upstream budget is passed
    /// through UNCHANGED (`None` stays `None`) — it is never derived from
    /// `amount` or `pages`, both of which are sentinels on one caller or the
    /// other (see `BoardSearchInput`).
    fn from_input(input: &BoardSearchInput) -> Self {
        Self {
            amount: input.amount as usize,
            provider_amount: input.provider_amount,
        }
    }

    /// Test-only constructor. Production goes through [`Self::from_input`], so
    /// this cannot be the thing a mutation of the real mapping slips past.
    #[cfg(test)]
    pub(crate) fn new(amount: usize, provider_amount: Option<u32>) -> Self {
        Self {
            amount,
            provider_amount,
        }
    }

    /// Output cap only, no upstream spend target — the quota-neutral shape a
    /// scheduled run uses, and the default for tests that only exercise the
    /// fallback chain with fake providers.
    #[cfg(test)]
    pub(crate) fn items_only(amount: usize) -> Self {
        Self::new(amount, None)
    }

    /// MANUAL-search shape: one user-typed count that is both the output cap and
    /// the upstream spend target (exactly what `commands::scrape` builds). The
    /// shape any PAID-tier test needs — with `items_only` the aggregator buys
    /// nothing upstream, so an Apify assertion would pass vacuously.
    #[cfg(test)]
    pub(crate) fn manual(amount: usize) -> Self {
        Self::new(amount, Some(amount as u32))
    }
}

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
    /// `amount` is the caller's UPSTREAM SPEND target (`SearchBudget::provider_amount`
    /// — NOT `BoardSearchInput::amount`, which is a sentinel-prone output cap).
    /// Providers that can bound their own spend by it use it: Adzuna pages by it
    /// (`adzuna_page_budget`), JSearch maps it to `num_pages`
    /// (`jsearch_num_pages`), and Apify uses it as a hard `maxItems` cap.
    /// Providers with no such knob ignore it (`_amount`). `None` means "no
    /// target" and every provider degrades to its cheapest single-request form —
    /// the default, and what every scheduled (autopilot) run passes.
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
///
/// **Jooble (last-resort tier)**: tried only when JSearch ALSO fails to produce
/// a decisive result (unconfigured or `Err`) — i.e. only in the terminal branches
/// this function would otherwise resolve to a diagnostic `Err` or keyless-empty.
/// A JSearch `Ok(items)` (even empty) still short-circuits before Jooble is ever
/// reached, symmetric with Adzuna's own "configured Ok, even empty, wins" rule —
/// this keeps Jooble off the hot path for a routine zero-results search (its rate
/// limit is undocumented) and reserves it for genuinely unmet capacity.
async fn primary_chain(
    providers: &[Box<dyn JobProvider>],
    query: &str,
    location: &str,
    country: &str,
    country_guessed: bool,
    date_filter: Option<&str>,
    amount: Option<u32>,
    signal: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Vec<JobPosting>> {
    if signal.is_cancelled() {
        return Ok(vec![]);
    }

    // Locate primary (Adzuna), fallback (JSearch), and the last-resort tier
    // (Jooble) by id.
    let primary = providers.iter().find(|p| p.provider_id() == "adzuna");
    let fallback = providers.iter().find(|p| p.provider_id() == "jsearch");
    let jooble = providers.iter().find(|p| p.provider_id() == "jooble");

    // Track whether each provider was CONFIGURED but its call FAILED, so we can
    // distinguish "keys present, request failed" from "no keys at all" at the
    // end. All three are collected (not just the first) — see the total-failure
    // resolution below: a LATER-tier provider's failure must never be silently
    // dropped just because an EARLIER-tier provider also failed.
    let mut adzuna_configured_failed: Option<anyhow::Error> = None;
    let mut jsearch_configured_failed: Option<anyhow::Error> = None;
    // Jooble's tracking closes the same silent-empty-failure gap Adzuna/JSearch
    // already guard: without it, a user with ONLY a Jooble key whose Jooble call
    // fails would get a silent `Ok(empty)` ("no jobs found") instead of an
    // honest error.
    let mut jooble_configured_failed: Option<anyhow::Error> = None;

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
                    amount,
                    signal.clone(),
                )
                .await
            {
                Ok(items)
                    if items.len() < ADZUNA_BROADEN_FLOOR
                        && country_guessed
                        && !location.is_empty()
                        // A user's Stop mid-search also comes back short (the
                        // page loop returns what it had collected — see
                        // `fetch_adzuna_pages`). That is a deliberate stop, not
                        // a market the guess got wrong, so it must not log a
                        // "too few results, attempting jsearch fallback"
                        // diagnostic about a fallback the cancel guard below
                        // will never let run.
                        && !signal.is_cancelled() =>
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

    // Try JSearch fallback.
    if let Some(f) = fallback {
        if f.is_configured() {
            match f
                .search(
                    query,
                    location,
                    country,
                    country_guessed,
                    date_filter,
                    amount,
                    signal.clone(),
                )
                .await
            {
                Ok(items) => return Ok(dedupe(items)),
                Err(e) => {
                    // JSearch itself failed. Don't salvage/return yet — Jooble (the
                    // last-resort tier, below) still gets a chance at a decisive
                    // result before falling back to the sparse-guessed salvage or
                    // the diagnostic error.
                    log::warn!("[aggregator] jsearch error, attempting jooble fallback: {e}");
                    jsearch_configured_failed = Some(e);
                }
            }
        }
    }

    // Guard: don't fire a paid/rate-limited Jooble call after cancellation.
    if signal.is_cancelled() {
        return Ok(vec![]);
    }

    // Try Jooble — LAST-RESORT tier (see the doc comment above `primary_chain`).
    if let Some(j) = jooble {
        if j.is_configured() {
            match j
                .search(
                    query,
                    location,
                    country,
                    country_guessed,
                    date_filter,
                    // Deliberately NOT `amount`: Jooble's knob is `ResultOnPage`
                    // (page SIZE, not a page count) and its rate limit is
                    // undocumented, so raising it is a separate decision from the
                    // amount-bounded page loop. Unchanged from before that loop.
                    None,
                    signal,
                )
                .await
            {
                Ok(items) => return Ok(dedupe(items)),
                Err(e) => {
                    log::warn!("[aggregator] jooble fallback failed: {e}");
                    jooble_configured_failed = Some(e);
                    // Fall through to the sparse-guessed salvage / diagnostic below,
                    // same as a JSearch failure would without Jooble configured.
                }
            }
        }
    }

    // No working fallback fired. If we retained sparse guessed-market items, return
    // them rather than the diagnostic — a user with only Adzuna keys keeps their few
    // legit hits (better than nothing). The uncertainty was already logged above.
    //
    // NOTE (deliberately under-surfaced, PR D): this salvage path returns the sparse
    // guessed hits WITHOUT a `BoardScrapeSummary.note`, because only `primary_chain`
    // (not `AdzunaProvider`, which holds the note sink) knows the guess was salvaged
    // rather than authoritative or replaced. Revisit when PR F threads location
    // context through this path.
    if let Some(items) = sparse_guessed_items {
        return Ok(dedupe(items));
    }

    // Total-failure resolution: gather every CONFIGURED-and-failed provider's
    // error (tier order — adzuna, jsearch, jooble), not just the first. Each
    // provider's error message is already self-prefixed (`"adzuna: …"` /
    // `"jsearch: …"` / `"jooble: …"`) at its own call site, so combining them
    // (when more than one failed) names every failing provider instead of
    // surfacing only the first and silently dropping the rest — e.g. Adzuna AND
    // Jooble both configured+failing with JSearch unconfigured must name BOTH,
    // not just Adzuna's (with a now-stale "add a JSearch key" nudge on top).
    // The engine records the result in BoardScrapeSummary.error, which the Jobs
    // page renders as a partial-failure warning and autopilot logs as a skipped
    // board.
    let failures: Vec<(&'static str, anyhow::Error)> = [
        adzuna_configured_failed.map(|e| ("adzuna", e)),
        jsearch_configured_failed.map(|e| ("jsearch", e)),
        jooble_configured_failed.map(|e| ("jooble", e)),
    ]
    .into_iter()
    .flatten()
    .collect();

    if failures.len() == 1 {
        // Solo failure — the pre-multi-failure per-provider contract is
        // unchanged: JSearch-alone or Jooble-alone returns its own message
        // verbatim (no suffix). Adzuna-alone keeps its "add a fallback" nudge —
        // reaching here with Adzuna as the ONLY failure means neither JSearch
        // nor Jooble was ever configured (a configured provider either
        // succeeds — an early return above — or fails, landing in `failures`),
        // so the nudge is still accurate; the wording now names both fallback
        // options instead of only JSearch.
        let (source, e) = failures
            .into_iter()
            .next()
            .expect("len == 1, checked above");
        return Err(if source == "adzuna" {
            anyhow::anyhow!(
                "{e}; add a JSearch or Jooble key in Settings → API Keys for global coverage"
            )
        } else {
            e
        });
    } else if !failures.is_empty() {
        // Two or more configured providers failed — combine so every one is
        // named. No suffix: the combined message already names what failed.
        let combined = failures
            .into_iter()
            .map(|(_, e)| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(anyhow::anyhow!("{combined}"));
    }

    // None of the providers configured → keyless-empty (intended, never an error).
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

/// How many items the paid LinkedIn (Apify) tier may be asked for — `0` meaning
/// "do not call it at all". The single place the paid tier's spend is decided,
/// pulled out of [`search_with_providers`] so the cost contract is assertable
/// without a network call.
///
/// It rides `provider_amount`, NEVER `amount`:
/// * `provider_amount: None` → `0`. Apify bills per dataset result, so it is an
///   UPSTREAM SPEND, and `amount` is a "don't cap me" sentinel on the scheduled
///   path (`amount: 100`, no item-count intent). Gating a paid run on that
///   sentinel meant every scheduled run bought a full 50-item actor run forever —
///   the exact class of bug `SearchBudget` exists to prevent.
/// * `Some(spend)` → the still-UNMET part of that budget (`spend - primary_len`),
///   clamped to [`APIFY_MAX_ITEMS`]. A primary result that already meets the
///   budget yields `0`, so LinkedIn stays a fill for unmet capacity.
///
/// BEHAVIOR CHANGE (deliberate, PR #896 review): an Apify-enabled user's
/// SCHEDULED runs no longer buy LinkedIn results — only manual searches, which
/// are the ones carrying a real, user-typed spend target. The opt-in toggle
/// still gates whether Apify may run at all (`is_configured`); this decides
/// whether the current search has money to spend on it.
fn apify_cap(budget: SearchBudget, primary_len: usize) -> u32 {
    let Some(spend) = budget.provider_amount else {
        return 0;
    };
    (spend as usize)
        .saturating_sub(primary_len)
        .min(APIFY_MAX_ITEMS as usize) as u32
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
async fn search_with_providers(
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
mod test;
