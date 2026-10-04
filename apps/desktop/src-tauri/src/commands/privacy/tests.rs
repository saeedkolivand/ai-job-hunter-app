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

// HIGH fix: guards the WIRING specifically, not just `clear()`'s own
// body (that's `email_watch::tests::clear_resets_the_auto_write_opt_in`)
// — a regression back to a no-op `reset` (or a call to some OTHER
// wipe that forgets `auto_write_enabled`) here would pass every other
// test in this file (labels, other stores' resets) while silently
// letting a factory reset carry a stranger account's auto-write
// opt-in forward to whichever mailbox connects next.
#[test]
fn email_watch_store_reset_clears_the_auto_write_opt_in() {
    let dir = TempDir::new().unwrap();
    let store = EmailWatchStore::open(&dir.path().to_path_buf()).unwrap();
    store.connect("a@gmail.com", "imap.gmail.com", 993).unwrap();
    assert!(store.set_auto_write_enabled(true).unwrap());
    assert!(store.status().auto_write_enabled, "precondition: opted in");

    Resettable::reset(&store);

    assert!(
        !store.status().auto_write_enabled,
        "a factory reset (Resettable::reset) must clear the auto-write \
             opt-in, not just the account/seen rows — a different mailbox \
             may connect next and never made this choice"
    );
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
            email_subject: String::new(),
            email_body: String::new(),
            application_id: None,
            quality_report: String::new(),
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
            salary_expectation: Some("€75,000".into()),
            extra_agency_companies: Some(vec!["Hays".into()]),
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
    assert!(
        after.salary_expectation.is_none(),
        "salary_expectation must also be None after reset"
    );
    assert!(
        after.extra_agency_companies.is_none(),
        "extra_agency_companies must also be None after reset"
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
    reg.register::<DedupStore>("dedup_tombstones");
    reg.register::<DiscoveredCompanyStore>("discovered_companies");
    reg.register::<PipelineRunStore>("pipeline_runs");
    reg.register::<std::sync::Arc<crate::scraping::BoardHealthStore>>("board_health");

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
