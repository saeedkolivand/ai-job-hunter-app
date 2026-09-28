//! `RequestedFilters` — the central location/work-type post-filter phase
//! (trust PR F / Phase 2b) of `scrape_boards_with_resolver_and_overrides`
//! (issue #1280 review round): computed once, reused by the fan-out
//! [`RequestedFilters::keep_item`] predicate and the post-hoc safety-net
//! pass in `assemble_results`.

use std::collections::HashSet;
use std::sync::Arc;

use crate::scraping::types::{BoardSearchInput, JobPosting, LocationSpec, Scraper, WorkType};

use super::super::{location_filter, work_type_filter, KeepItemByBoardFn};

pub(super) struct RequestedFilters {
    pub(super) requested_location: Option<LocationSpec>,
    pub(super) non_location_boards: HashSet<String>,
    pub(super) requested_work_types: Option<Vec<WorkType>>,
    pub(super) non_work_type_boards: HashSet<String>,
}

impl RequestedFilters {
    /// Pure extraction of the former inline block in
    /// `scrape_boards_with_resolver_and_overrides` — same statements, same
    /// order.
    pub(super) fn compute(
        input: &BoardSearchInput,
        resolved: &[(String, anyhow::Result<&'static dyn Scraper>)],
    ) -> Self {
        // Central location post-filter (trust PR F): when a location was requested,
        // boards that do NOT consume it server-side (`supports_location() == false`)
        // get their results conservatively filtered so a wrong-city row can't pass
        // as a hit. Computed once here; the set is empty (filter inert) when no
        // location was requested, keeping location-agnostic searches byte-identical.
        let requested_location = input.location_spec();
        let non_location_boards: std::collections::HashSet<String> = if requested_location.is_some()
        {
            resolved
                .iter()
                .filter(|(_, scraper)| {
                    scraper
                        .as_ref()
                        .map(|s| !s.supports_location())
                        .unwrap_or(false)
                })
                .map(|(id, _)| id.clone())
                .collect()
        } else {
            std::collections::HashSet::new()
        };

        // Work-type sibling of the location post-filter above (same shape,
        // same conservatism — see `work_type_filter`'s module doc).
        // `BoardSearchInput::work_type_spec` is the one place that resolves
        // "empty/absent means no request" (mirroring `location_spec`) and
        // dedupes, so this is the ONLY reader of `input.work_types` — a future
        // upstream-pass-through board reads the same seam instead of
        // re-deriving the invariant.
        let requested_work_types: Option<Vec<crate::scraping::types::WorkType>> =
            input.work_type_spec();
        let non_work_type_boards: std::collections::HashSet<String> =
            if requested_work_types.is_some() {
                resolved
                    .iter()
                    .filter(|(_, scraper)| {
                        scraper
                            .as_ref()
                            .map(|s| !s.supports_work_type())
                            .unwrap_or(false)
                    })
                    .map(|(id, _)| id.clone())
                    .collect()
            } else {
                std::collections::HashSet::new()
            };

        Self {
            requested_location,
            non_location_boards,
            requested_work_types,
            non_work_type_boards,
        }
    }

    /// The composed keep-predicate — pure extraction of the former inline
    /// closure-building block in `scrape_boards_with_resolver_and_overrides`,
    /// same statements/order; `location_drops`/`work_type_drops` now arrive
    /// as parameters (they live in the sibling `FanOutSinks`) instead of a
    /// shared local. See `KeepItemFn`'s module-level cap/filter ordering doc.
    pub(super) fn keep_item(
        &self,
        location_drops: Arc<std::sync::Mutex<std::collections::HashMap<String, usize>>>,
        work_type_drops: Arc<std::sync::Mutex<std::collections::HashMap<String, usize>>>,
    ) -> Option<Arc<KeepItemByBoardFn>> {
        // Single composed predicate (NOT two): `run_boards`/`run_one` accept only
        // one `KeepItemByBoardFn`, so this is `Some` whenever EITHER filter is
        // active, and each half below is individually inert (`true`/no-op) when
        // its OWN filter was not requested — see the module-level `KeepItemFn`
        // doc for the cap/filter ordering invariant this participates in.
        if self.requested_location.is_some() || self.requested_work_types.is_some() {
            let non_loc = self.non_location_boards.clone();
            let loc_req = self.requested_location.clone();
            let loc_drops = location_drops.clone();
            let non_wt = self.non_work_type_boards.clone();
            let wt_req = self.requested_work_types.clone();
            let wt_drops = work_type_drops.clone();
            let f: Arc<KeepItemByBoardFn> = Arc::new(move |board: &str, item: &JobPosting| {
                // Location half — inert (no-op) when no location was requested.
                if let Some(ref req) = loc_req {
                    if non_loc.contains(board) && location_filter::location_mismatch(item, req) {
                        if let Ok(mut guard) = loc_drops.lock() {
                            *guard.entry(board.to_string()).or_insert(0) += 1;
                        }
                        return false;
                    }
                }
                // Work-type half — inert (no-op) when no work type was requested.
                if let Some(ref wanted) = wt_req {
                    if non_wt.contains(board) && work_type_filter::work_type_mismatch(item, wanted)
                    {
                        if let Ok(mut guard) = wt_drops.lock() {
                            *guard.entry(board.to_string()).or_insert(0) += 1;
                        }
                        return false;
                    }
                }
                true
            });
            Some(f)
        } else {
            None
        }
    }
}
