/// ScraperEngine — in-process scraper orchestrator.
///
/// Uses interior mutability so `Arc<ScraperEngine>` can be cloned into Tauri
/// commands and scrape jobs run concurrently (bounded by `semaphore`) without
/// serializing on an outer mutex.
use super::types::{AuthRequirement, JobPosting, ScraperMode};
use arc_swap::ArcSwap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use crate::jobs::cancel::CancelRegistry;

/// `pub(crate)` (not private to `engine`) so the L3 hard-constraint pass in
/// `commands::match_resume::constraints` reuses this module's three-valued
/// [`location_filter::location_verdict`] instead of forking a second place-name
/// matcher with its own remote-marker list and exonym table.
pub(crate) mod location_filter;
/// Work-type sibling of [`location_filter`] — see its module doc for shape and
/// conservatism, and for why it does not read `location_filter::REMOTE_MARKERS`.
pub(crate) mod work_type_filter;

/// Per-item keep predicate for a single board (already bound to that board's
/// name where relevant) — `true` = keep. Composes trust PR F's central
/// location filter AND the work-type filter into ONE predicate (never two —
/// `run_one`/`run_boards` accept a single `KeepItemFn`); `None` only when
/// NEITHER filter applies to this run.
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
    /// Whether the board narrows results by the requested work type
    /// server-side. When `false`, the engine post-filters this board's results
    /// on device, keeping every posting whose work type is undeclared. Drives
    /// the picker's per-board indicator, same as [`Self::supports_location`].
    #[serde(rename = "supportsWorkType")]
    pub supports_work_type: bool,
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
    /// Zero or more informational notes about a policy the board/engine applied
    /// that the user did not explicitly request (NOT a failure — `count` is
    /// still authoritative). Widened from a single `Option<String>` (trust PR
    /// F/H) because up to THREE independent honesty signals can legitimately
    /// coexist on one board in one run — a board-native note (e.g. an ATS
    /// board's own `slugs-invalid:<n>`), the central location post-filter, and
    /// the central work-type post-filter — and a single slot silently dropped
    /// whichever one lost. Precedence/order when more than one applies: the
    /// board's own note first, then `location-filtered`, then
    /// `work-type-filtered`. Possible entries:
    /// - `"guessed-market:<cc>"` — no country was supplied, so the `<cc>` market
    ///   was guessed and returned an authoritative result set; set a country for
    ///   deterministic results.
    /// - `"broadened:<cc>"` — a sparse city search was widened country-wide within
    ///   the `<cc>` market.
    /// - `"location-filtered:<n>"` / `"work-type-filtered:<n>"` — see
    ///   [`location_filter`]/[`work_type_filter`] module docs.
    ///
    /// `<cc>` is an ISO country code; an entry never carries the raw location
    /// (free-text PII). `#[serde(default)]` so pre-existing records (which
    /// carried a singular `note` field under a different JSON key) deserialize
    /// as an empty list rather than failing — this is a display-only,
    /// informational field, so losing a historical entry on an old record is an
    /// acceptable, already-established trade-off (same as `truncated`/`health`
    /// above).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Cross-run reliability for this board, folded from every previous run by
    /// [`super::board_health`] and attached here so a chip can distinguish "this
    /// board found nothing today" from "this board has been broken since
    /// Tuesday". `None` when the engine has no health store wired (tests, and
    /// any run whose diagnostics write failed — a broken diagnostic must never
    /// break the scrape).
    ///
    /// `#[serde(default)]` is load-bearing: `lastRunSummaries` records persisted
    /// before this field existed (and every backup bundle containing them) must
    /// still deserialize.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<super::board_health::BoardHealth>,
}

/// Maximum number of distinct boards processed per `scrape_boards` call —
/// bound to the number of registered scrapers ([`super::boards::all`]) rather
/// than a fixed number.
///
/// The registry size is the real ceiling on distinct board dispatches a
/// request can ever need, so a legitimate "search every catalog board"
/// selection is never truncated as the catalog grows (no schema/engine edit
/// needed when a board is added — see the Future-Proof Extensibility rule).
/// This still defends CWE-770: dedup (below) collapses a crafted payload with
/// thousands of (even valid) board ids to at most one dispatch per registered
/// board, so it cannot build thousands of concurrent futures or drive ban
/// amplification against the user's own authenticated sessions.
fn max_boards_per_batch() -> usize {
    super::boards::all().len()
}

pub struct ScraperEngine {
    /// Bounded concurrency — swapped on `set_concurrency`. Holding an
    /// owned permit for the duration of a scrape lets us shrink the limit
    /// without cancelling running work.
    semaphore: ArcSwap<Semaphore>,
    /// Active jobs keyed by job_id. Used by `cancel(job_id)`.
    ///
    /// Shared, not owned: this is the app-wide [`CancelRegistry`] (L1
    /// `jobs::cancel`), which agent runs — and, from Phase 3, pipeline runs —
    /// also register against, so ONE `jobs_cancel` reaches every job kind. The
    /// engine keeps the same three verbs it always had; they are now thin
    /// delegations.
    jobs: Arc<CancelRegistry>,
    /// Process-wide browser semaphore — one chromiumoxide session at a time.
    /// Shared across concurrent `scrape_boards` calls so two jobs that each
    /// include a browser board cannot spin up two headless instances at once.
    browser_sem: Arc<Semaphore>,
    /// Per-board reliability history (Track B1). `None` until `shell/state.rs`
    /// hands the opened store in — the engine is constructed before the L1
    /// stores are, and tests construct it with no store at all, so every use is
    /// optional and a missing store simply means "no health on the summaries".
    /// `ArcSwapOption` (not a constructor arg) so the already-`Arc`ed, already-
    /// managed engine can be given the store after the fact.
    health: arc_swap::ArcSwapOption<super::board_health::BoardHealthStore>,
}

impl ScraperEngine {
    pub fn new() -> Self {
        Self {
            semaphore: ArcSwap::from_pointee(Semaphore::new(2)),
            jobs: Arc::new(CancelRegistry::new()),
            browser_sem: Arc::new(Semaphore::new(1)),
            health: arc_swap::ArcSwapOption::empty(),
        }
    }

    /// Attach the per-board reliability store, after which every multi-board
    /// scrape folds its summaries into it and attaches the resulting health to
    /// each [`BoardScrapeSummary`]. Called once from `shell/state.rs`; idempotent
    /// (a second call replaces the handle).
    pub fn set_health_store(&self, store: Arc<super::board_health::BoardHealthStore>) {
        self.health.store(Some(store));
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
                supports_work_type: s.supports_work_type(),
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
}

mod dedup;
mod health_record;
mod run;
mod scrape_boards;

impl ScraperEngine {
    /// Signal cancellation to a running job by id. No-op if the id is unknown.
    ///
    /// Thin delegation to the shared [`CancelRegistry`], which owns the
    /// cancel-in-place semantics and the "whoever registered removes it"
    /// ownership rule — see [`CancelRegistry::cancel`] for both, and
    /// [`Self::cancel_registry`] for why the map is no longer the engine's.
    ///
    /// The owners that remove their own slot are unchanged:
    /// `commands::scrape::scrape_boards` (early cancelled return and after the
    /// engine call), `commands::autopilot::autopilot_run` (scrape-Err,
    /// cancelled, and success paths), and
    /// `commands::resume_pipeline::resume_pipeline_run` (every validation
    /// failure, plus the spawned task's own exit) — while an engine-minted
    /// slot is removed by the `we_minted` branch.
    pub async fn cancel(&self, job_id: &str) {
        self.jobs.cancel(job_id).await;
    }

    /// Register a job token so it can be reached by `cancel(job_id)`. Used by
    /// callers that manage their own token outside `scrape_boards`.
    pub async fn register_token(&self, job_id: &str, token: CancellationToken) {
        self.jobs.register(job_id, token).await;
    }

    /// Remove a registered token. Idempotent.
    pub async fn unregister_token(&self, job_id: &str) {
        self.jobs.unregister(job_id).await;
    }

    /// The shared cancel registry this engine dispatches through, so a caller
    /// with no scraping concern (agent runs; pipeline runs from Phase 3) can
    /// register against the SAME map `jobs_cancel` reaches without borrowing the
    /// engine. `shell/state.rs` manages this clone as app state.
    pub fn cancel_registry(&self) -> Arc<CancelRegistry> {
        self.jobs.clone()
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
mod tests;
