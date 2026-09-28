//! Multi-board scrape orchestration: resolve + dedupe the board list,
//! skip boards with no usable session/company/keys, fan out via
//! [`super::run`], apply the central location/work-type post-filters, and
//! fold the run into board health. Split out of `mod.rs` (issue #1280) and
//! further split (issue #1280 review round) into one file per phase — the
//! shared locals of the former single function are grouped into small
//! per-phase structs ([`ResolvedBoards`], [`RequestedFilters`],
//! [`SkipOutcome`], [`FanOutSinks`]) instead of threading a dozen
//! parameters, so [`ScraperEngine::scrape_boards_with_resolver_and_overrides`]
//! reads as a short, ordered phase list.

use std::collections::HashMap;
use std::sync::Arc;

use crate::scraping::types::{BoardSearchInput, JobPosting, Scraper};

use super::dedup::dedup_cross_source;
use super::{BoardScrapeSummary, ScraperEngine};

mod fan_out_sinks;
mod requested_filters;
mod resolved_boards;
mod skip_outcome;

use fan_out_sinks::FanOutSinks;
use requested_filters::RequestedFilters;
use resolved_boards::ResolvedBoards;
use skip_outcome::SkipOutcome;

impl ScraperEngine {
    /// Multi-board scrape: acquire one engine permit, reuse or mint the parent
    /// cancellation token under `job_id`, fan out via [`run_boards`], then
    /// assemble results and summaries.
    ///
    /// Returns `(postings, summaries)` where `postings` is the concatenation of
    /// all boards' results in input order, and `summaries` describe per-board
    /// counts/errors. Returns `Err` only when the user cancelled AND every board
    /// errored with no recovered items.
    pub async fn scrape_boards(
        &self,
        boards: &[String],
        input: BoardSearchInput,
        job_id: String,
        on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>,
        on_item: Option<Arc<dyn Fn(JobPosting) + Send + Sync>>,
    ) -> anyhow::Result<(Vec<JobPosting>, Vec<BoardScrapeSummary>)> {
        self.scrape_boards_with_resolver(
            boards,
            input,
            job_id,
            on_progress,
            on_item,
            &crate::platform::config::data_dir(),
            |id| {
                crate::scraping::boards::get(id)
                    .ok_or_else(|| anyhow::anyhow!("Unknown board: {id}"))
            },
        )
        .await
    }

    /// Watched-companies variant (ADR-030 §e): like [`scrape_boards`] but routes an
    /// explicit PER-BOARD company override into the engine's existing per-board
    /// `seeded_companies` path. `None` = today's behavior (curated `ats_seed`
    /// fallback for a company-scoped board with an empty `companies` list).
    /// `Some(map)` = watched mode: a company-scoped board runs ONLY with its own
    /// slugs (`map[board]`) — the `ats_seed` fallback is bypassed and a board with
    /// no (or empty) entry is skipped `needs-company`, so it is NEVER fetched with a
    /// foreign ATS's slugs. Callers pass an empty `input.companies` in this mode.
    pub async fn scrape_boards_with_overrides(
        &self,
        boards: &[String],
        input: BoardSearchInput,
        job_id: String,
        on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>,
        on_item: Option<Arc<dyn Fn(JobPosting) + Send + Sync>>,
        company_overrides: Option<&HashMap<String, Vec<String>>>,
    ) -> anyhow::Result<(Vec<JobPosting>, Vec<BoardScrapeSummary>)> {
        self.scrape_boards_with_resolver_and_overrides(
            boards,
            input,
            job_id,
            on_progress,
            on_item,
            &crate::platform::config::data_dir(),
            |id| {
                crate::scraping::boards::get(id)
                    .ok_or_else(|| anyhow::anyhow!("Unknown board: {id}"))
            },
            company_overrides,
        )
        .await
    }

    /// Test-only resolver seam: identical to `scrape_boards` but accepts a
    /// caller-supplied `resolve` function and an explicit `data_dir` so tests
    /// can inject fake scrapers and an isolated tempdir without touching the
    /// real `boards::get` registry or `crate::platform::config::data_dir()`.
    /// Signature preserved (no `company_overrides`) so existing engine tests are
    /// untouched; delegates with `None` (curated `ats_seed` behavior).
    #[doc(hidden)]
    pub(crate) async fn scrape_boards_with_resolver<F>(
        &self,
        boards: &[String],
        input: BoardSearchInput,
        job_id: String,
        on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>,
        on_item: Option<Arc<dyn Fn(JobPosting) + Send + Sync>>,
        data_dir: &std::path::Path,
        resolve: F,
    ) -> anyhow::Result<(Vec<JobPosting>, Vec<BoardScrapeSummary>)>
    where
        F: Fn(&str) -> anyhow::Result<&'static dyn Scraper>,
    {
        self.scrape_boards_with_resolver_and_overrides(
            boards,
            input,
            job_id,
            on_progress,
            on_item,
            data_dir,
            resolve,
            None,
        )
        .await
    }

    /// Core impl behind [`scrape_boards_with_resolver`] +
    /// [`scrape_boards_with_overrides`]. `company_overrides` routes watched-company
    /// per-board slugs through the existing `seeded_companies` path (see
    /// [`scrape_boards_with_overrides`]); `None` keeps the curated `ats_seed`
    /// fallback. `pub(crate)` so the per-board routing is directly testable.
    ///
    /// A short, ordered phase list (issue #1280 review round — was one ~400-line
    /// function): each phase is a method on a small struct that owns exactly the
    /// locals it needs, in the SAME order, with the SAME awaits/error paths/
    /// logging as before the split — see the per-phase modules for the moved
    /// bodies.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn scrape_boards_with_resolver_and_overrides<F>(
        &self,
        boards: &[String],
        input: BoardSearchInput,
        job_id: String,
        on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>,
        on_item: Option<Arc<dyn Fn(JobPosting) + Send + Sync>>,
        data_dir: &std::path::Path,
        resolve: F,
        company_overrides: Option<&HashMap<String, Vec<String>>>,
    ) -> anyhow::Result<(Vec<JobPosting>, Vec<BoardScrapeSummary>)>
    where
        F: Fn(&str) -> anyhow::Result<&'static dyn Scraper>,
    {
        // F1 — guard against empty board list before doing any async work.
        if boards.is_empty() {
            return Err(anyhow::anyhow!("at least one board is required"));
        }

        // Bound concurrency — one engine permit for the whole multi-board batch.
        let sem = self.semaphore.load_full();
        let _permit = sem
            .acquire_owned()
            .await
            .map_err(|_| anyhow::anyhow!("scraper engine semaphore closed"))?;

        // F2/F5 — reuse a pre-registered token (Autopilot pre-registers its own
        // token for the whole run) or mint a fresh one. Track whether WE minted it
        // so we only remove the slot when we own it — a pre-registered token is
        // managed by the caller (Autopilot calls `unregister_token` itself).
        let (parent, we_minted) = self.jobs.get_or_register(&job_id).await;

        // Phase: resolve + dedupe the caller's board list.
        let ResolvedBoards {
            resolved,
            resolvable_boards,
        } = ResolvedBoards::resolve(boards, resolve);

        // Phase: the central location/work-type post-filter inputs (trust PR F /
        // Phase 2b) — computed once, reused by the fan-out keep predicate below
        // AND the post-hoc safety-net pass in `assemble_results`.
        let filters = RequestedFilters::compute(&input, &resolved);

        // Phase: per-board skip checks (needs-login / needs-company / needs-keys).
        let skipped = SkipOutcome::compute(resolved, &input, data_dir, company_overrides);

        let sinks = FanOutSinks::new();
        let keep_item =
            filters.keep_item(sinks.location_drops.clone(), sinks.work_type_drops.clone());

        // Phase: seed company slugs for the ATS-requiring boards that are
        // actually running.
        let seeded_companies = skipped.seeded_companies(company_overrides);

        // Phase: fan out.
        let results = Self::run_boards(
            skipped.runnable,
            input,
            parent.clone(),
            on_progress,
            on_item,
            Some(sinks.truncation_sink.clone()),
            Some(sinks.note_sink.clone()),
            keep_item,
            self.browser_sem.clone(),
            &seeded_companies,
        )
        .await;

        // F5 — only remove the token slot when we minted it. A pre-registered
        // token (Autopilot pre-registers its own) is managed by the caller.
        if we_minted {
            self.jobs.unregister(&job_id).await;
        }

        // Phase: assemble per-board summaries, applying the central
        // location/work-type post-filter safety net.
        let (all_postings, summaries, any_recovered_items) = assemble_results(
            results,
            &skipped.name_to_idx,
            skipped.slot_summaries,
            &filters,
            &sinks,
        );

        // F6 — return Err only when the user cancelled AND no items were recovered.
        // A board returning Ok([]) after observing cancellation is not a success.
        if parent.is_cancelled() && !any_recovered_items {
            return Err(anyhow::anyhow!("scrape cancelled"));
        }

        // Track B1 — fold this run into the per-board reliability history and
        // attach the result, so a chip can tell "found nothing today" apart from
        // "broken since Tuesday". Deliberately AFTER the cancellation check: a
        // cancelled run's per-board errors are the user's own cancel, not the
        // board's fault, and recording them would manufacture failure streaks.
        // Never suppresses a board (owner decision: record and display, never
        // skip) — a failing board still runs on the next search.
        let summaries = if parent.is_cancelled() {
            summaries
        } else {
            self.record_health(&job_id, summaries, &resolvable_boards)
                .await
        };

        // Cross-source dedup (trust PR E, stage 1): the same job surfaced by two
        // boards was concatenated above as separate rows — collapse to one, upgrading
        // the incumbent's description/extra from the richer duplicate in first-seen
        // order (see `dedup_cross_source`). Per-board `summaries[i].count` stay
        // as-fetched (they describe each board's raw return), so the removed count is
        // the cross-source overlap, surfaced as a log line only (no summary field / no
        // renderer change). Not board-attributed here: `summaries.len()` would also
        // count skipped/errored boards that contributed nothing to collapse.
        let before = all_postings.len();
        let all_postings = dedup_cross_source(all_postings);
        let removed = before - all_postings.len();
        if removed > 0 {
            log::info!("[scrape] collapsed {removed} cross-source duplicate(s)");
        }

        Ok((all_postings, summaries))
    }
}

/// Assemble per-board [`BoardScrapeSummary`]s from `run_boards`'s results,
/// applying the central location/work-type post-filter as a SAFETY NET (a
/// no-op when the live `keep_item` gate already ran — see `KeepItemFn`'s
/// cap/filter ordering doc). Pure extraction of the former inline loop in
/// `scrape_boards_with_resolver_and_overrides` (issue #1280 review round) —
/// same statements, same order, only the outer locals now read from `filters`/
/// `sinks` instead of the function's own scope.
///
/// Returns `(all_postings, summaries, any_recovered_items)` — `any_recovered_items`
/// feeds the F6 cancel-with-nothing-recovered check right after this call.
fn assemble_results(
    results: Vec<(String, anyhow::Result<Vec<JobPosting>>)>,
    name_to_idx: &HashMap<String, usize>,
    mut slot_summaries: Vec<Option<BoardScrapeSummary>>,
    filters: &RequestedFilters,
    sinks: &FanOutSinks,
) -> (Vec<JobPosting>, Vec<BoardScrapeSummary>, bool) {
    let mut all_postings: Vec<JobPosting> = Vec::new();
    // F6 — true only when at least one board returned a non-empty Ok.
    let mut any_recovered_items = false;

    // Fill run results back into their original positions.
    for (board, res) in results {
        let idx = name_to_idx[&board];
        match res {
            Ok(postings) => {
                // Central location post-filter (trust PR F, see `location_filter`)
                // — a SAFETY NET here, not the primary filter: a no-op when the
                // live gate already ran (see `KeepItemFn`), the only filtering
                // pass otherwise (callers with no live on_item, e.g. tests).
                let (postings, post_hoc_dropped) = match &filters.requested_location {
                    Some(req) if filters.non_location_boards.contains(&board) => {
                        super::location_filter::filter_postings(postings, req)
                    }
                    _ => (postings, 0),
                };
                // Central work-type post-filter — the identical safety-net shape,
                // one line down: a no-op when the live gate already ran, the only
                // filtering pass otherwise.
                let (postings, wt_post_hoc_dropped) = match &filters.requested_work_types {
                    Some(wanted) if filters.non_work_type_boards.contains(&board) => {
                        super::work_type_filter::filter_postings(postings, wanted)
                    }
                    _ => (postings, 0),
                };
                if !postings.is_empty() {
                    any_recovered_items = true;
                }
                // A partial harvest (paginated board that failed on a later page)
                // is surfaced here so it is not indistinguishable from a complete
                // run; a board that completed its pages has no map entry.
                let truncated = sinks
                    .truncations
                    .lock()
                    .ok()
                    .and_then(|mut m| m.remove(&board));
                // `notes` (plural) can now carry MULTIPLE independent honesty
                // signals at once — see `BoardScrapeSummary::notes`. Order fixes
                // the precedence: the board's own note (if any) first, then
                // location, then work type.
                let mut board_notes: Vec<String> = sinks
                    .notes
                    .lock()
                    .ok()
                    .and_then(|mut m| m.remove(&board))
                    .into_iter()
                    .collect();
                // Combine the live gate's drop count with this pass's.
                let live_location_dropped = sinks
                    .location_drops
                    .lock()
                    .ok()
                    .and_then(|mut m| m.remove(&board))
                    .unwrap_or(0);
                let location_dropped = live_location_dropped + post_hoc_dropped;
                // UNCONDITIONAL (incl. dropped==0): a location was requested and
                // this board doesn't honor it server-side, so its results were
                // never authoritative for that location regardless of whether any
                // row actually got dropped this run — the picker/chips must say so
                // every time, not just when there happened to be a drop. Emitting
                // only on dropped>0 let a non-supporting board with 0 drops read as
                // indistinguishable from a supporting one ("all ok"), half-telling
                // the 17/23-boards-ignore-location story. Deliberate consequence:
                // any run touching a non-supporting board with a location set no
                // longer collapses to a clean chip — that's intended honesty, not
                // a bug (stage-2 renders n=0 as a plain "location filtered
                // locally" marker, n>0 as the count).
                if filters.non_location_boards.contains(&board)
                    && filters.requested_location.is_some()
                {
                    // Surface via the existing note side-channel using the PR D
                    // `kind:value` grammar (cf. `broadened:<cc>`). Count only —
                    // never the raw location text (free-text PII). `notes` now
                    // holds every applicable signal (see the doc above), so this
                    // ADDS a `location-filtered` entry rather than only filling an
                    // empty slot — a board-native note no longer silently loses it.
                    board_notes.push(format!("location-filtered:{location_dropped}"));
                }
                // Combine the live gate's drop count with this pass's (work type).
                let live_wt_dropped = sinks
                    .work_type_drops
                    .lock()
                    .ok()
                    .and_then(|mut m| m.remove(&board))
                    .unwrap_or(0);
                let wt_dropped = live_wt_dropped + wt_post_hoc_dropped;
                // UNCONDITIONAL for the identical reason as `location-filtered`
                // above: a non-supporting board's results were never authoritative
                // for the requested work type regardless of whether a row happened
                // to drop this run.
                if filters.non_work_type_boards.contains(&board)
                    && filters.requested_work_types.is_some()
                {
                    board_notes.push(format!("work-type-filtered:{wt_dropped}"));
                }
                slot_summaries[idx] = Some(BoardScrapeSummary {
                    board,
                    count: postings.len(),
                    error: None,
                    skipped: None,
                    truncated,
                    notes: board_notes,
                    health: None,
                });
                all_postings.extend(postings);
            }
            Err(e) => {
                slot_summaries[idx] = Some(BoardScrapeSummary {
                    board,
                    count: 0,
                    error: Some(e.to_string()),
                    skipped: None,
                    truncated: None,
                    notes: Vec::new(),
                    health: None,
                });
            }
        }
    }

    // Flatten in input order — every slot is now Some (skips were filled above,
    // run results were filled by name_to_idx lookup).
    let summaries: Vec<BoardScrapeSummary> = slot_summaries.into_iter().flatten().collect();
    (all_postings, summaries, any_recovered_items)
}
