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
}

#[derive(Debug, Clone)]
pub struct ScraperRuntimeHealth {
    pub mode: String,
    pub scrapers: Vec<ScraperCatalogEntry>,
    pub ready: bool,
}

/// Per-board outcome reported by [`ScraperEngine::scrape_boards`].
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardScrapeSummary {
    pub board: String,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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
            let boxed: Box<dyn Fn(JobPosting) + Send> = Box::new(move |item: JobPosting| {
                let n = streamed.fetch_add(1, Ordering::SeqCst);
                if n < amount {
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

                        let res =
                            Self::run_one(&name, scraper, input, child, None, per_board_on_item)
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
        self.scrape_boards_with_resolver(boards, input, job_id, on_progress, on_item, |id| {
            super::boards::get(id).ok_or_else(|| anyhow::anyhow!("Unknown board: {id}"))
        })
        .await
    }

    /// Test-only resolver seam: identical to `scrape_boards` but accepts a
    /// caller-supplied `resolve` function so tests can inject fake scrapers
    /// without touching the real `boards::get` registry.
    #[doc(hidden)]
    pub(crate) async fn scrape_boards_with_resolver<F>(
        &self,
        boards: &[String],
        input: BoardSearchInput,
        job_id: String,
        on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>,
        on_item: Option<Arc<dyn Fn(JobPosting) + Send + Sync>>,
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

        let results = Self::run_boards(
            resolved,
            input,
            parent.clone(),
            on_progress,
            on_item,
            self.browser_sem.clone(),
        )
        .await;

        // F5 — only remove the token slot when we minted it. A pre-registered
        // token (Autopilot pre-registers its own) is managed by the caller.
        if we_minted {
            self.jobs.lock().await.remove(&job_id);
        }

        let mut all_postings: Vec<JobPosting> = Vec::new();
        let mut summaries: Vec<BoardScrapeSummary> = Vec::new();
        // F6 — true only when at least one board returned a non-empty Ok.
        let mut any_recovered_items = false;

        for (board, res) in results {
            match res {
                Ok(postings) => {
                    if !postings.is_empty() {
                        any_recovered_items = true;
                    }
                    summaries.push(BoardScrapeSummary {
                        board,
                        count: postings.len(),
                        error: None,
                    });
                    all_postings.extend(postings);
                }
                Err(e) => {
                    summaries.push(BoardScrapeSummary {
                        board,
                        count: 0,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

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
