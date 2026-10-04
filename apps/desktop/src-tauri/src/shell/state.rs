//! Managed application state, built during `setup`: the persistent user-data stores
//! (each registered for factory reset), the process-local state, and the stores that
//! must follow it. Split out of `lib.rs` for R8 (issue #1280). The three groups run
//! in the order they always did — the `ResetRegistry` labels are pinned to that order
//! by `MANAGE_RESETTABLE_LABELS`, and the scraper engine is handed the board-health
//! store only once that store is open.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{App, Manager};

use crate::autopilot::AutopilotStore;
use crate::commands::privacy::{manage_resettable, ResetRegistry};
use crate::credentials::CredentialStore;
use crate::data_store::Resettable;
use crate::error::AppResult;
use crate::jobs::JobTracker;
use crate::postings::{InteractionStore, PostingsCache};
use crate::scraping::ScraperEngine;
use crate::updater::UpdaterState;
use crate::{
    ai_config, ai_generations, applications, contact_profile, dedup, discovered, documents,
    email_watch, job_preferences, limits, referrals, spend,
};

/// Register `opened` for factory reset under `label`, or log the non-fatal failure
/// to open it. Only the path-free `code()` of the error is logged: a storage error
/// can embed the data-dir path, and the repo path-privacy rule extends to logs.
fn manage_opened<T: Resettable + Send + Sync + 'static>(
    app: &App,
    registry: &mut ResetRegistry,
    label: &'static str,
    what: &str,
    opened: AppResult<T>,
) {
    match opened {
        Ok(store) => manage_resettable(app, registry, label, store),
        Err(e) => log::warn!("[setup] {what} failed to open (non-fatal): {}", e.code()),
    }
}

/// The persistent user-data stores opened first, each wiped on factory reset.
pub(super) fn manage_user_stores(app: &App, registry: &mut ResetRegistry, data_dir: &PathBuf) {
    // Persistent user-data stores are managed via `manage_resettable`, which
    // also registers each one into the `ResetRegistry` so `privacy_reset_app`
    // wipes it on factory reset — no hand-maintained clear list.
    manage_resettable(
        app,
        registry,
        "autopilots",
        Arc::new(Mutex::new(AutopilotStore::new(data_dir))),
    );
    manage_resettable(
        app,
        registry,
        "credentials",
        Mutex::new(CredentialStore::new(data_dir)),
    );
    manage_opened(
        app,
        registry,
        "documents",
        "document store",
        documents::DocumentStore::open(data_dir),
    );
    manage_opened(
        app,
        registry,
        "ai_generations",
        "ai generations store",
        ai_generations::AiGenerationStore::open(data_dir),
    );
    // Applications: the status-bearing aggregate root (ADR 0001). Opening it
    // runs the one-time, idempotent backfill from ai_generations.db, so it
    // is managed AFTER the generation store above (order is cosmetic — the
    // backfill reads ai_generations.db by path, not via Tauri state).
    manage_opened(
        app,
        registry,
        "applications",
        "applications store",
        applications::ApplicationStore::open(data_dir),
    );
    manage_opened(
        app,
        registry,
        "job_preferences",
        "job preferences store",
        job_preferences::JobPreferencesStore::open(data_dir),
    );
    manage_opened(
        app,
        registry,
        "contact_profile",
        "contact profile store",
        contact_profile::ContactProfileStore::open(data_dir),
    );
    // Backend-owned active AI provider store (task #16): the single source of
    // truth for generation routing (provider/model/base_url). Holds no
    // secrets; wiped on factory reset, included in backups.
    manage_opened(
        app,
        registry,
        "ai_provider_config",
        "ai provider config store",
        ai_config::AiConfigStore::open(data_dir),
    );
    manage_opened(
        app,
        registry,
        "referrals",
        "referrals store",
        referrals::ReferralStore::open(data_dir),
    );
    manage_resettable(
        app,
        registry,
        "job_tracker",
        Mutex::new(JobTracker::open(data_dir)),
    );
    manage_resettable(
        app,
        registry,
        "postings",
        Mutex::new(PostingsCache::default()),
    );
    manage_resettable(
        app,
        registry,
        "interactions",
        Mutex::new(InteractionStore::new(data_dir)),
    );
    // AI-spend visibility: real per-call token usage + estimated cost,
    // recorded by the streaming and pipeline chokepoints in
    // `commands::ai_provider::stream` / `pipeline::Completer::complete`.
    manage_opened(
        app,
        registry,
        "spend",
        "spend store",
        spend::SpendStore::open(data_dir),
    );
    // Email-confirmation watching (task #23, auto-track Layer C). Holds
    // no secrets (the app password lives in the OS keychain via
    // CredentialStore); wiped on factory reset, NOT included in backups
    // (machine-local mailbox bookkeeping — see `email_watch::mod` doc).
    // The poller itself is started separately, below (`email_watch_scheduler::start`).
    manage_opened(
        app,
        registry,
        "email_watch",
        "email watch store",
        email_watch::EmailWatchStore::open(data_dir),
    );
}

/// The process-local state: nothing here is user data, so none of it is registered
/// for factory reset. Returns the scraper engine, which the late stores wire into.
pub(super) fn manage_process_state(app: &App) -> Arc<ScraperEngine> {
    app.manage(Mutex::new(UpdaterState::default()));
    // ONE cancel registry for every job kind (`jobs::cancel`). The
    // engine dispatches through it (`ScraperEngine::cancel`, which
    // `jobs_cancel` calls for every job id regardless of kind), and it
    // is also managed on its own so a non-scraping run — a pipeline
    // run — registers its token without borrowing the scraper.
    // Process-local, holds no user data.
    let scraper_engine = std::sync::Arc::new(ScraperEngine::new());
    app.manage(scraper_engine.cancel_registry());
    // Cloned into state rather than moved: the engine is also handed its
    // per-board reliability store further down (that store is opened with
    // the other L1 stores, after this point).
    app.manage(scraper_engine.clone());
    // In-memory anti-abuse limiter (rate + concurrency + per-provider daily
    // ceiling) for the expensive commands `ai_generate`, `scrape_board`, and
    // `scrape_url`. Process-local; resets on restart. Not in the reset
    // registry — it holds no user data, only transient counters.
    app.manage(std::sync::Arc::new(limits::Limiter::new()));
    // Live performance config (balanced default). Updated by system_set_performance_mode.
    crate::performance::set(crate::performance::PerformanceConfig::default());
    app.manage(crate::commands::translation::TranslationCache::new());
    // Live close-to-tray flag (default on). The renderer pushes the
    // persisted preference via `system_set_close_to_tray` on boot; the
    // window-close handler in `run()` reads it.
    app.manage(crate::CloseToTray::default());
    // The conversations (chat) feature was removed; best-effort delete the
    // now-orphaned conversations.db (+ WAL/SHM sidecars) it left in the app-data
    // dir so dead chat history isn't kept on disk. Idempotent (no-op once gone).
    if let Ok(app_data) = app.handle().path().app_data_dir() {
        for f in [
            "conversations.db",
            "conversations.db-wal",
            "conversations.db-shm",
        ] {
            let _ = std::fs::remove_file(app_data.join(f));
        }
    }
    scraper_engine
}

/// The stores registered after the process-local state. Each is registered last at
/// its point, so its label sits at the tail of `MANAGE_RESETTABLE_LABELS`, matching
/// the order the boot assertion in `setup` pins.
pub(super) fn manage_late_stores(
    app: &App,
    registry: &mut ResetRegistry,
    data_dir: &Path,
    scraper_engine: &ScraperEngine,
) {
    manage_opened(
        app,
        registry,
        "cache",
        "pipeline cache",
        crate::pipeline::cache::KvCache::open(data_dir),
    );
    // Cross-board dedup verdict store (ADR-029): the durable "not a
    // duplicate" pair tombstones. Registered LAST so its label sits at the
    // tail of `MANAGE_RESETTABLE_LABELS`, matching the order the boot
    // assertion in `setup` pins. Holds only opaque canonical-key pairs.
    manage_opened(
        app,
        registry,
        "dedup_tombstones",
        "dedup store",
        dedup::DedupStore::open(data_dir),
    );
    // Passively-harvested ATS company slugs (ADR-030): the slug typeahead +
    // watched-company autopilot targets. Holds only public ATS slugs +
    // display names — no secrets. Registered after dedup so its label sits
    // at the tail of `MANAGE_RESETTABLE_LABELS`.
    manage_opened(
        app,
        registry,
        "discovered_companies",
        "discovered store",
        discovered::DiscoveredCompanyStore::open(data_dir),
    );
    // Pipeline/agent run history + per-stage event trail. Its own DB
    // (`pipeline_runs.db`); retention is newest-3-per-job, pruned on the
    // performance-tier hook (`system_set_performance_mode`). Registered
    // last, so its label sits at the tail of `MANAGE_RESETTABLE_LABELS`.
    manage_opened(
        app,
        registry,
        "pipeline_runs",
        "pipeline run store",
        crate::pipeline::runs::PipelineRunStore::open(data_dir),
    );
    // Per-board reliability history (Track B1): ONE row per board, folded
    // from every run's `BoardScrapeSummary`, so a chip can tell "found
    // nothing today" apart from "broken since Tuesday". Bounded by the
    // scraper registry (~24 rows) — it has no growth axis and nothing to
    // prune. Machine-local diagnostics: wiped on factory reset,
    // deliberately NOT in the backup bundle (like `email_watch`).
    // Registered last, so its label sits at the tail of
    // `MANAGE_RESETTABLE_LABELS`.
    match crate::scraping::board_health::BoardHealthStore::open(data_dir) {
        Ok(store) => {
            let store = Arc::new(store);
            scraper_engine.set_health_store(store.clone());
            manage_resettable(app, registry, "board_health", store);
        }
        // `e` is an `AppError::Storage`, and `BoardHealthStore::open`'s
        // underlying `rusqlite`/`std::fs` error can embed the full
        // data-dir path (e.g. `rusqlite::Error::InvalidPath`) — log
        // the path-free category code instead of the raw error (the
        // repo path-privacy rule extends to logs).
        Err(e) => log::warn!(
            "[setup] board health store failed to open (non-fatal): {}",
            e.code()
        ),
    }
}
