//! The `JobProvider` trait and the Adzuna → JSearch → Jooble fallback chain
//! (`primary_chain`) — split out of `mod.rs` (R8 module-size guard).
//! `merge.rs` holds the dedup + additive Apify-LinkedIn merge that
//! `search_with_providers` layers on top of this.
use async_trait::async_trait;

use crate::scraping::types::JobPosting;

use super::merge::dedupe;

/// Below this many results from a supported market, a non-empty `where` retries
/// once country-wide (see `AdzunaProvider::search` in `adzuna.rs`). A city
/// geocode can be sparse even when the country has plenty; broadening recovers
/// the full market page. Also gates `primary_chain`'s guessed-market fallback
/// below — shared between both, hence it stays at this level rather than moving
/// into the provider modules with the provider implementations.
pub(super) const ADZUNA_BROADEN_FLOOR: usize = 3;

// ── Provider trait ────────────────────────────────────────────────────────────

/// A single search-API backend.  Object-safe so the scraper can hold a
/// `Vec<Box<dyn JobProvider>>` without generics leaking into the `Scraper` trait.
#[async_trait]
pub(super) trait JobProvider: Send + Sync {
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
pub(super) async fn primary_chain(
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
                    return Ok(super::merge::dedupe(items));
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
                    signal.clone(),
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
    // guessed hits WITHOUT a `BoardScrapeSummary.notes` entry, because only `primary_chain`
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
