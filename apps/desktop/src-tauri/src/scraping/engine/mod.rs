/// ScraperEngine — in-process scraper orchestrator.
///
/// Uses interior mutability so `Arc<ScraperEngine>` can be cloned into Tauri
/// commands and scrape jobs run concurrently (bounded by `semaphore`) without
/// serializing on an outer mutex.
use super::types::{
    AuthRequirement, BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode,
};
use arc_swap::ArcSwap;
use futures::StreamExt as _;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScraperCatalogEntry {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub mode: String,
    /// Auth tier — `"guest" | "optional" | "required"`.
    pub auth: AuthRequirement,
    /// Whether the board shows in the manual jobs picker.
    pub listed: bool,
    /// Whether the board requires at least one company slug in `input.companies`
    /// to return results. The engine skips boards with `requires_company=true`
    /// when `input.companies` is empty, reporting `skipped: "needs-company"`.
    #[serde(rename = "requiresCompany")]
    pub requires_company: bool,
}

#[derive(Debug, Clone)]
pub struct ScraperRuntimeHealth {
    pub mode: String,
    pub scrapers: Vec<ScraperCatalogEntry>,
    pub ready: bool,
}

/// Per-board outcome reported by [`ScraperEngine::scrape_boards`].
///
/// `Deserialize` is derived (not just `Serialize`) so the autopilot run record
/// can persist these on disk and load them back — the missing-field cases are
/// covered by serde's implicit `Option → None` (for `error`/`skipped`) and the
/// explicit `#[serde(default)]` on `truncated`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardScrapeSummary {
    pub board: String,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Set when the board was skipped without running (e.g. `"needs-login"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    /// Set when a paginated board kept a partial harvest after a mid-run page
    /// failure (e.g. `"page 3 of 5 failed: HTTP 429"`); `count` is then a partial
    /// tally, not the full result set. `None` means the harvest ran to completion.
    /// Serde-optional so records persisted before this field deserialize as `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated: Option<String>,
}

/// Maximum number of distinct boards processed per `scrape_boards` call.
///
/// Mirrors the Zod `.array().max(6)` cap in the renderer contract, but enforced
/// server-side so a crafted IPC payload or tampered autopilots.json with thousands
/// of (valid) board ids cannot build thousands of concurrent futures and drive
/// ban amplification against the user's own authenticated sessions.
const MAX_BOARDS_PER_BATCH: usize = 6;

pub struct ScraperEngine {
    /// Bounded concurrency — swapped on `set_concurrency`. Holding an
    /// owned permit for the duration of a scrape lets us shrink the limit
    /// without cancelling running work.
    semaphore: ArcSwap<Semaphore>,
    /// Active jobs keyed by job_id. Used by `cancel(job_id)`.
    jobs: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// Process-wide browser semaphore — one chromiumoxide session at a time.
    /// Shared across concurrent `scrape_boards` calls so two jobs that each
    /// include a browser board cannot spin up two headless instances at once.
    browser_sem: Arc<Semaphore>,
}

impl ScraperEngine {
    pub fn new() -> Self {
        Self {
            semaphore: ArcSwap::from_pointee(Semaphore::new(2)),
            jobs: Arc::new(Mutex::new(HashMap::new())),
            browser_sem: Arc::new(Semaphore::new(1)),
        }
    }

    pub fn catalog(&self) -> Vec<ScraperCatalogEntry> {
        super::boards::all()
            .iter()
            .map(|s| ScraperCatalogEntry {
                id: s.id().to_string(),
                display_name: s.display_name().to_string(),
                mode: match s.mode() {
                    ScraperMode::Http => "http",
                    ScraperMode::Browser => "browser",
                }
                .to_string(),
                auth: s.auth(),
                listed: s.listed(),
                requires_company: s.requires_company(),
            })
            .collect()
    }

    pub fn health(&self) -> ScraperRuntimeHealth {
        ScraperRuntimeHealth {
            mode: "in-process".to_string(),
            scrapers: self.catalog(),
            ready: true,
        }
    }

    /// Single-board core — the `amount`-cap wrapper, `ScrapeContext` build,
    /// `scraper.search`, and the reached-cap recovery. Cancels `token` when the
    /// cap is hit; touches no semaphore and no `jobs` map.
    ///
    /// `scraper` is the resolved board (or an error for an unknown board);
    /// tests inject a fake scraper through this seam.
    pub async fn run_one(
        board: &str,
        scraper: anyhow::Result<&dyn Scraper>,
        input: BoardSearchInput,
        token: CancellationToken,
        on_progress: Option<Box<dyn Fn(f32) + Send>>,
        on_item: Option<Box<dyn Fn(JobPosting) + Send>>,
        on_truncation: Option<Box<dyn Fn(String) + Send>>,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // Central item cap. The board loops stream items through `on_item` and
        // check `ctx.signal`; we count the stream here and cancel the token the
        // instant `amount` is reached, so whichever limit (page budget or item
        // cap) is hit first stops the scrape — without touching any board loop.
        let amount = (input.amount as usize).max(1);
        let streamed = Arc::new(AtomicUsize::new(0));
        let reached = Arc::new(AtomicBool::new(false));
        let kept: Arc<std::sync::Mutex<Vec<JobPosting>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));

        // Wrap the caller's `on_item` so each streamed posting is counted, kept,
        // and forwarded only while under the cap; the cap-reaching item flips
        // `reached` and cancels the token so each board's pagination stops early.
        let wrapped: Option<Box<dyn Fn(JobPosting) + Send>> = on_item.map(|inner| {
            let streamed = streamed.clone();
            let reached = reached.clone();
            let kept = kept.clone();
            let token_for_wrapper = token.clone();
            let boxed: Box<dyn Fn(JobPosting) + Send> = Box::new(move |mut item: JobPosting| {
                let n = streamed.fetch_add(1, Ordering::SeqCst);
                if n < amount {
                    // Trust assessment is attached here — the single funnel every
                    // board's streamed item passes through before it reaches the
                    // caller's `on_item` (PostingsCache + `job.stream`/`SCRAPE_ITEM`
                    // for both the manual scrape and Autopilot UIs).
                    super::trust::attach(&mut item);
                    if let Ok(mut guard) = kept.lock() {
                        guard.push(item.clone());
                    }
                    inner(item);
                    if n + 1 >= amount {
                        reached.store(true, Ordering::SeqCst);
                        token_for_wrapper.cancel();
                    }
                }
                // n >= amount → drop the item (already at the cap).
            });
            boxed
        });

        let ctx = ScrapeContext {
            signal: token,
            on_progress,
            on_item: wrapped,
            on_truncation,
        };

        let span = crate::observability::Span::begin("scrape", format!("board={board}"));
        let result = match scraper {
            Ok(scraper) => scraper.search(input, ctx).await,
            Err(e) => Err(e),
        };

        match result {
            Ok(mut items) => {
                items.truncate(amount);
                span.end_with(&format!("count={}", items.len()), true);
                Ok(items)
            }
            Err(e) => {
                // Our own target-reached cancellation: the kept items were already
                // streamed to the renderer, so recover and return them as success.
                // A real user cancel leaves `reached == false`, so it propagates the
                // error exactly as before.
                if reached.load(Ordering::SeqCst) {
                    let mut kept_items = kept
                        .lock()
                        .map(|mut g| std::mem::take(&mut *g))
                        .unwrap_or_default();
                    kept_items.truncate(amount);
                    span.end_with(&format!("count={}", kept_items.len()), true);
                    Ok(kept_items)
                } else {
                    span.end(false);
                    Err(e)
                }
            }
        }
    }

    /// Fan-out core — run multiple boards concurrently (up to 3 in parallel;
    /// browser boards are serialized via the process-wide `browser_sem`) and
    /// collect per-board results in **input order**. Tests inject fake scrapers
    /// through this seam.
    pub async fn run_boards<'s>(
        resolved: Vec<(String, anyhow::Result<&'s dyn Scraper>)>,
        input: BoardSearchInput,
        parent: CancellationToken,
        on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>,
        on_item: Option<Arc<dyn Fn(JobPosting) + Send + Sync>>,
        on_truncation: Option<Arc<dyn Fn(String, String) + Send + Sync>>,
        browser_sem: Arc<Semaphore>,
    ) -> Vec<(String, anyhow::Result<Vec<JobPosting>>)> {
        let total = resolved.len();
        let done = Arc::new(AtomicUsize::new(0));

        // Build per-board tasks as boxed futures so the closure doesn't have to
        // be higher-kinded over the scraper's lifetime (which triggers rustc's
        // FnOnce-not-general-enough error when combined with async move).
        use futures::future::BoxFuture;
        let tasks: Vec<BoxFuture<'s, (String, anyhow::Result<Vec<JobPosting>>)>> = resolved
            .into_iter()
            .map(|(name, scraper)| {
                let input = input.clone();
                let parent = parent.clone();
                let on_progress = on_progress.clone();
                let on_item = on_item.clone();
                let on_truncation = on_truncation.clone();
                let done = done.clone();
                let browser_sem = browser_sem.clone();

                let fut: BoxFuture<'s, (String, anyhow::Result<Vec<JobPosting>>)> =
                    Box::pin(async move {
                        // Acquire the browser semaphore ONLY for browser-mode boards,
                        // so HTTP boards fan out freely while browser ones serialize.
                        let _browser_permit = if scraper.as_ref().ok().map(|s| s.mode())
                            == Some(ScraperMode::Browser)
                        {
                            Some(
                                browser_sem
                                    .clone()
                                    .acquire_owned()
                                    .await
                                    .expect("browser semaphore never closes"),
                            )
                        } else {
                            None
                        };

                        let child = parent.child_token();

                        // Wrap the shared `on_item` Arc into a per-board Box.
                        let per_board_on_item: Option<Box<dyn Fn(JobPosting) + Send>> =
                            on_item.as_ref().map(|arc| {
                                let arc = arc.clone();
                                let boxed: Box<dyn Fn(JobPosting) + Send> =
                                    Box::new(move |item: JobPosting| arc(item));
                                boxed
                            });

                        // Wrap the shared truncation sink into a per-board Box that
                        // tags the reason with this board's name, so scrape_boards can
                        // attribute a partial harvest to the right BoardScrapeSummary.
                        let per_board_on_truncation: Option<Box<dyn Fn(String) + Send>> =
                            on_truncation.as_ref().map(|arc| {
                                let arc = arc.clone();
                                let name = name.clone();
                                let boxed: Box<dyn Fn(String) + Send> =
                                    Box::new(move |reason: String| arc(name.clone(), reason));
                                boxed
                            });

                        let res = Self::run_one(
                            &name,
                            scraper,
                            input,
                            child,
                            None,
                            per_board_on_item,
                            per_board_on_truncation,
                        )
                        .await;

                        // Batch progress: coarse done/total fraction after each board.
                        let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if let Some(ref cb) = on_progress {
                            cb(finished as f32 / total as f32);
                        }

                        (name, res)
                    });
                fut
            })
            .collect();

        // `.buffered` (not `.buffer_unordered`) preserves input order so that
        // `postings` is the concatenation of all boards' results in input order,
        // matching the doc comment on `scrape_boards`.
        futures::stream::iter(tasks).buffered(3).collect().await
    }

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
            |id| super::boards::get(id).ok_or_else(|| anyhow::anyhow!("Unknown board: {id}")),
        )
        .await
    }

    /// Test-only resolver seam: identical to `scrape_boards` but accepts a
    /// caller-supplied `resolve` function and an explicit `data_dir` so tests
    /// can inject fake scrapers and an isolated tempdir without touching the
    /// real `boards::get` registry or `crate::platform::config::data_dir()`.
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
        let (parent, we_minted) = {
            let mut jobs = self.jobs.lock().await;
            if let Some(existing) = jobs.get(&job_id) {
                (existing.clone(), false)
            } else {
                let token = CancellationToken::new();
                jobs.insert(job_id.clone(), token.clone());
                (token, true)
            }
        };

        // Dedupe (first-seen order) + truncate to MAX_BOARDS_PER_BATCH so a
        // crafted payload with thousands of valid ids cannot build thousands of
        // futures and drive ban amplification on the user's own sessions.
        let boards_deduped: Vec<&String> = {
            let mut seen = std::collections::HashSet::new();
            boards
                .iter()
                .filter(|id| seen.insert(id.as_str()))
                .take(MAX_BOARDS_PER_BATCH)
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
                    let no_session = super::board_login::load_cookies(data_dir, &id).is_empty()
                        || super::board_login::session_age_ms(data_dir, &id).is_none()
                        || super::board_login::session_is_stale(data_dir, &id);
                    if no_session {
                        return Some("needs-login");
                    }
                }
                // Skip 2: ATS board that requires a company slug but none usable.
                // Treats whitespace-only entries (e.g. [" ", "\t"]) the same as
                // an empty list — they are trimmed-and-dropped by ATS scrapers.
                if s.requires_company() && !has_usable_company {
                    return Some("needs-company");
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
                });
            } else {
                name_to_idx.insert(id.clone(), idx);
                runnable.push((id, scraper));
            }
        }

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

        let results = Self::run_boards(
            runnable,
            input,
            parent.clone(),
            on_progress,
            on_item,
            Some(truncation_sink),
            self.browser_sem.clone(),
        )
        .await;

        // F5 — only remove the token slot when we minted it. A pre-registered
        // token (Autopilot pre-registers its own) is managed by the caller.
        if we_minted {
            self.jobs.lock().await.remove(&job_id);
        }

        let mut all_postings: Vec<JobPosting> = Vec::new();
        // F6 — true only when at least one board returned a non-empty Ok.
        let mut any_recovered_items = false;

        // Fill run results back into their original positions.
        for (board, res) in results {
            let idx = name_to_idx[&board];
            match res {
                Ok(postings) => {
                    if !postings.is_empty() {
                        any_recovered_items = true;
                    }
                    // A partial harvest (paginated board that failed on a later page)
                    // is surfaced here so it is not indistinguishable from a complete
                    // run; a board that completed its pages has no map entry.
                    let truncated = truncations.lock().ok().and_then(|mut m| m.remove(&board));
                    slot_summaries[idx] = Some(BoardScrapeSummary {
                        board,
                        count: postings.len(),
                        error: None,
                        skipped: None,
                        truncated,
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
                    });
                }
            }
        }

        // Flatten in input order — every slot is now Some (skips were filled above,
        // run results were filled by name_to_idx lookup).
        let summaries: Vec<BoardScrapeSummary> = slot_summaries.into_iter().flatten().collect();

        // F6 — return Err only when the user cancelled AND no items were recovered.
        // A board returning Ok([]) after observing cancellation is not a success.
        if parent.is_cancelled() && !any_recovered_items {
            return Err(anyhow::anyhow!("scrape cancelled"));
        }

        Ok((all_postings, summaries))
    }

    /// Signal cancellation to a running job by id. No-op if the id is unknown.
    pub async fn cancel(&self, job_id: &str) {
        let mut jobs = self.jobs.lock().await;
        if let Some(token) = jobs.remove(job_id) {
            token.cancel();
        }
    }

    /// Register a job token so it can be reached by `cancel(job_id)`. Used by
    /// the apply flow, which manages its own token outside `scrape_boards`.
    pub async fn register_token(&self, job_id: &str, token: CancellationToken) {
        self.jobs.lock().await.insert(job_id.to_string(), token);
    }

    /// Remove a registered token. Idempotent.
    pub async fn unregister_token(&self, job_id: &str) {
        self.jobs.lock().await.remove(job_id);
    }

    /// Resize the concurrency limit. Already-running jobs keep their permits
    /// until they finish; new jobs are bounded by the new value (min 1).
    pub fn set_concurrency(&self, n: usize) {
        self.semaphore.store(Arc::new(Semaphore::new(n.max(1))));
    }
}

impl Default for ScraperEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test;
