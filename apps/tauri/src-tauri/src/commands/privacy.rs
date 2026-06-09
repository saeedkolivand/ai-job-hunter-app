use std::sync::Arc;

use parking_lot::Mutex;

use serde_json::{json, Value};
use tauri::{App, AppHandle, Manager};

use crate::ai_generations::AiGenerationStore;
use crate::autopilot::AutopilotStore;
use crate::contact_profile::ContactProfileStore;
use crate::credentials::CredentialStore;
use crate::data_store::Resettable;
use crate::documents::DocumentStore;
use crate::job_preferences::JobPreferencesStore;
use crate::jobs::JobTracker;
use crate::pipeline::cache::KvCache;
use crate::postings::{InteractionStore, PostingsCache};
use crate::referrals::ReferralStore;

// ── Full-reset registry ───────────────────────────────────────────────────────
//
// Each managed persistent store implements `Resettable` (clearing its own
// contents), and is registered into the `ResetRegistry` at its `.manage()` site
// via `manage_resettable`. `privacy_reset_app` then iterates the registry — so a
// new store is wiped on factory reset just by being managed through the helper,
// with no edit to the reset command and no hand-maintained clear list.

impl Resettable for Mutex<PostingsCache> {
    fn reset(&self) {
        self.lock().clear_all();
    }
}
impl Resettable for Mutex<InteractionStore> {
    fn reset(&self) {
        self.lock().clear_all();
    }
}
impl Resettable for Mutex<JobTracker> {
    fn reset(&self) {
        self.lock().clear();
    }
}
impl Resettable for Mutex<CredentialStore> {
    fn reset(&self) {
        // Clears every stored secret (AI/provider keys incl. the Ollama account
        // key, board passwords), driven off the credential metadata index.
        let _ = self.lock().clear_all();
    }
}
impl Resettable for Arc<Mutex<AutopilotStore>> {
    fn reset(&self) {
        self.lock().clear_all();
    }
}
impl Resettable for DocumentStore {
    fn reset(&self) {
        self.clear_all();
    }
}
impl Resettable for AiGenerationStore {
    fn reset(&self) {
        self.clear_all();
    }
}
impl Resettable for JobPreferencesStore {
    fn reset(&self) {
        let _ = self.clear();
    }
}
impl Resettable for ContactProfileStore {
    fn reset(&self) {
        let _ = self.clear();
    }
}
impl Resettable for ReferralStore {
    fn reset(&self) {
        self.clear_all();
    }
}
impl Resettable for KvCache {
    fn reset(&self) {
        self.clear();
    }
}

/// A type-erased factory-reset action: resolve a store from app state and wipe it.
type ResetAction = Box<dyn Fn(&AppHandle) + Send + Sync>;

/// Registry of factory-reset actions, populated as stores are managed and
/// iterated by [`privacy_reset_app`]. Holds type-erased closures that resolve
/// each store from Tauri state and call its [`Resettable::reset`].
#[derive(Default)]
pub struct ResetRegistry {
    actions: Vec<(&'static str, ResetAction)>,
}

impl ResetRegistry {
    /// Register a managed store type `T` (a `Resettable` wrapper, e.g.
    /// `Mutex<PostingsCache>`). At reset time the store is resolved from state and
    /// wiped; if it was never managed (a store that failed to open), it's skipped.
    pub fn register<T: Resettable + Send + Sync + 'static>(&mut self, label: &'static str) {
        self.actions.push((
            label,
            Box::new(|app: &AppHandle| {
                if let Some(state) = app.try_state::<T>() {
                    state.reset();
                }
            }),
        ));
    }

    /// Wipe every registered store. Returns the labels cleared (for logging/tests).
    pub fn reset_all(&self, app: &AppHandle) -> Vec<&'static str> {
        for (_, action) in &self.actions {
            action(app);
        }
        self.labels()
    }

    /// Labels of all registered stores, in registration order.
    pub fn labels(&self) -> Vec<&'static str> {
        self.actions.iter().map(|(label, _)| *label).collect()
    }
}

/// Manage a persistent store **and** register its factory-reset wipe in one call,
/// so the `.manage()` site is the single place a store is wired for full-reset
/// coverage. Use this (not bare `app.manage`) for every persistent user-data
/// store; ephemeral/non-user state (updater, scraper engine) stays plain `manage`.
pub fn manage_resettable<T: Resettable + Send + Sync + 'static>(
    app: &App,
    registry: &mut ResetRegistry,
    label: &'static str,
    store: T,
) {
    app.manage(store);
    registry.register::<T>(label);
}

#[tauri::command]
pub fn privacy_clear_data(app: AppHandle) -> Value {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    for board_id in &["linkedin", "indeed", "xing", "glassdoor"] {
        crate::scraping::board_login::disconnect(&data_dir, board_id);
    }
    app.state::<Mutex<PostingsCache>>().lock().clear_all();
    app.state::<Mutex<InteractionStore>>().lock().clear_all();
    json!({ "success": true })
}

#[tauri::command]
pub fn privacy_clear_interactions(app: AppHandle) -> Value {
    app.state::<Mutex<InteractionStore>>().lock().clear_all();
    json!({ "success": true })
}

/// Sign out of all connected job boards (sessions only, data is preserved).
#[tauri::command]
pub fn privacy_sign_out_all(app: AppHandle) -> Value {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    for board_id in &["linkedin", "indeed", "xing", "glassdoor"] {
        crate::scraping::board_login::disconnect(&data_dir, board_id);
    }
    json!({ "success": true })
}

/// Full factory reset: sign out all boards and wipe every persistent store.
/// The frontend is responsible for resetting persisted preferences (localStorage).
#[tauri::command]
pub fn privacy_reset_app(app: AppHandle) -> Value {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    // Sign out all board sessions
    for board_id in &["linkedin", "indeed", "xing", "glassdoor"] {
        crate::scraping::board_login::disconnect(&data_dir, board_id);
    }

    // Wipe every persistent store registered via `manage_resettable` in
    // `main.rs::setup` — résumé/doc/generation stores, secrets, caches, and the
    // job log. Registry-driven, so adding a store needs registration only; this
    // command never changes. (Stores that failed to open are skipped.)
    let cleared = app.state::<ResetRegistry>().reset_all(&app);
    log::info!(
        "[privacy] factory reset wiped {} stores: {cleared:?}",
        cleared.len()
    );

    json!({ "success": true })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // The `Resettable` impl for the job log uses `.clear()`, not `.clear_all()` —
    // guard against that mapping silently regressing.
    #[test]
    fn job_tracker_reset_clears_the_job_log() {
        let dir = TempDir::new().unwrap();
        let tracker = Mutex::new(JobTracker::open(dir.path()));
        tracker.lock().start("job-1", "ai.generate");
        assert!(!tracker.lock().list().is_empty());

        Resettable::reset(&tracker);
        assert!(tracker.lock().list().is_empty(), "job log wiped on reset");
    }

    // A bare (non-Mutex) store impl, exercising the cache path.
    #[test]
    fn kv_cache_reset_clears_entries() {
        let dir = TempDir::new().unwrap();
        let cache = KvCache::open(dir.path()).unwrap();
        cache.set("ns", "k", "v");
        assert!(cache.get("ns", "k", 3600).is_some());

        Resettable::reset(&cache);
        assert!(cache.get("ns", "k", 3600).is_none(), "cache wiped on reset");
    }

    // Registration is type-checked (`T: Resettable`) and ordered — a store that
    // doesn't implement `Resettable` can't be registered, and the labels reflect
    // exactly what was registered.
    #[test]
    fn registry_records_registrations_in_order() {
        let mut reg = ResetRegistry::default();
        reg.register::<Mutex<JobTracker>>("job_tracker");
        reg.register::<AiGenerationStore>("ai_generations");
        reg.register::<KvCache>("cache");
        assert_eq!(reg.labels(), vec!["job_tracker", "ai_generations", "cache"]);
    }
}
