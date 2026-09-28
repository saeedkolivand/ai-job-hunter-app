//! `resolve + dedupe` phase of `scrape_boards_with_resolver_and_overrides`
//! (issue #1280 review round): deduplicate the caller's board list,
//! resolve each id, and record which ids the board-health store may key
//! a row on.

use std::collections::HashSet;

use crate::scraping::types::Scraper;

use super::super::max_boards_per_batch;

/// Output of [`ResolvedBoards::resolve`].
pub(super) struct ResolvedBoards {
    pub(super) resolved: Vec<(String, anyhow::Result<&'static dyn Scraper>)>,
    pub(super) resolvable_boards: HashSet<String>,
}

impl ResolvedBoards {
    /// Pure extraction of the former inline dedupe/resolve block in
    /// `scrape_boards_with_resolver_and_overrides` — same statements, same
    /// order.
    pub(super) fn resolve<F>(boards: &[String], resolve: F) -> Self
    where
        F: Fn(&str) -> anyhow::Result<&'static dyn Scraper>,
    {
        // Dedupe (first-seen order) + truncate to max_boards_per_batch() (the
        // registry size) so a crafted payload with thousands of valid ids
        // cannot build thousands of futures and drive ban amplification on
        // the user's own sessions.
        let boards_deduped: Vec<&String> = {
            let mut seen = std::collections::HashSet::new();
            boards
                .iter()
                .filter(|id| seen.insert(id.as_str()))
                .take(max_boards_per_batch())
                .collect()
        };

        // Resolve board ids via the supplied resolver; unknown boards become
        // per-entry Err values so the run still proceeds for the boards that ARE known.
        let resolved: Vec<(String, anyhow::Result<&'static dyn Scraper>)> = boards_deduped
            .iter()
            .map(|id| {
                let scraper = resolve(id.as_str());
                (id.to_string(), scraper)
            })
            .collect();

        // The ONLY ids the board-health store may key a row on (Track B1).
        //
        // An unknown id deliberately passes through to an ordinary error summary
        // below rather than being dropped — but `board` is that store's PRIMARY
        // KEY and this string is renderer-supplied verbatim (`commands::scrape`
        // clones `req.boards` in; the generated `ScrapeBoardsRequest.boards` is
        // an unvalidated `Vec<String>`). Recording it would give a looping or
        // XSS'd renderer unbounded on-disk row creation — the same threat the
        // scrape limiter in `commands::scrape` already exists to stop, and the
        // reason this store can honestly claim to be bounded by the scraper
        // registry. Keyed off the RESOLVER (not `boards::get`) so the engine's
        // fake-resolver tests keep their synthetic ids.
        let resolvable_boards: HashSet<String> = resolved
            .iter()
            .filter(|(_, scraper)| scraper.is_ok())
            .map(|(id, _)| id.clone())
            .collect();

        Self {
            resolved,
            resolvable_boards,
        }
    }
}
