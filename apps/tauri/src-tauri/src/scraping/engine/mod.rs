/// ScraperEngine — in-process scraper orchestrator.
///
/// Uses interior mutability so `Arc<ScraperEngine>` can be cloned into Tauri
/// commands and scrape jobs run concurrently (bounded by `semaphore`) without
/// serializing on an outer mutex.
use super::types::{
    AuthRequirement, BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode,
};
use arc_swap::ArcSwap;
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

pub struct ScraperEngine {
    /// Bounded concurrency — swapped on `set_concurrency`. Holding an
    /// owned permit for the duration of a scrape lets us shrink the limit
    /// without cancelling running work.
    semaphore: ArcSwap<Semaphore>,
    /// Active jobs keyed by job_id. Used by `cancel(job_id)`.
    jobs: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl ScraperEngine {
    pub fn new() -> Self {
        Self {
            semaphore: ArcSwap::from_pointee(Semaphore::new(2)),
            jobs: Arc::new(Mutex::new(HashMap::new())),
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

    pub async fn scrape_board(
        &self,
        board: &str,
        input: BoardSearchInput,
        job_id: String,
        on_progress: Option<Box<dyn Fn(f32) + Send>>,
        on_item: Option<Box<dyn Fn(JobPosting) + Send>>,
    ) -> anyhow::Result<Vec<JobPosting>> {
        let scraper =
            super::boards::get(board).ok_or_else(|| anyhow::anyhow!("Unknown board: {}", board));
        self.run_search(board, scraper, input, job_id, on_progress, on_item)
            .await
    }

    /// Core scrape path with the central `amount` cap enforced here (no board
    /// loop is touched). `scraper` is the resolved board (or an error to report
    /// for an unknown board); tests inject a fake scraper through this seam.
    async fn run_search(
        &self,
        board: &str,
        scraper: anyhow::Result<&dyn Scraper>,
        input: BoardSearchInput,
        job_id: String,
        on_progress: Option<Box<dyn Fn(f32) + Send>>,
        on_item: Option<Box<dyn Fn(JobPosting) + Send>>,
    ) -> anyhow::Result<Vec<JobPosting>> {
        // Bound concurrency — acquire owned permit so the limit can be reduced
        // mid-flight without aborting this job.
        let sem = self.semaphore.load_full();
        let _permit = sem
            .acquire_owned()
            .await
            .map_err(|_| anyhow::anyhow!("scraper engine semaphore closed"))?;

        // Cancellation token under job_id so `cancel(job_id)` works. Reuse a
        // token a caller pre-registered for this job (e.g. autopilot owns one
        // for the whole run, spanning scrape + post-processing) rather than
        // overwriting it; otherwise mint a fresh one.
        let token = {
            let mut jobs = self.jobs.lock().await;
            jobs.entry(job_id.clone())
                .or_insert_with(CancellationToken::new)
                .clone()
        };

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

        let span =
            crate::observability::Span::begin("scrape", format!("board={board} job={job_id}"));
        let result = match scraper {
            Ok(scraper) => scraper.search(input, ctx).await,
            Err(e) => Err(e),
        };

        // Always clear the token slot, even on error.
        self.jobs.lock().await.remove(&job_id);

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

    /// Signal cancellation to a running job by id. No-op if the id is unknown.
    pub async fn cancel(&self, job_id: &str) {
        let mut jobs = self.jobs.lock().await;
        if let Some(token) = jobs.remove(job_id) {
            token.cancel();
        }
    }

    /// Register a job token so it can be reached by `cancel(job_id)`. Used by
    /// the apply flow, which manages its own token outside `scrape_board`.
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
