//! `FanOutSinks` — the shared mutable sinks the `run_boards` fan-out phase
//! writes into concurrently (issue #1280 review round): per-board
//! truncation/note strings plus location/work-type live-drop counters,
//! all folded into `BoardScrapeSummary` by `assemble_results`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub(super) struct FanOutSinks {
    pub(super) truncations: Arc<Mutex<HashMap<String, String>>>,
    pub(super) truncation_sink: Arc<dyn Fn(String, String) + Send + Sync>,
    pub(super) notes: Arc<Mutex<HashMap<String, String>>>,
    pub(super) note_sink: Arc<dyn Fn(String, String) + Send + Sync>,
    pub(super) location_drops: Arc<Mutex<HashMap<String, usize>>>,
    pub(super) work_type_drops: Arc<Mutex<HashMap<String, usize>>>,
}

impl FanOutSinks {
    /// Pure extraction of the former inline block in
    /// `scrape_boards_with_resolver_and_overrides` — same statements, same
    /// order; fully self-contained, zero renames.
    pub(super) fn new() -> Self {
        // Per-board truncation sink: a paginated board that keeps a partial harvest
        // after a mid-run page failure reports the reason through its ScrapeContext;
        // run_boards tags it with the board name and we attribute it to that board's
        // summary below. Empty map for a run where every board completed its pages.
        let truncations: Arc<std::sync::Mutex<HashMap<String, String>>> =
            Arc::new(std::sync::Mutex::new(HashMap::new()));
        let truncation_sink: Arc<dyn Fn(String, String) + Send + Sync> = {
            let truncations = truncations.clone();
            Arc::new(move |board, reason| {
                if let Ok(mut guard) = truncations.lock() {
                    guard.insert(board, reason);
                }
            })
        };

        // Per-board informational location-policy notes (aggregator guessed-market /
        // sparse city broadened country-wide). Same board-name-keyed collection as
        // truncations; folded into `BoardScrapeSummary.notes` below. Empty for a run
        // where no board applied such a policy.
        let notes: Arc<std::sync::Mutex<HashMap<String, String>>> =
            Arc::new(std::sync::Mutex::new(HashMap::new()));
        let note_sink: Arc<dyn Fn(String, String) + Send + Sync> = {
            let notes = notes.clone();
            Arc::new(move |board, note| {
                if let Ok(mut guard) = notes.lock() {
                    guard.insert(board, note);
                }
            })
        };

        // Per-board LIVE drop counts (see `KeepItemFn`) — the only place a
        // live-filtered item's count is observable; merged with the post-hoc
        // pass below (the no-live-streaming path, e.g. tests) into the note.
        // Location and work-type each get their OWN counter, but they are NOT
        // independent for a row that fails BOTH: the composed predicate below
        // evaluates location first and returns as soon as it drops a row, so a
        // row failing both is counted ONCE, under location — first-match-wins,
        // not two disjoint tallies. That is the right behavior (sum of the two
        // counters always equals total drops, never double-counting one row),
        // but it means `work-type-filtered:<n>` on such a run reports `n` net
        // of every row location already claimed, not "how many rows would have
        // failed the work-type check on their own" — still emitted
        // unconditionally (even at 0) below so the chip never reads as "this
        // filter didn't run" when it demonstrably did.
        let location_drops: Arc<std::sync::Mutex<HashMap<String, usize>>> =
            Arc::new(std::sync::Mutex::new(HashMap::new()));
        let work_type_drops: Arc<std::sync::Mutex<HashMap<String, usize>>> =
            Arc::new(std::sync::Mutex::new(HashMap::new()));
        Self {
            truncations,
            truncation_sink,
            notes,
            note_sink,
            location_drops,
            work_type_drops,
        }
    }
}
