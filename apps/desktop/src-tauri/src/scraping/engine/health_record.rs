//! Fold a run's summaries into the per-board reliability history
//! ([`super::super::board_health`]) — split out of `mod.rs` (issue #1280).

use std::collections::HashSet;

use super::{BoardScrapeSummary, ScraperEngine};

impl ScraperEngine {
    /// Fold `summaries` into the per-board reliability history and stamp the
    /// UNHEALTHY ones with the result (Track B1).
    ///
    /// Only a noteworthy health (failing / stale) is attached: a healthy board
    /// adds nothing to its chip, and leaving it off keeps the persisted
    /// `lastRunSummaries` records — and the "N boards · all ok" collapse — as
    /// small and quiet as they are today.
    ///
    /// A missing store or a storage error degrades to "no health this run" and
    /// returns the summaries untouched: the diagnostic must never break the
    /// scrape it describes. The SQLite work runs on the blocking pool.
    pub(super) async fn record_health(
        &self,
        run_id: &str,
        mut summaries: Vec<BoardScrapeSummary>,
        resolvable_boards: &HashSet<String>,
    ) -> Vec<BoardScrapeSummary> {
        let Some(store) = self.health.load_full() else {
            return summaries;
        };
        // Only ids the resolver recognised may key a row — see
        // `resolvable_boards`. Positions are kept so each health lands back on
        // the summary it came from; an unresolvable board simply never enters
        // the store and never gets a badge.
        let recorded: Vec<usize> = summaries
            .iter()
            .enumerate()
            .filter(|(_, s)| resolvable_boards.contains(&s.board))
            .map(|(idx, _)| idx)
            .collect();
        if recorded.is_empty() {
            return summaries;
        }
        // The blocking task gets a CLONE (a handful of small structs) rather than
        // the summaries themselves, so a panicked or aborted task costs the
        // health badge, never the diagnostics the renderer actually needs.
        let run_id = run_id.to_string();
        let to_record: Vec<BoardScrapeSummary> =
            recorded.iter().map(|&idx| summaries[idx].clone()).collect();
        let health = match tokio::task::spawn_blocking(move || {
            store.record_run(&run_id, &to_record)
        })
        .await
        {
            Ok(Ok(health)) => health,
            Ok(Err(e)) => {
                // Path-free category code, not the raw `AppError` — the same
                // rule as the store's own `open()` failure in `lib.rs`'s setup
                // path: a storage error can embed the db path.
                log::warn!(
                    "[scrape] board-health history unavailable this run ({}); chips lose \
                     their reliability badge but the scrape stands",
                    e.code()
                );
                return summaries;
            }
            Err(e) => {
                log::warn!("[scrape] board-health record task failed: {e}");
                return summaries;
            }
        };
        for (&idx, h) in recorded.iter().zip(health) {
            if h.is_noteworthy() {
                summaries[idx].health = Some(h);
            }
        }
        summaries
    }
}
