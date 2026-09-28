//! Single-board (`run_one`) and multi-board fan-out (`run_boards`) cores —
//! the seams `scrape_boards` (see [`super::scrape_boards`]) and the engine
//! tests inject fake scrapers through. Split out of `mod.rs` (issue #1280).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use futures::StreamExt as _;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use crate::scraping::types::{BoardSearchInput, JobPosting, ScrapeContext, Scraper, ScraperMode};

use super::{KeepItemByBoardFn, KeepItemFn, ScraperEngine};

impl ScraperEngine {
    /// Single-board core — the `amount`-cap wrapper, `ScrapeContext` build,
    /// `scraper.search`, and the reached-cap recovery. Cancels `token` when the
    /// cap is hit; touches no semaphore and no `jobs` map.
    ///
    /// `scraper` is the resolved board (or an error for an unknown board);
    /// tests inject a fake scraper through this seam.
    ///
    /// One parameter per independent callback/seam (progress, item, truncation,
    /// note, keep-filter) — each is optional and semantically distinct, so
    /// bundling them into a struct would obscure the call sites more than it
    /// clarifies; matches the `#[allow(...)]` precedent used elsewhere in this
    /// codebase for similar low-level fan-out functions.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_one(
        board: &str,
        scraper: anyhow::Result<&dyn Scraper>,
        input: BoardSearchInput,
        token: CancellationToken,
        on_progress: Option<Box<dyn Fn(f32) + Send>>,
        on_item: Option<Box<dyn Fn(JobPosting) + Send>>,
        on_truncation: Option<Box<dyn Fn(String) + Send>>,
        on_note: Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>,
        // See `KeepItemFn` for the cap/filter ordering invariant this implements.
        keep_item: Option<Box<KeepItemFn>>,
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

        // Only exercised when a live on_item sink is ALSO wired (the gate lives
        // inside its wrapper below). See `KeepItemFn`.
        let has_active_filter = keep_item.is_some() && on_item.is_some();

        // Wrap the caller's `on_item` so each streamed posting is counted, kept,
        // and forwarded only while under the cap; the cap-reaching item flips
        // `reached` and cancels the token so each board's pagination stops early.
        let wrapped: Option<Box<dyn Fn(JobPosting) + Send>> = on_item.map(|inner| {
            let streamed = streamed.clone();
            let reached = reached.clone();
            let kept = kept.clone();
            let token_for_wrapper = token.clone();
            let boxed: Box<dyn Fn(JobPosting) + Send> = Box::new(move |mut item: JobPosting| {
                // Gate first — see `KeepItemFn`.
                if let Some(ref keep) = keep_item {
                    if !keep(&item) {
                        return;
                    }
                }
                let n = streamed.fetch_add(1, Ordering::SeqCst);
                if n < amount {
                    // Trust assessment is attached here — the single funnel every
                    // board's streamed item passes through before it reaches the
                    // caller's `on_item` (PostingsCache + `job.stream`/`SCRAPE_ITEM`
                    // for both the manual scrape and Autopilot UIs).
                    crate::scraping::trust::attach(&mut item);
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
            on_note,
        };

        let span = crate::observability::Span::begin("scrape", format!("board={board}"));
        let result = match scraper {
            Ok(scraper) => scraper.search(input, ctx).await,
            Err(e) => Err(e),
        };

        match result {
            Ok(mut items) => {
                let out = if has_active_filter {
                    // `kept`, not a raw truncate — see `KeepItemFn`.
                    match kept.lock() {
                        Ok(mut g) => std::mem::take(&mut *g),
                        Err(_) => {
                            // A poisoned mutex here silently drops this board's
                            // WHOLE result (empty Vec) — that must be visible, not
                            // a quiet zero indistinguishable from a clean run.
                            log::warn!(
                                "[scrape] board '{board}' kept-items mutex poisoned; \
                                 returning empty result"
                            );
                            Vec::new()
                        }
                    }
                } else {
                    items.truncate(amount);
                    items
                };
                span.end_with(&format!("count={}", out.len()), true);
                Ok(out)
            }
            Err(e) => {
                // Our own target-reached cancellation: the kept items were already
                // streamed to the renderer, so recover and return them as success.
                // A real user cancel leaves `reached == false`, so it propagates the
                // error exactly as before.
                if reached.load(Ordering::SeqCst) {
                    let mut kept_items = match kept.lock() {
                        Ok(mut g) => std::mem::take(&mut *g),
                        Err(_) => {
                            // Same visibility concern as the Ok(items) arm above:
                            // a poisoned mutex must not silently present as "0
                            // items recovered" with no trace.
                            log::warn!(
                                "[scrape] board '{board}' kept-items mutex poisoned on \
                                 cap-recovery; returning empty result"
                            );
                            Vec::new()
                        }
                    };
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
    ///
    /// See `run_one`'s doc note on the per-callback parameter shape.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_boards<'s>(
        resolved: Vec<(String, anyhow::Result<&'s dyn Scraper>)>,
        input: BoardSearchInput,
        parent: CancellationToken,
        on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>,
        on_item: Option<Arc<dyn Fn(JobPosting) + Send + Sync>>,
        on_truncation: Option<Arc<dyn Fn(String, String) + Send + Sync>>,
        on_note: Option<Arc<dyn Fn(String, String) + Send + Sync>>,
        // `None` when no location filter applies to this run. See `KeepItemFn`.
        keep_item: Option<Arc<KeepItemByBoardFn>>,
        browser_sem: Arc<Semaphore>,
        // Per-board company-slug override, keyed by the same board id used as
        // this fn's `name` (not `Scraper::id()`). `run_boards` stays seed-
        // agnostic — it only applies a caller-provided map; the caller
        // (`scrape_boards_with_resolver`) is what actually consults `ats_seed`.
        seeded_companies: &HashMap<String, Vec<String>>,
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
                let mut input = input.clone();
                if let Some(slugs) = seeded_companies.get(&name) {
                    input.companies = slugs.clone();
                }
                let parent = parent.clone();
                let on_progress = on_progress.clone();
                let on_item = on_item.clone();
                let on_truncation = on_truncation.clone();
                let on_note = on_note.clone();
                let keep_item = keep_item.clone();
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

                        // Same per-board tagging for the informational location-policy
                        // note channel; kept as an `Arc` (not a `Box`) because the
                        // aggregator forwards it to a sub-provider held across `.await`.
                        let per_board_on_note: Option<Arc<dyn Fn(String) + Send + Sync>> =
                            on_note.as_ref().map(|arc| {
                                let arc = arc.clone();
                                let name = name.clone();
                                let wrapped: Arc<dyn Fn(String) + Send + Sync> =
                                    Arc::new(move |note: String| arc(name.clone(), note));
                                wrapped
                            });

                        // Bind to this board's name — see `KeepItemFn`.
                        let per_board_keep_item: Option<Box<KeepItemFn>> =
                            keep_item.as_ref().map(|arc| {
                                let arc = arc.clone();
                                let name = name.clone();
                                let boxed: Box<KeepItemFn> =
                                    Box::new(move |item: &JobPosting| arc(&name, item));
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
                            per_board_on_note,
                            per_board_keep_item,
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
}
