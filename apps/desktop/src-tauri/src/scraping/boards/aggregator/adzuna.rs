//! The Adzuna `JobProvider` — the aggregator's PRIMARY tier. Wraps
//! `adzuna_fetch`'s amount-bounded page loop with the near-empty
//! country-wide broaden retry and the guessed-market policy note. Split out
//! of `providers.rs` (R8 module-size guard); `adzuna_fetch.rs` holds the pure
//! market/response-mapping/page-fetch plumbing this orchestrates.
//!
//! Visibility: items here are `pub(super)` (visible to `aggregator` and its
//! descendants, including `tests`) rather than fully private — no API surface
//! beyond `aggregator` is intended.
use std::sync::Arc;

use async_trait::async_trait;

use crate::observability::sanitize_reason;
use crate::scraping::types::JobPosting;

use super::adzuna_fetch::{
    adzuna_supports_country, adzuna_where, fetch_adzuna_page, fetch_adzuna_pages,
    guessed_market_note, should_broaden, AdzunaPageRequest, ADZUNA_BASE_URL,
    ADZUNA_SUPPORTED_COUNTRIES,
};
use super::JobProvider;

pub(crate) struct AdzunaProvider {
    pub(super) app_id: Option<String>,
    pub(super) app_key: Option<String>,
    /// Optional side-channel for user-facing location-policy notes (guessed
    /// market, sparse city → country-wide broadening). Injected by
    /// `AggregatorScraper::search` from the `ScrapeContext`; `None` in unit tests
    /// and credential-state probes. `Arc` (Send + Sync) so the provider stays
    /// `Sync` while it holds the sink across `.await`.
    pub(super) note_sink: Option<Arc<dyn Fn(String) + Send + Sync>>,
    /// API host. [`ADZUNA_BASE_URL`] in production ([`Self::new`]); tests point it
    /// at a local `wiremock` server via [`Self::with_base_url`] so the POLICY that
    /// runs on top of the page loop — the guessed-market note and the near-empty
    /// broaden retry, both of which read the loop's post-dedup count — is testable
    /// through `search` itself rather than only through the fetchers underneath it.
    pub(super) base_url: String,
}

impl AdzunaProvider {
    pub(super) fn new() -> Self {
        use crate::ipc_contracts::provider_slots::{ADZUNA_APP_ID, ADZUNA_APP_KEY};
        Self {
            app_id: crate::credentials::read_credential(&format!("ai:{ADZUNA_APP_ID}"))
                .unwrap_or_else(|e| {
                    log::warn!(
                        "[aggregator] {ADZUNA_APP_ID} keyring error: {}",
                        sanitize_reason(&e.to_string())
                    );
                    None
                }),
            app_key: crate::credentials::read_credential(&format!("ai:{ADZUNA_APP_KEY}"))
                .unwrap_or_else(|e| {
                    log::warn!(
                        "[aggregator] {ADZUNA_APP_KEY} keyring error: {}",
                        sanitize_reason(&e.to_string())
                    );
                    None
                }),
            note_sink: None,
            base_url: ADZUNA_BASE_URL.to_string(),
        }
    }

    /// Attach a location-policy note sink (from the aggregator's `ScrapeContext`).
    pub(super) fn with_note_sink(
        mut self,
        sink: Option<Arc<dyn Fn(String) + Send + Sync>>,
    ) -> Self {
        self.note_sink = sink;
        self
    }

    /// Point the provider at a different API host. Test-only seam (see
    /// [`Self::base_url`]); production always uses [`ADZUNA_BASE_URL`].
    #[cfg(test)]
    pub(super) fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Emit an informational location-policy note through the injected sink, if any.
    fn report_note(&self, note: String) {
        if let Some(ref sink) = self.note_sink {
            sink(note);
        }
    }
}

#[async_trait]
impl JobProvider for AdzunaProvider {
    fn provider_id(&self) -> &'static str {
        "adzuna"
    }

    fn is_configured(&self) -> bool {
        self.app_id.is_some() && self.app_key.is_some()
    }

    async fn search(
        &self,
        query: &str,
        location: &str,
        country: &str,
        country_guessed: bool,
        date_filter: Option<&str>,
        amount: Option<u32>,
        signal: tokio_util::sync::CancellationToken,
    ) -> anyhow::Result<Vec<JobPosting>> {
        if !self.is_configured() {
            return Err(anyhow::anyhow!("adzuna: not configured"));
        }

        // Reject unsupported countries before issuing any HTTP request.
        // Adzuna only hosts a fixed set of markets; an unsupported country code
        // would produce a non-2xx response (indistinguishable from an auth error
        // at the HTTP level without real keys). Returning Err here lets the
        // `search_with_providers` fallback chain transparently route to JSearch
        // (which uses free-text location and is globally scoped).
        let country = if country.is_empty() { "de" } else { country };
        if !adzuna_supports_country(country) {
            return Err(anyhow::anyhow!(
                "adzuna: country '{country}' is not in Adzuna's supported market list \
                 (supported: {}); configure a JSearch key for global coverage",
                ADZUNA_SUPPORTED_COUNTRIES.join(", ")
            ));
        }

        let app_id = self.app_id.as_deref().unwrap_or("");
        let app_key = self.app_key.as_deref().unwrap_or("");

        // Drop redundant country suffixes so a ", Germany"/", Deutschland" tail
        // doesn't over-narrow the geocode (the country is already the URL path).
        let where_hygienic = adzuna_where(location);

        let req = AdzunaPageRequest {
            base_url: &self.base_url,
            country,
            app_id,
            app_key,
            query,
            where_val: where_hygienic,
            date_filter,
        };

        let postings = fetch_adzuna_pages(req, amount, signal.clone()).await?;

        // Surface the guessed-market policy when this guess produced the
        // authoritative result (>= floor, so `primary_chain` keeps it). `broaden`
        // never fires for a guessed market, so `postings.len()` here is final for
        // that branch. Country code only — the raw location is never emitted.
        if let Some(note) = guessed_market_note(country_guessed, location, postings.len(), country)
        {
            self.report_note(note);
        }

        // Broaden on near-empty: even a hygienic `where` can over-narrow a sparse
        // market, so if a real Adzuna market returned under the floor, retry ONCE
        // country-wide (`where=""`) — same `what`, sort, and `max_days_old` — and
        // keep whichever set is larger. A transient error on the retry keeps the
        // narrow result rather than discarding it.
        //
        // SINGLE PAGE, deliberately: paging the retry as well would multiply the
        // two budgets. The retry only fires when the paged loop collected fewer
        // than `ADZUNA_BROADEN_FLOOR` (3) results, which is USUALLY because page 1
        // came back short and the loop stopped after one fetch — but not always: a
        // FULL page 1 whose postings all dedupe away keeps the loop going, so the
        // worst-case cost of a search is `ADZUNA_MAX_PAGES + 1` DAILY-QUOTA CALLS,
        // not 2. Quota calls and fetches are the same number only because
        // `ADZUNA_RETRIES` is 0; with the default `retries: 2` each fetch would
        // bill up to 3 (see the constant).
        //
        // GUARD: never broaden a GUESSED market (`country_guessed`). Turning a
        // guessed-market empty/near-empty into a non-empty country-wide result
        // would defeat `primary_chain`'s guessed-market guard, which relies on
        // an empty Adzuna result to fall through to JSearch (global, free-text
        // location) when the guess is probably wrong (e.g. "London" defaulting
        // to "de"). Only broaden for an explicitly-supplied country.
        // GUARD: never spend the broaden retry's quota call after a Stop. The loop
        // above returns `Ok(what it had)` on cancel, and a cancelled run is short
        // by construction — without this check the deliberate stop would look like
        // a sparse market and buy one more request on the way out.
        if !signal.is_cancelled() && should_broaden(country_guessed, where_hygienic, postings.len())
        {
            match fetch_adzuna_page(
                AdzunaPageRequest {
                    where_val: "",
                    ..req
                },
                1,
                signal,
            )
            .await
            {
                Ok(broadened) if broadened.len() > postings.len() => {
                    // PRIVACY: never log the raw `where`/location — free-text PII.
                    log::info!(
                        "[aggregator] adzuna sparse result ({}), broadened country-wide ({})",
                        postings.len(),
                        broadened.len()
                    );
                    // Surface the sparse-city → country-wide broadening. Country
                    // code only — never the raw location (free-text PII).
                    self.report_note(format!("broadened:{country}"));
                    return Ok(broadened);
                }
                Ok(_) => {}
                Err(e) => {
                    log::warn!(
                        "[aggregator] adzuna broaden retry failed, keeping narrow result: {e}"
                    )
                }
            }
        }

        Ok(postings)
    }
}
