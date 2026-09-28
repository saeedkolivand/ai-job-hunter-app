//! `SkipOutcome` — the per-board skip-check phase of
//! `scrape_boards_with_resolver_and_overrides` (issue #1280 review round):
//! boards short-circuited before any fetch (`needs-login`/`needs-company`/
//! `needs-keys`) vs. runnable, plus the `seeded_companies` phase that
//! consumes its `runnable`/`has_usable_company` output.

use std::collections::HashMap;
use std::path::Path;

use crate::scraping::board_login::{load_cookies, session_age_ms, session_is_stale};
use crate::scraping::boards::ats_seed;
use crate::scraping::types::{AuthRequirement, BoardSearchInput, Scraper};

use super::super::BoardScrapeSummary;

pub(super) struct SkipOutcome {
    pub(super) slot_summaries: Vec<Option<BoardScrapeSummary>>,
    pub(super) runnable: Vec<(String, anyhow::Result<&'static dyn Scraper>)>,
    pub(super) name_to_idx: HashMap<String, usize>,
    pub(super) has_usable_company: bool,
}

impl SkipOutcome {
    /// Pure extraction of the former inline block in
    /// `scrape_boards_with_resolver_and_overrides` — same statements, same
    /// order (the `has_usable_company` pre-computation, then the skip loop).
    pub(super) fn compute(
        resolved: Vec<(String, anyhow::Result<&'static dyn Scraper>)>,
        input: &BoardSearchInput,
        data_dir: &Path,
        company_overrides: Option<&HashMap<String, Vec<String>>>,
    ) -> Self {
        // Short-circuit: skip Required boards that have no usable session.
        // Skipped boards never enter run_boards (no fetch, no browser_sem acquire).
        // Use a position-indexed Option<BoardScrapeSummary> so skips are slotted
        // at their original index and the final flatten preserves input order.
        let n = resolved.len();
        let mut slot_summaries: Vec<Option<BoardScrapeSummary>> = (0..n).map(|_| None).collect();
        let mut runnable: Vec<(String, anyhow::Result<&'static dyn Scraper>)> = Vec::new();
        // Map board name → original index for filling run results later.
        let mut name_to_idx: HashMap<String, usize> = HashMap::new();

        // Pre-compute whether the input contains at least one non-whitespace company
        // slug.  A payload like [" ", "\t"] bypasses `is_empty()` but ATS scrapers
        // trim-and-drop those entries, yielding no usable company — the same outcome
        // as an empty list.  Check once here so the per-board skip stays O(1).
        let has_usable_company = input.companies.iter().any(|c| !c.trim().is_empty());

        for (idx, (id, scraper)) in resolved.into_iter().enumerate() {
            // Determine skip reason (if any) from the resolved scraper.
            // Unknown-board Err values always pass through (no skip) so they
            // produce a normal error summary rather than a misleading skip.
            let skip_reason: Option<&'static str> = scraper.as_ref().ok().and_then(|s| {
                // Skip 1: Required auth board with no valid session.
                if s.auth() == AuthRequirement::Required {
                    let no_session = load_cookies(data_dir, &id).is_empty()
                        || session_age_ms(data_dir, &id).is_none()
                        || session_is_stale(data_dir, &id);
                    if no_session {
                        return Some("needs-login");
                    }
                }
                // Skip 2: ATS board that requires a company slug but none usable.
                // Treats whitespace-only entries (e.g. [" ", "\t"]) the same as
                // an empty list — they are trimmed-and-dropped by ATS scrapers.
                if s.requires_company() {
                    match company_overrides {
                        // Watched mode (ADR-030 §e): run ONLY when this board has a
                        // non-empty per-board override; a board with no watched slug
                        // is skipped `needs-company` — never fetched with a foreign
                        // ATS's slugs, and the `ats_seed` fallback is bypassed.
                        Some(overrides) => {
                            let has_watched = overrides
                                .get(&id)
                                .map(|slugs| !slugs.is_empty())
                                .unwrap_or(false);
                            if !has_watched {
                                return Some("needs-company");
                            }
                        }
                        // Normal mode: a board with a curated `ats_seed` entry still
                        // runs — the engine auto-populates `input.companies` from the
                        // seed right before `run_boards` (see `seeded_companies`
                        // below), so skipping here would strand those seeded slugs.
                        // Keyed on `s.id()` (the Scraper trait id the seed's `ats`
                        // field matches), NOT the board-list string (`id`).
                        None => {
                            if !has_usable_company && ats_seed::by_ats(s.id()).next().is_none() {
                                return Some("needs-company");
                            }
                        }
                    }
                }
                // Skip 3: key-backed board (e.g. the aggregator) with no API keys
                // configured. Surfaces "needs-keys" so the UI can prompt the user
                // to add keys instead of showing a silent, unexplained zero.
                if s.needs_keys() {
                    return Some("needs-keys");
                }
                None
            });

            if let Some(reason) = skip_reason {
                slot_summaries[idx] = Some(BoardScrapeSummary {
                    board: id,
                    count: 0,
                    error: None,
                    skipped: Some(reason.into()),
                    truncated: None,
                    notes: Vec::new(),
                    health: None,
                });
            } else {
                name_to_idx.insert(id.clone(), idx);
                runnable.push((id, scraper));
            }
        }

        Self {
            slot_summaries,
            runnable,
            name_to_idx,
            has_usable_company,
        }
    }

    /// Pure extraction of the former inline block in
    /// `scrape_boards_with_resolver_and_overrides` — same statements, same
    /// order.
    pub(super) fn seeded_companies(
        &self,
        company_overrides: Option<&HashMap<String, Vec<String>>>,
    ) -> HashMap<String, Vec<String>> {
        // Per-board company slugs for the company-scoped ATS scrapers. Keyed on the
        // caller-supplied board-list id (`run_boards`'s `name` param), matching how
        // `runnable` is keyed — NOT on `s.id()`.
        //
        // Watched mode (ADR-030 §e): use the explicit per-board override verbatim
        // (boards without a non-empty entry were already skipped `needs-company`
        // above); the curated `ats_seed` fallback is bypassed so no board is ever
        // fed a foreign ATS's slug.
        //
        // Normal mode: auto-populate from the curated `ats_seed` table when the user
        // left the global company field blank (an explicit user `companies` list
        // always wins — this only fills when `companies` is globally empty).
        let mut seeded_companies: HashMap<String, Vec<String>> = HashMap::new();
        match company_overrides {
            Some(overrides) => {
                for (id, scraper) in &self.runnable {
                    let Ok(s) = scraper else { continue };
                    if !s.requires_company() {
                        continue;
                    }
                    if let Some(slugs) = overrides.get(id) {
                        if !slugs.is_empty() {
                            seeded_companies.insert(id.clone(), slugs.clone());
                        }
                    }
                }
            }
            None => {
                if !self.has_usable_company {
                    for (id, scraper) in &self.runnable {
                        let Ok(s) = scraper else { continue };
                        if !s.requires_company() {
                            continue;
                        }
                        let slugs: Vec<String> = ats_seed::by_ats(s.id())
                            .map(|e| e.slug.to_string())
                            .collect();
                        if !slugs.is_empty() {
                            seeded_companies.insert(id.clone(), slugs);
                        }
                    }
                }
            }
        }
        seeded_companies
    }
}
