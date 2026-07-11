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

mod location_filter;

/// Per-item keep predicate for a single board (already bound to that board's
/// name where relevant) — `true` = keep. Trust PR F's central location filter;
/// `None` when no location filter applies to this run.
///
/// **Cap/filter ordering invariant (canonical explanation — HIGH-1):** `run_one`
/// checks this predicate BEFORE its item cap counts/cancels, so a filtered item
/// never increments the cap and never triggers cap-cancel — a board keeps
/// paginating until `amount` MATCHING items are found, not `amount` raw ones.
/// When active, `run_one` also returns the tracked kept set (not a raw-Vec
/// truncate) as the final result, since raw order can otherwise keep an early
/// mismatch while dropping a later real match. See `run_one`/`run_boards` for
/// the wiring; every other mention below just points back here.
type KeepItemFn = dyn Fn(&JobPosting) -> bool + Send;

/// Board-name-aware keep predicate shared across every board in a
/// `run_boards` fan-out; `run_boards` binds it to each board's name into a
/// [`KeepItemFn`] before passing it to `run_one`.
type KeepItemByBoardFn = dyn Fn(&str, &JobPosting) -> bool + Send + Sync;

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
    /// Whether the board narrows results by the requested location server-side.
    /// When `false`, the engine conservatively post-filters this board's results
    /// against the requested location (drops only clear city mismatches; never
    /// remote/unknown-location rows). Drives the picker's per-board indicator.
    #[serde(rename = "supportsLocation")]
    pub supports_location: bool,
    /// Curated company display names this company-scoped ATS board will query
    /// when the user supplies none (from `boards::ats_seed::by_ats`, source
    /// order). Empty for boards without a curated seed.
    #[serde(rename = "seededCompanies")]
    pub seeded_companies: Vec<String>,
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
    /// Set when a board applied a location policy the user did not explicitly
    /// request (informational, NOT a failure — `count` is still authoritative):
    /// - `"guessed-market:<cc>"` — no country was supplied, so the `<cc>` market
    ///   was guessed and returned an authoritative result set; set a country for
    ///   deterministic results.
    /// - `"broadened:<cc>"` — a sparse city search was widened country-wide within
    ///   the `<cc>` market.
    ///
    /// `<cc>` is an ISO country code; the note never carries the raw location
    /// (free-text PII). Serde-optional so pre-existing records deserialize as `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
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
                supports_location: s.supports_location(),
                seeded_companies: super::boards::ats_seed::by_ats(s.id())
                    .map(|e| e.company.to_string())
                    .collect(),
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
                // Exception: a board with a curated `ats_seed` entry still runs —
                // the engine auto-populates `input.companies` from the seed right
                // before `run_boards` (see `seeded_companies` below), so skipping
                // here would strand those seeded slugs unused. Keyed on `s.id()`
                // (the Scraper trait id the seed's `ats` field matches), NOT the
                // caller-supplied board-list string (`id`), which can differ.
                if s.requires_company()
                    && !has_usable_company
                    && super::boards::ats_seed::by_ats(s.id()).next().is_none()
                {
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
                    note: None,
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

        // Per-board informational location-policy notes (aggregator guessed-market /
        // sparse city broadened country-wide). Same board-name-keyed collection as
        // truncations; folded into `BoardScrapeSummary.note` below. Empty for a run
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
        let location_drops: Arc<std::sync::Mutex<HashMap<String, usize>>> =
            Arc::new(std::sync::Mutex::new(HashMap::new()));
        let keep_item: Option<Arc<KeepItemByBoardFn>> = requested_location.clone().map(|req| {
            let non_loc = non_location_boards.clone();
            let drops = location_drops.clone();
            let f: Arc<KeepItemByBoardFn> = Arc::new(move |board: &str, item: &JobPosting| {
                if !non_loc.contains(board) {
                    return true; // board supports location — never filtered here
                }
                if location_filter::location_mismatch(item, &req) {
                    if let Ok(mut guard) = drops.lock() {
                        *guard.entry(board.to_string()).or_insert(0) += 1;
                    }
                    false
                } else {
                    true
                }
            });
            f
        });

        // Auto-populate ATS boards' company filter from the curated `ats_seed`
        // table when the user left the global company field blank — gives the
        // company-scoped ATS scrapers real slugs to fetch without hand-typed
        // input. Only when `companies` is globally empty so an explicit user
        // list always wins (never overridden). Keyed on the caller-supplied
        // board-list id (`run_boards`'s `name` param), matching how `runnable`
        // is keyed — NOT on `s.id()` (used above for the seed lookup itself).
        let mut seeded_companies: HashMap<String, Vec<String>> = HashMap::new();
        if !has_usable_company {
            for (id, scraper) in &runnable {
                let Ok(s) = scraper else { continue };
                if !s.requires_company() {
                    continue;
                }
                let slugs: Vec<String> = super::boards::ats_seed::by_ats(s.id())
                    .map(|e| e.slug.to_string())
                    .collect();
                if !slugs.is_empty() {
                    seeded_companies.insert(id.clone(), slugs);
                }
            }
        }

        let results = Self::run_boards(
            runnable,
            input,
            parent.clone(),
            on_progress,
            on_item,
            Some(truncation_sink),
            Some(note_sink),
            keep_item,
            self.browser_sem.clone(),
            &seeded_companies,
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
                    // Central location post-filter (trust PR F, see `location_filter`)
                    // — a SAFETY NET here, not the primary filter: a no-op when the
                    // live gate already ran (see `KeepItemFn`), the only filtering
                    // pass otherwise (callers with no live on_item, e.g. tests).
                    let (postings, post_hoc_dropped) = match &requested_location {
                        Some(req) if non_location_boards.contains(&board) => {
                            location_filter::filter_postings(postings, req)
                        }
                        _ => (postings, 0),
                    };
                    if !postings.is_empty() {
                        any_recovered_items = true;
                    }
                    // A partial harvest (paginated board that failed on a later page)
                    // is surfaced here so it is not indistinguishable from a complete
                    // run; a board that completed its pages has no map entry.
                    let truncated = truncations.lock().ok().and_then(|mut m| m.remove(&board));
                    let mut note = notes.lock().ok().and_then(|mut m| m.remove(&board));
                    // Combine the live gate's drop count with this pass's.
                    let live_dropped = location_drops
                        .lock()
                        .ok()
                        .and_then(|mut m| m.remove(&board))
                        .unwrap_or(0);
                    let dropped = live_dropped + post_hoc_dropped;
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
                    if non_location_boards.contains(&board) && requested_location.is_some() {
                        // Surface via the existing note side-channel using the PR D
                        // `kind:value` grammar (cf. `broadened:<cc>`). Count only —
                        // never the raw location text (free-text PII). Precedence: a
                        // board-native note (e.g. an ATS board's `slugs-invalid:<n>`/
                        // `rows-dropped:<n>`, trust-H) wins — `get_or_insert_with` only
                        // fills an empty slot, so `location-filtered` never clobbers a
                        // message the board already reported this run.
                        note.get_or_insert_with(|| format!("location-filtered:{dropped}"));
                    }
                    slot_summaries[idx] = Some(BoardScrapeSummary {
                        board,
                        count: postings.len(),
                        error: None,
                        skipped: None,
                        truncated,
                        note,
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
                        note: None,
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

/// Collapse cross-source duplicate postings into one, keyed by the app-wide
/// [`canonical_job_key`](super::boards::common::canonical_job_key). Boards run
/// independently, so a job surfaced by two of them lands in the concatenated
/// result as separate rows; this is the single cross-board dedup pass
/// (trust-program PR E, stage 1).
///
/// **Survivor policy — field-level upgrade, incumbent identity kept.** The
/// FIRST-seen posting for a key is the incumbent and stays in its original slot
/// (order-preserving, stable) — its `id`/`url`/`source` (board attribution) are
/// NEVER overwritten by a later duplicate, only two things are merged in from
/// each challenger:
///   - `description` — upgraded when the challenger's is longer (a fuller
///     description scores and reads better, and aggregator snippets are often
///     truncated — trust-audit root cause 6 — so a board returning full text
///     should win the description regardless of which came first);
///   - `extra` — unioned key-by-key: a non-empty (non-null, non-`""`) incumbent
///     value is kept as-is; only a key the incumbent lacks (or holds
///     null/empty for) is filled from the challenger.
///
/// This mirrors the within-batch upgrade step in `autopilot::merge_found_jobs`
/// and is deliberately NOT a whole-struct replace: a full swap would silently
/// discard every field only the incumbent had — concretely, Adzuna rows carry
/// `extra["salaryMin"/"salaryMax"/"salaryCurrency"]` (consumed by
/// usePostingActions/TailorFlow/ApplicationDetailPage) that most direct boards
/// don't, so a direct board winning purely on description length must not
/// delete the salary.
///
/// Per-board `BoardScrapeSummary.count`s are intentionally NOT adjusted here:
/// each describes what that board returned (as-fetched), so after this pass the
/// per-board counts may not sum to the deduped total — that difference IS the
/// cross-source overlap, logged by the caller.
///
/// Pure (no I/O) so the survivor policy is directly unit-testable.
fn dedup_cross_source(postings: Vec<JobPosting>) -> Vec<JobPosting> {
    let mut index_of: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<JobPosting> = Vec::with_capacity(postings.len());
    for p in postings {
        let key = super::boards::common::canonical_job_key(&p.url, &p.title, &p.company);
        match index_of.get(&key) {
            Some(&i) => {
                // Field-level upgrade only — incumbent id/url/source (board
                // attribution) are left untouched.
                if desc_len(&p) > desc_len(&out[i]) {
                    out[i].description = p.description;
                }
                merge_extra(&mut out[i].extra, p.extra);
            }
            None => {
                index_of.insert(key, out.len());
                out.push(p);
            }
        }
    }
    out
}

/// Byte length of a posting's description (`None`/absent → 0). The description-
/// upgrade tiebreak in [`dedup_cross_source`].
fn desc_len(p: &JobPosting) -> usize {
    p.description.as_deref().map(str::len).unwrap_or(0)
}

/// Union a challenger posting's `extra` map into the incumbent's, in place. A
/// key the incumbent already holds a non-empty value for (anything but
/// JSON `null` or `""`) is left untouched; a key the incumbent lacks, or holds
/// null/empty for, is filled from the challenger. Never removes an incumbent
/// key the challenger doesn't have.
///
/// Unions arbitrary extra keys; the TS mirror
/// (`features/jobs/lib/merge-postings.ts` `collapseDuplicate`) fills a FIXED
/// field list instead — any NEW key a board writes into `JobPosting.extra`
/// must be added to that TS fill-list too (lockstep pair).
fn merge_extra(
    incumbent: &mut HashMap<String, serde_json::Value>,
    challenger: HashMap<String, serde_json::Value>,
) {
    for (k, v) in challenger {
        let incumbent_is_empty = incumbent
            .get(&k)
            .map(|existing| existing.is_null() || existing.as_str() == Some(""))
            .unwrap_or(true);
        if incumbent_is_empty {
            incumbent.insert(k, v);
        }
    }
}

#[cfg(test)]
mod test;
