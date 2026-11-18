//! Optional self-hosted sync (ADR-051): the background runs (shortly after
//! launch, then every 15 minutes) and [`run_now`], which the `sync_now`
//! command and the Settings card share.
//!
//! Mirrors the `email_watch`/`email_watch_scheduler` split: `sync` (the
//! engine, the HTTP transport, the settings) stays Tauri-free, and this module
//! is the one that resolves the stores from app state and spawns.
//!
//! Off by default: nothing runs and nothing is sent until the user has saved
//! a server URL and a token, and turned sync on.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tauri::{AppHandle, Manager, State};

use crate::ai_config::AiConfigStore;
use crate::ai_generations::AiGenerationStore;
use crate::applications::{ApplicationStatusEvents, ApplicationStore};
use crate::autopilot::AutopilotStore;
use crate::contact_profile::ContactProfileStore;
use crate::data_store::SyncSection;
use crate::db::now_ms;
use crate::dedup::DedupStore;
use crate::discovered::DiscoveredCompanyStore;
use crate::documents::DocumentStore;
use crate::error::{AppError, AppResult};
use crate::job_preferences::JobPreferencesStore;
use crate::pipeline::runs::PipelineRunStore;
use crate::posting_history::PostingHistoryStore;
use crate::postings::{InteractionStore, InteractionsSection};
use crate::referrals::ReferralStore;
use crate::spend::SpendStore;
use crate::sync::engine::{self, SyncReport};
use crate::sync::envelope::scope_key;
use crate::sync::http::{HttpTransport, TOKEN_REJECTED};
use crate::sync::settings::{self, SyncStatus};
use crate::sync::state::SyncStateStore;

/// The keychain slot of the sync token (`credentials::CredentialStore`), so
/// a factory reset's credential wipe removes it with every other secret.
pub const TOKEN_SLOT: &str = "sync-token";

/// Let the app finish starting before the first run.
const STARTUP_GRACE: Duration = Duration::from_secs(30);
/// ADR-051: on launch, every 15 minutes, and "Sync now".
const INTERVAL: Duration = Duration::from_secs(15 * 60);

/// One run at a time, whoever starts it.
static RUNNING: AtomicBool = AtomicBool::new(false);

struct RunningGuard;

impl RunningGuard {
    fn acquire() -> AppResult<Self> {
        if RUNNING.swap(true, Ordering::AcqRel) {
            return Err(AppError::RateLimited("a sync is already running".into()));
        }
        Ok(Self)
    }
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::Release);
    }
}

/// Spawn the background loop. Same shape as `email_watch_scheduler::start`.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_GRACE).await;
        let mut interval = tokio::time::interval(INTERVAL);
        loop {
            interval.tick().await;
            if is_enabled(&app) {
                // The outcome is recorded in the status; nothing to do here.
                let _ = run_now(&app).await;
            }
        }
    });
}

fn is_enabled(app: &AppHandle) -> bool {
    app.try_state::<SyncStateStore>()
        .and_then(|s| settings::status(&s).ok())
        .is_some_and(|s| s.enabled)
}

fn state_store(app: &AppHandle) -> AppResult<State<'_, SyncStateStore>> {
    app.try_state::<SyncStateStore>()
        .ok_or_else(|| AppError::Storage("sync is unavailable".into()))
}

/// Run one sync now and record its outcome. Errors when sync is off, has no
/// token, or another run is in progress; a failed run is also recorded in the
/// returned status. A rejected token turns sync off (protocol "Failure
/// handling").
pub async fn run_now(app: &AppHandle) -> AppResult<SyncStatus> {
    let state = state_store(app)?;
    let status = settings::status(&state)?;
    let url = match (status.enabled, status.server_url) {
        (true, Some(url)) => url,
        _ => return Err(AppError::Validation("sync is off".into())),
    };
    let Some(token) = crate::credentials::read_credential(TOKEN_SLOT)? else {
        // The keychain lost the token (cleared outside the app, or a reset
        // that wiped credentials but not sync_state). Say so on the card and
        // stop retrying every 15 minutes.
        let message = "no sync token is saved; turn sync on again";
        settings::record_failure(&state, message)?;
        settings::disable(&state)?;
        return Err(AppError::Config(message.into()));
    };
    let _guard = RunningGuard::acquire()?;

    let runtime = tokio::runtime::Handle::current();
    let worker = app.clone();
    let outcome = tokio::task::spawn_blocking(move || run_blocking(&worker, &url, &token, runtime))
        .await
        .unwrap_or_else(|_| Err(AppError::message("sync run stopped unexpectedly")));

    match &outcome {
        Ok(report) => settings::record_success(&state, report, now_ms())?,
        Err(e) => {
            log::warn!("[sync] run failed: {}", e.code());
            settings::record_failure(&state, &e.to_string())?;
            if matches!(e, AppError::Config(m) if m == TOKEN_REJECTED) {
                settings::disable(&state)?;
            }
        }
    }
    let status = settings::status(&state)?;
    outcome.map(|_| status)
}

fn run_blocking(
    app: &AppHandle,
    url: &str,
    token: &str,
    runtime: tokio::runtime::Handle,
) -> AppResult<SyncReport> {
    let state = state_store(app)?;
    let transport = HttpTransport::new(url, token, runtime);
    let scope = scope_key(url, token);
    with_sections(app, |sections| {
        engine::run(sections, &state, &transport, &scope, now_ms())
    })
}

/// A managed store, or an error naming it. A run must see every section: a
/// section missing from a run would look like all its records were deleted.
fn need<'a, T: Send + Sync + 'static>(app: &'a AppHandle, name: &str) -> AppResult<State<'a, T>> {
    app.try_state::<T>()
        .ok_or_else(|| AppError::Storage(format!("sync skipped: the {name} store is unavailable")))
}

/// Run `f` over every export-bundle section, in bundle order.
fn with_sections<R>(
    app: &AppHandle,
    f: impl FnOnce(&[&dyn SyncSection]) -> AppResult<R>,
) -> AppResult<R> {
    let documents = need::<DocumentStore>(app, "documents")?;
    let generations = need::<AiGenerationStore>(app, "AI generations")?;
    let applications = need::<ApplicationStore>(app, "applications")?;
    let prefs = need::<JobPreferencesStore>(app, "job preferences")?;
    let contact = need::<ContactProfileStore>(app, "contact profile")?;
    let ai_config = need::<AiConfigStore>(app, "AI provider config")?;
    let referrals = need::<ReferralStore>(app, "referrals")?;
    let autopilots = need::<Arc<Mutex<AutopilotStore>>>(app, "autopilots")?;
    let interactions = need::<Mutex<InteractionStore>>(app, "interactions")?;
    let spend = need::<SpendStore>(app, "spend")?;
    let dedup = need::<DedupStore>(app, "dedup")?;
    let discovered = need::<DiscoveredCompanyStore>(app, "discovered companies")?;
    let runs = need::<PipelineRunStore>(app, "pipeline runs")?;
    let history = need::<PostingHistoryStore>(app, "posting history")?;

    let events = ApplicationStatusEvents(applications.inner());
    let interactions = InteractionsSection(interactions.inner());
    let sections: [&dyn SyncSection; 15] = [
        documents.inner(),
        generations.inner(),
        applications.inner(),
        &events,
        prefs.inner(),
        contact.inner(),
        ai_config.inner(),
        referrals.inner(),
        // Locked per call, never across a run's network I/O.
        autopilots.inner().as_ref(),
        &interactions,
        spend.inner(),
        dedup.inner(),
        discovered.inner(),
        runs.inner(),
        history.inner(),
    ];
    f(&sections)
}
