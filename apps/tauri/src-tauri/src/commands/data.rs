//! Full backup / restore across every persistent user-data store.
//!
//! Builds one versioned JSON bundle from each store's `DataStore::export`, and
//! restores via `DataStore::import` (REPLACE semantics). Secrets and ephemeral
//! caches are intentionally excluded — see `data_store.rs`.

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ai_generations::AiGenerationStore;
use crate::applications::ApplicationStore;
use crate::autopilot::AutopilotStore;
use crate::contact_profile::ContactProfileStore;
use crate::data_store::DataStore;
use crate::documents::DocumentStore;
use crate::job_preferences::JobPreferencesStore;
use crate::postings::{InteractionRecord, InteractionStore};
use crate::referrals::ReferralStore;

const BUNDLE_VERSION: u32 = 1;

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn date_stamp() -> String {
    // YYYY-MM-DD from the system clock (UTC-ish, good enough for a filename).
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = secs / 86_400;
    // Civil date from days since epoch (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Structurally validate every section that is present in `stores`, returning
/// `Err(message)` for the first section whose top-level shape does not match what
/// its store's `import` requires. Runs BEFORE any store is mutated so a malformed
/// bundle aborts the restore without clearing a store. Only checks presence +
/// top-level shape (array vs object); deep per-record validation happens
/// atomically inside each store's transactional `import`.
fn validate_sections(stores: &Value) -> crate::error::AppResult<()> {
    // Sections whose export is a JSON array of records.
    const ARRAY_SECTIONS: &[&str] = &[
        "documents",
        "aiGenerations",
        "applications",
        "referrals",
        "autopilots",
        "interactions",
    ];
    // Sections whose export is a single JSON object (single-row settings stores).
    const OBJECT_SECTIONS: &[&str] = &["jobPreferences", "contactProfile"];

    for key in ARRAY_SECTIONS {
        if let Some(section) = stores.get(*key) {
            if !section.is_array() {
                return Err(crate::error::AppError::Validation(format!(
                    "section '{key}' must be a JSON array"
                )));
            }
        }
    }
    for key in OBJECT_SECTIONS {
        if let Some(section) = stores.get(*key) {
            if !section.is_object() {
                return Err(crate::error::AppError::Validation(format!(
                    "section '{key}' must be a JSON object"
                )));
            }
        }
    }
    Ok(())
}

/// Collect a bundle of every store's exported data.
fn build_bundle(app: &AppHandle) -> Value {
    let mut stores = serde_json::Map::new();

    if let Some(s) = app.try_state::<DocumentStore>() {
        stores.insert(s.key().to_string(), s.export());
    }
    if let Some(s) = app.try_state::<AiGenerationStore>() {
        stores.insert(s.key().to_string(), s.export());
    }
    if let Some(s) = app.try_state::<ApplicationStore>() {
        stores.insert(s.key().to_string(), s.export());
    }
    if let Some(s) = app.try_state::<JobPreferencesStore>() {
        stores.insert(s.key().to_string(), s.export());
    }
    if let Some(s) = app.try_state::<ContactProfileStore>() {
        stores.insert(s.key().to_string(), s.export());
    }
    if let Some(s) = app.try_state::<ReferralStore>() {
        stores.insert(s.key().to_string(), s.export());
    }
    if let Some(s) = app.try_state::<std::sync::Arc<Mutex<AutopilotStore>>>() {
        let guard = s.lock();
        stores.insert(guard.key().to_string(), guard.export());
    }
    if let Some(s) = app.try_state::<Mutex<InteractionStore>>() {
        let interactions = s.lock().export_all();
        stores.insert("interactions".to_string(), json!(interactions));
    }

    json!({
        "version": BUNDLE_VERSION,
        "exportedAt": now_ms(),
        "stores": Value::Object(stores),
    })
}

/// Export all user data to a user-chosen JSON file.
#[tauri::command]
pub async fn data_export(app: AppHandle) -> Value {
    use tauri_plugin_dialog::{DialogExt, FilePath};

    let bundle = build_bundle(&app);

    let default_name = format!("ajh-backup-{}.json", date_stamp());
    let path = app
        .dialog()
        .file()
        .set_title("Export App Data")
        .set_file_name(&default_name)
        .add_filter("JSON", &["json"])
        .blocking_save_file();

    let Some(file_path) = path else {
        return json!({ "success": false });
    };
    let path_str = match file_path {
        FilePath::Path(p) => p.to_string_lossy().into_owned(),
        FilePath::Url(u) => u.to_string(),
    };

    match std::fs::write(
        &path_str,
        serde_json::to_string_pretty(&bundle).unwrap_or_default(),
    ) {
        Ok(()) => json!({ "success": true, "filePath": path_str }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

/// Restore all user data from a user-chosen backup file (REPLACE semantics).
#[tauri::command]
pub async fn data_import(app: AppHandle) -> Value {
    use tauri_plugin_dialog::{DialogExt, FilePath};

    let path = app
        .dialog()
        .file()
        .set_title("Import App Data")
        .add_filter("JSON", &["json"])
        .blocking_pick_file();

    let Some(file_path) = path else {
        return json!({ "success": false });
    };
    let path_str = match file_path {
        FilePath::Path(p) => p.to_string_lossy().into_owned(),
        FilePath::Url(u) => u.to_string(),
    };

    let raw = match std::fs::read_to_string(&path_str) {
        Ok(s) => s,
        Err(e) => return json!({ "success": false, "error": e.to_string() }),
    };
    let bundle: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return json!({ "success": false, "error": "Invalid JSON" }),
    };
    if bundle.get("version").and_then(|v| v.as_u64()) != Some(BUNDLE_VERSION as u64) {
        return json!({ "success": false, "error": "Unsupported backup version" });
    }
    let stores = match bundle.get("stores") {
        Some(s) => s,
        None => return json!({ "success": false, "error": "Backup has no 'stores'" }),
    };

    // Validate the SHAPE of every present section BEFORE mutating any store, so a
    // malformed section aborts the whole restore without having cleared a store.
    // Each store's `import` clears-then-repopulates inside one transaction, and the
    // two multi-row stores (aiGenerations, applications) also deserialize every
    // record before touching their table — so a single store's restore is atomic.
    //
    // Residual limit: the stores live in SEPARATE SQLite files, which cannot share
    // one transaction. This pre-pass guarantees we never *begin* mutating if any
    // section is structurally invalid, but if a later store's write fails at the
    // SQLite level (e.g. disk error) after an earlier store already committed, the
    // earlier store is not rolled back. Full cross-file atomicity would require a
    // single shared database; that is out of scope here.
    if let Err(err) = validate_sections(stores) {
        return json!({ "success": false, "error": err.to_string() });
    }

    let mut imported = serde_json::Map::new();
    let mut had_error = false;
    let mut import_into = |key: &str, store: &dyn DataStore| {
        if let Some(data) = stores.get(key) {
            match store.import(data) {
                Ok(n) => {
                    imported.insert(key.to_string(), json!(n));
                }
                Err(e) => {
                    had_error = true;
                    imported.insert(key.to_string(), json!({ "error": e }));
                }
            }
        }
    };

    if let Some(s) = app.try_state::<DocumentStore>() {
        import_into("documents", s.inner());
    }
    if let Some(s) = app.try_state::<AiGenerationStore>() {
        import_into("aiGenerations", s.inner());
    }
    if let Some(s) = app.try_state::<ApplicationStore>() {
        import_into("applications", s.inner());
    }
    if let Some(s) = app.try_state::<JobPreferencesStore>() {
        import_into("jobPreferences", s.inner());
    }
    if let Some(s) = app.try_state::<ContactProfileStore>() {
        import_into("contactProfile", s.inner());
    }
    if let Some(s) = app.try_state::<ReferralStore>() {
        import_into("referrals", s.inner());
    }
    if let Some(s) = app.try_state::<std::sync::Arc<Mutex<AutopilotStore>>>() {
        let guard = s.lock();
        import_into("autopilots", &*guard);
    }
    if let Some(s) = app.try_state::<Mutex<InteractionStore>>() {
        if let Some(data) = stores.get("interactions") {
            match serde_json::from_value::<Vec<InteractionRecord>>(data.clone()) {
                Ok(records) => {
                    let n = s.lock().import_bundle(records);
                    imported.insert("interactions".to_string(), json!(n));
                }
                Err(e) => {
                    had_error = true;
                    imported.insert(
                        "interactions".to_string(),
                        json!({ "error": e.to_string() }),
                    );
                }
            }
        }
    }

    json!({
        "success": !had_error,
        "partial": had_error,
        "imported": Value::Object(imported),
    })
}
