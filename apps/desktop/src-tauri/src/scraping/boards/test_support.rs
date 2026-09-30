//! Shared test-only builders for the per-board `Scraper::search` suites.
//!
//! Every board's tests built the full 18-field `BoardSearchInput` and the
//! 5-field `ScrapeContext` literal by hand — one to a handful of fields ever
//! differ per test, the rest is the same boilerplate copied into every board's
//! `test.rs`. `default_search_input()`/`default_ctx()` hold that boilerplate
//! once; a test overrides only what it varies via struct-update syntax
//! (`BoardSearchInput { companies: vec![...], ..default_search_input() }`).

use crate::scraping::types::{BoardSearchInput, ScrapeContext};

/// The zero-signal, no-company, no-keyword default a `BoardSearchInput` test
/// composes from. `amount: 10, pages: 1` matches what every board suite already
/// hardcoded.
pub(super) fn default_search_input() -> BoardSearchInput {
    BoardSearchInput {
        query: String::new(),
        location: None,
        amount: 10,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: None,
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: Vec::new(),
    }
}

/// A fresh, never-cancelled `ScrapeContext` with every sink unset. A test that
/// needs to cancel mid-run still can — `default_ctx().signal.cancel()` — and a
/// test that needs a shared token constructs one first and overrides `signal`
/// (`ScrapeContext { signal, ..default_ctx() }`).
pub(super) fn default_ctx() -> ScrapeContext {
    ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: None,
    }
}
