use std::sync::Arc;

use parking_lot::Mutex;

use serde_json::{json, Value};
use tauri::{App, AppHandle, Manager};

use crate::ai_config::AiConfigStore;
use crate::ai_generations::AiGenerationStore;
use crate::applications::ApplicationStore;
use crate::autopilot::AutopilotStore;
use crate::contact_profile::ContactProfileStore;
use crate::credentials::CredentialStore;
use crate::data_store::Resettable;
use crate::documents::DocumentStore;
use crate::email_watch::EmailWatchStore;
use crate::job_preferences::JobPreferencesStore;
use crate::jobs::JobTracker;
use crate::notifications::NotificationStore;
use crate::pipeline::cache::KvCache;
use crate::postings::{InteractionStore, PostingsCache};
use crate::referrals::ReferralStore;
use crate::spend::SpendStore;

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
impl Resettable for ApplicationStore {
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
impl Resettable for AiConfigStore {
    fn reset(&self) {
        self.clear();
    }
}
impl Resettable for ReferralStore {
    fn reset(&self) {
        self.clear_all();
    }
}
impl Resettable for NotificationStore {
    fn reset(&self) {
        self.clear_all();
    }
}
impl Resettable for KvCache {
    fn reset(&self) {
        self.clear();
    }
}
impl Resettable for SpendStore {
    fn reset(&self) {
        self.clear_all();
    }
}
impl Resettable for EmailWatchStore {
    fn reset(&self) {
        // Account row back to defaults + every `seen` row gone. The keychain
        // app password is cleared separately by `Mutex<CredentialStore>`'s own
        // `reset` (registered under the "credentials" label), which wipes
        // every stored secret including this store's `email-imap` slot.
        let _ = self.clear();
    }
}

/// A type-erased factory-reset action: resolve a store from app state and wipe it.
type ResetAction = Box<dyn Fn(&AppHandle) + Send + Sync>;

/// The factory-reset labels every persistent user-data store is registered under
/// via `manage_resettable` in `lib.rs::setup`. SINGLE source of truth: `setup`
/// debug-asserts the live registry matches this (catching a forgotten
/// `manage_resettable`) and the completeness test pins it. Bridge/notification
/// stores register through their own `manage` helpers and are NOT in this list.
pub const MANAGE_RESETTABLE_LABELS: &[&str] = &[
    "autopilots",
    "credentials",
    "documents",
    "ai_generations",
    "applications",
    "job_preferences",
    "contact_profile",
    "ai_provider_config",
    "referrals",
    "job_tracker",
    "postings",
    "interactions",
    "spend",
    "email_watch",
    "cache",
];

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
    // Bail rather than fall back to the CWD (".") here: `data_dir` backs the
    // destructive `remove_dir_all(data_dir.join("browser-state"))` below, so a
    // CWD-relative fallback would wipe the wrong target. Refuse the reset if the
    // OS can't resolve the app data dir.
    let data_dir = match app.path().app_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            log::error!("[privacy] factory reset aborted: could not resolve app data dir: {e}");
            return json!({ "success": false, "error": "could not resolve app data directory" });
        }
    };

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

    // Delete the persisted Chromium board-login profiles wholesale: the
    // `browser-state/<board_id>/{profile,cookies.json,auth-status.json}` tree
    // (see `scraping::board_login`). The `disconnect` loop above only flips the
    // status flag (it deliberately leaves the profile in place while Chromium may
    // hold file locks mid-session), so a factory reset would otherwise leave
    // authenticated board sessions on disk — a privacy gap (ADR 0005). Best-effort:
    // remove_dir_all can fail if a browser still holds a lock (common on Windows);
    // log and continue rather than failing the reset. Skip when absent so a fresh
    // install (no logins yet) doesn't log a spurious warning.
    let browser_state = data_dir.join("browser-state");
    let mut browser_state_cleared = true;
    if browser_state.exists() {
        if let Err(e) = std::fs::remove_dir_all(&browser_state) {
            log::warn!(
                "[privacy] factory reset could not remove browser-state (may be locked): {e}"
            );
            browser_state_cleared = false;
        }
    }

    if browser_state_cleared {
        json!({ "success": true })
    } else {
        // Partial reset: the persistent stores above WERE wiped, but the on-disk
        // Chromium board-login profiles could not be removed (commonly a Windows
        // file lock while a browser still holds the profile). Report this honestly
        // — returning `success: true` here would tell the user their sessions were
        // cleared while authenticated board logins remain on disk (a privacy gap).
        // The renderer surfaces this as a warning so the reset isn't silently
        // over-reported as complete.
        json!({
            "success": false,
            "error": "board login sessions could not be fully cleared",
            "browserStateRetained": true,
        })
    }
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

    // C3 — `Resettable` for AiGenerationStore: populate then reset, verify empty.
    #[test]
    fn ai_generation_store_reset_empties_all_records() {
        let dir = TempDir::new().unwrap();
        let store = AiGenerationStore::open(&dir.path().to_path_buf()).unwrap();
        store
            .insert(&crate::ai_generations::AiGenerationRecord {
                id: "g1".into(),
                created_at: 1000,
                candidate_name: "Jane".into(),
                job_title: "Engineer".into(),
                company_name: "Acme".into(),
                resume_language: "en".into(),
                job_ad_language: "en".into(),
                target_language: "en".into(),
                mismatch: false,
                top_requirements: vec![],
                mode: "ats".into(),
                resume_text: "R".into(),
                cover_letter_text: "C".into(),
                job_ad: "JD".into(),
                job_url: String::new(),
                board: String::new(),
                application_answers: vec![],
                company_brief: String::new(),
                interview_questions: vec![],
                application_id: None,
            })
            .unwrap();
        assert_eq!(store.list().len(), 1, "precondition: one record inserted");

        Resettable::reset(&store);
        assert!(
            store.list().is_empty(),
            "AiGenerationStore must be empty after Resettable::reset"
        );
    }

    // C3 — `Resettable` for ApplicationStore: populate both tables then reset.
    #[test]
    fn application_store_reset_empties_applications_and_events() {
        let dir = TempDir::new().unwrap();
        let store = crate::applications::ApplicationStore::open(dir.path()).unwrap();
        store
            .track_manual(
                "",
                "",
                &crate::applications::ApplicationMeta {
                    company: "Acme".into(),
                    title: "Dev".into(),
                    candidate: "Jane".into(),
                    brief: String::new(),
                    job_description: String::new(),
                    answers: vec![],
                    job_summary: String::new(),
                    salary_min: None,
                    salary_max: None,
                    salary_currency: None,
                },
            )
            .unwrap();
        let id = store.list().first().unwrap().id.clone();
        assert!(!store.events(&id).is_empty(), "precondition: event exists");

        Resettable::reset(&store);
        assert!(
            store.list().is_empty(),
            "ApplicationStore.list must be empty after reset"
        );
        assert!(
            store.events(&id).is_empty(),
            "status_events must also be wiped by reset"
        );
    }

    // C3 — `Resettable` for JobPreferencesStore: the impl calls `.clear()`, NOT
    // `.clear_all()`. Pin that this wrapping correctly zeroes the preferences.
    #[test]
    fn job_preferences_store_reset_nullifies_all_fields() {
        let dir = TempDir::new().unwrap();
        let store =
            crate::job_preferences::JobPreferencesStore::open(&dir.path().to_path_buf()).unwrap();
        store
            .set(&crate::job_preferences::JobPreferences {
                location: Some("Berlin".into()),
                country_code: Some("de".into()),
                tech_stack: Some(vec![crate::job_preferences::TechStackItem {
                    name: "Rust".into(),
                    category: "backend".into(),
                }]),
            })
            .unwrap();
        let before = store.get();
        assert!(
            before.location.is_some(),
            "precondition: location set before reset"
        );

        Resettable::reset(&store);
        let after = store.get();
        assert!(
            after.location.is_none(),
            "location must be None after reset"
        );
        assert!(
            after.tech_stack.is_none(),
            "tech_stack must be None after reset"
        );
        assert!(
            after.country_code.is_none(),
            "country_code must also be None after reset"
        );
    }

    // C3 — `Resettable` for ContactProfileStore: the impl calls `.clear()` which
    // writes a default profile. Pin that the reset yields an empty profile.
    #[test]
    fn contact_profile_store_reset_yields_default_empty_profile() {
        let dir = TempDir::new().unwrap();
        let store =
            crate::contact_profile::ContactProfileStore::open(&dir.path().to_path_buf()).unwrap();
        let profile = crate::contact_profile::ContactProfile {
            email: Some("jane@acme.com".into()),
            ..Default::default()
        };
        store.set(&profile).unwrap();
        assert!(
            store.get().email.is_some(),
            "precondition: email set before reset"
        );

        Resettable::reset(&store);
        let after = store.get();
        assert!(
            after.email.is_none(),
            "email must be None after ContactProfileStore reset"
        );
    }

    // C3 — Registry completeness: every label that `privacy_reset_app` must wipe
    // is registered. The expected set now comes from the shared
    // `MANAGE_RESETTABLE_LABELS` const (the single source of truth), and
    // `lib.rs::setup` debug-asserts the live registry equals it — so a new
    // persistent store added via `manage_resettable` can't silently escape the
    // factory-reset wipe without tripping both this test and the boot assertion.
    //
    // NOTE: This exercises the type-erased registry labels and the compile-time
    // `T: Resettable` bound, not the AppHandle dispatch (which needs a live
    // Tauri runtime). The dispatch is correct-by-construction: `register::<T>`
    // only compiles when `T` implements `Resettable`.
    #[test]
    fn reset_registry_expected_labels_match_lib_rs_setup() {
        // Labels come from the shared const that `lib.rs::setup` debug-asserts
        // against. Bridge/notification labels register via their own `manage`
        // helpers and are intentionally excluded.
        let expected: &[&str] = MANAGE_RESETTABLE_LABELS;

        let mut reg = ResetRegistry::default();
        // Replicate registrations (types must match the T used in lib.rs).
        reg.register::<Arc<Mutex<AutopilotStore>>>("autopilots");
        reg.register::<Mutex<CredentialStore>>("credentials");
        reg.register::<DocumentStore>("documents");
        reg.register::<AiGenerationStore>("ai_generations");
        reg.register::<ApplicationStore>("applications");
        reg.register::<JobPreferencesStore>("job_preferences");
        reg.register::<ContactProfileStore>("contact_profile");
        reg.register::<AiConfigStore>("ai_provider_config");
        reg.register::<ReferralStore>("referrals");
        reg.register::<Mutex<JobTracker>>("job_tracker");
        reg.register::<Mutex<PostingsCache>>("postings");
        reg.register::<Mutex<InteractionStore>>("interactions");
        reg.register::<SpendStore>("spend");
        reg.register::<EmailWatchStore>("email_watch");
        reg.register::<KvCache>("cache");

        let labels = reg.labels();
        for label in expected {
            assert!(
                labels.contains(label),
                "expected reset label '{label}' missing from registry; current labels: {labels:?}"
            );
        }
        // Count must match so a removal in lib.rs also breaks this guard.
        assert_eq!(
            labels.len(),
            expected.len(),
            "registry label count changed: got {labels:?}, expected {expected:?}"
        );
    }
}
