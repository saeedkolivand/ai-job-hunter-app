//! Full backup / restore across every persistent user-data store.
//!
//! Builds one versioned JSON bundle from each store's `DataStore::export`, and
//! restores via `DataStore::import` (REPLACE semantics). Secrets and ephemeral
//! caches are intentionally excluded — see `data_store.rs`.

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::ai_config::AiConfigStore;
use crate::ai_generations::AiGenerationStore;
use crate::applications::ApplicationStore;
use crate::autopilot::AutopilotStore;
use crate::contact_profile::ContactProfileStore;
use crate::data_store::DataStore;
use crate::db::now_ms;
use crate::dedup::DedupStore;
use crate::discovered::DiscoveredCompanyStore;
use crate::documents::DocumentStore;
use crate::job_preferences::JobPreferencesStore;
use crate::pipeline::runs::PipelineRunStore;
use crate::postings::{InteractionRecord, InteractionStore};
use crate::referrals::ReferralStore;
use crate::spend::SpendStore;

const BUNDLE_VERSION: u32 = 1;

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

/// Sections whose export is a JSON array of records.
///
/// Module-level (not fn-local) with [`OBJECT_SECTIONS`] because together they
/// are the bundle's section-key list, which the tests pin against the stores'
/// own `DataStore::key()` and against the import routing in [`data_import`].
const ARRAY_SECTIONS: &[&str] = &[
    "documents",
    "aiGenerations",
    "applications",
    "referrals",
    "autopilots",
    "interactions",
    "spend",
    "dedupTombstones",
    "discoveredCompanies",
];

/// Sections whose export is a single JSON object. Two shapes share this check:
/// the single-row settings stores, and `pipelineRuns`, whose export is an object
/// of two arrays (`{ runs, events }`) so an event whose run failed to
/// deserialize can't silently vanish with it.
const OBJECT_SECTIONS: &[&str] = &[
    "jobPreferences",
    "contactProfile",
    "aiProviderConfig",
    "pipelineRuns",
];

/// Structurally validate every section that is present in `stores`, returning
/// `Err(message)` for the first section whose top-level shape does not match what
/// its store's `import` requires. Runs BEFORE any store is mutated so a malformed
/// bundle aborts the restore without clearing a store. Only checks presence +
/// top-level shape (array vs object); deep per-record validation happens
/// atomically inside each store's transactional `import`.
fn validate_sections(stores: &Value) -> crate::error::AppResult<()> {
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
    // ADR-007: `ai_generations` is the application aggregate — every persistent
    // field on `AiGenerationRecord` (including `quality_report`) travels through
    // here automatically via `DataStore::export`/`list`/`row_to_record`. A new
    // column only needs a `AiGenerationRecord` field (with `#[serde(default)]`
    // so an older bundle still imports) — nothing to add HERE — but this comment
    // is the lockstep reminder to actually add that field, not skip it.
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
    // Active AI provider config (task #16). Holds NO secrets — API keys stay in the
    // OS keychain — so backing it up is safe; a factory reset clears it separately.
    if let Some(s) = app.try_state::<AiConfigStore>() {
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
    if let Some(s) = app.try_state::<SpendStore>() {
        stores.insert(s.key().to_string(), s.export());
    }
    // Cross-board dedup "not a duplicate" verdicts (ADR-029). Holds no secrets —
    // opaque `canonical_job_key` pairs — so it round-trips through backups.
    if let Some(s) = app.try_state::<DedupStore>() {
        stores.insert(s.key().to_string(), s.export());
    }
    // Passively-harvested ATS company slugs + watched-company stars (ADR-030).
    // Public slugs + display names only — no secrets — so it round-trips.
    if let Some(s) = app.try_state::<DiscoveredCompanyStore>() {
        stores.insert(s.key().to_string(), s.export());
    }
    // Pipeline/agent run history + its per-stage event trail. Local run
    // bookkeeping (ids, the job URL, stage names, clamped summary artifacts) —
    // no secrets, no generated documents — so it round-trips through backups.
    if let Some(s) = app.try_state::<PipelineRunStore>() {
        stores.insert(s.key().to_string(), s.export());
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

/// Pure gate for the restore-specific relink pass (own doc on
/// `ApplicationStore::relink_legacy_generations_after_restore` for why this
/// exact shape is the safety boundary): true only when the bundle replaced
/// `aiGenerations` but carries no `applications` section at all — the one
/// shape that is verifiably a pre-`ApplicationStore`-era backup, so it can
/// never contain a deliberately-detached row. A modern full backup (both
/// sections present) or a partial one missing `aiGenerations` must both
/// return `false`, or the relink can resurrect a row a user deliberately
/// detached before taking the backup.
fn is_legacy_generations_only_bundle(has_ai_generations: bool, has_applications: bool) -> bool {
    has_ai_generations && !has_applications
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
        // Restore-specific link pass (own doc on `ApplicationStore::
        // relink_legacy_generations_after_restore`): a bundle that replaced
        // `aiGenerations` but carries NO `"applications"` section is
        // verifiably a pre-`ApplicationStore`-era backup, so its orphaned
        // legacy rows can never be a deliberately-detached row (that
        // capability didn't exist yet when it was exported) — safe to
        // re-link/re-create outside the one-shot boot marker, which would
        // otherwise leave them stranded forever (the marker was set long
        // before this import ever ran). A bundle that DOES carry
        // `"applications"` is a modern full backup: its `aiGenerations` rows
        // already restore self-consistent `application_id` links via the two
        // imports above, and it CAN legitimately carry a deliberately
        // detached row — re-scanning that would resurrect it, so this must
        // NOT run for that shape.
        if is_legacy_generations_only_bundle(
            stores.get("aiGenerations").is_some(),
            stores.get("applications").is_some(),
        ) {
            // Same resolution setup uses (`resolve_and_export_data_dir` — own
            // doc there), not a bare `app.path().app_data_dir()`: the latter
            // has no fallback, so on a host where it errors every store still
            // opened fine through the fallback (the boot marker got set) and
            // this relink is the ONLY thing that can repair those rows —
            // skipping it strands them permanently. Idempotent to call again
            // here: the env var is already exported by setup, so this just
            // re-resolves and returns the same directory.
            let data_dir = crate::platform::config::resolve_and_export_data_dir(&app);
            if let Err(e) = s.inner().relink_legacy_generations_after_restore(&data_dir) {
                log::warn!(
                    "[applications] restore relink skipped (non-fatal): {}",
                    e.code()
                );
            }
        }
    }
    if let Some(s) = app.try_state::<JobPreferencesStore>() {
        import_into("jobPreferences", s.inner());
    }
    if let Some(s) = app.try_state::<ContactProfileStore>() {
        import_into("contactProfile", s.inner());
    }
    if let Some(s) = app.try_state::<AiConfigStore>() {
        import_into("aiProviderConfig", s.inner());
    }
    if let Some(s) = app.try_state::<ReferralStore>() {
        import_into("referrals", s.inner());
    }
    if let Some(s) = app.try_state::<std::sync::Arc<Mutex<AutopilotStore>>>() {
        let guard = s.lock();
        import_into("autopilots", &*guard);
    }
    if let Some(s) = app.try_state::<SpendStore>() {
        import_into("spend", s.inner());
    }
    if let Some(s) = app.try_state::<DedupStore>() {
        import_into("dedupTombstones", s.inner());
    }
    if let Some(s) = app.try_state::<DiscoveredCompanyStore>() {
        import_into("discoveredCompanies", s.inner());
    }
    if let Some(s) = app.try_state::<PipelineRunStore>() {
        import_into("pipelineRuns", s.inner());
    }
    // `import_into` (the `imported`/`had_error`-capturing closure) is never
    // called again after this point, so NLL can end its borrow here — letting
    // the interactions block below touch `imported`/`had_error` directly.
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

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::{
        is_legacy_generations_only_bundle, json, validate_sections, DataStore, PipelineRunStore,
        ARRAY_SECTIONS, OBJECT_SECTIONS,
    };
    use crate::pipeline::runs::RunRow;

    /// This file's own source, for the two WIRING pins at the bottom. Both
    /// commands here are `AppHandle`-bound and this crate has no `tauri::test`
    /// mock-app harness, so a source pin is the cheapest honest guard (the
    /// `commands::autopilot` / `pipeline::json` precedent); `include_str!` also
    /// makes rustc track the file, so this can never read a stale copy.
    const SRC: &str = include_str!("data.rs");

    fn a_run() -> RunRow {
        RunRow {
            id: "run-1".to_string(),
            job_url: "https://example.test/job/1".to_string(),
            kind: "resume".to_string(),
            depth: "full".to_string(),
            status: "done".to_string(),
            started_at: 1_700_000_000_000,
            finished_at: Some(1_700_000_050_000),
            stopped_reason: Some("done".to_string()),
            metrics_json: r#"{"steps":3}"#.to_string(),
        }
    }

    /// `pipelineRuns` is the one section whose export is an OBJECT OF TWO ARRAYS
    /// rather than the array shape most stores use, so it belongs in
    /// [`OBJECT_SECTIONS`] — and this pre-pass is what stops a bundle that got
    /// the shape wrong from reaching `import` and clearing the store first.
    #[test]
    fn validate_sections_holds_pipeline_runs_to_its_object_shape() {
        validate_sections(&json!({ "pipelineRuns": { "runs": [], "events": [] } }))
            .expect("the exported `{ runs, events }` shape must validate");

        let err = validate_sections(&json!({ "pipelineRuns": [] }))
            .expect_err("an array-shaped pipelineRuns section must be rejected")
            .to_string();
        assert!(
            err.contains("pipelineRuns"),
            "the error must name the offending section, got: {err}"
        );

        // An absent section is legal: a backup taken before this store existed
        // must still restore everything it does contain.
        validate_sections(&json!({})).expect("a bundle without the section still restores");
    }

    /// **FIX-2 mutation guard.** `is_legacy_generations_only_bundle` is the
    /// entire boundary that stops a modern full backup from resurrecting a
    /// deliberately-detached row (own doc on the function): exactly one of
    /// the four presence combinations may gate the restore-specific relink.
    /// Mutating the `&&` to `||`, or dropping either operand, changes which
    /// combination(s) return `true` — this asserts all four individually so
    /// any such mutation reddens the suite.
    #[test]
    fn is_legacy_generations_only_bundle_gates_exactly_one_combination() {
        assert!(
            !is_legacy_generations_only_bundle(true, true),
            "a modern full backup (both sections present) must NOT relink — \
             it can legitimately carry a deliberately-detached row"
        );
        assert!(
            is_legacy_generations_only_bundle(true, false),
            "aiGenerations replaced with no applications section at all is \
             the one shape verifiably pre-ApplicationStore-era — must relink"
        );
        assert!(
            !is_legacy_generations_only_bundle(false, true),
            "no aiGenerations section means nothing to relink"
        );
        assert!(
            !is_legacy_generations_only_bundle(false, false),
            "neither section present means nothing to relink"
        );
    }

    /// The section key is not a literal anyone maintains twice: it comes from
    /// `DataStore::key()`. This walks the whole bundle contract for one real
    /// store — export → the section key it lands under → the shape pre-pass →
    /// `import` back — which is everything `data_export`/`data_import` do to it
    /// except resolving the store from `AppHandle` state.
    #[test]
    fn the_run_store_round_trips_through_the_bundle_contract() {
        let dir = TempDir::new().unwrap();
        let source = PipelineRunStore::open(dir.path()).unwrap();
        source.upsert_run(&a_run()).unwrap();

        let key = source.key();
        assert!(
            OBJECT_SECTIONS.contains(&key),
            "the store exports under '{key}', which no section list knows about — \
             it would be written into a backup and silently skipped on restore"
        );

        let stores = json!({ key: source.export() });
        validate_sections(&stores).expect("a REAL export must satisfy the pre-pass");

        let dir2 = TempDir::new().unwrap();
        let restored = PipelineRunStore::open(dir2.path()).unwrap();
        let section = stores.get(key).expect("the bundle must carry the section");
        assert_eq!(restored.import(section).unwrap(), 1);
        assert_eq!(restored.run("run-1"), source.run("run-1"));
    }

    /// Pull the type argument out of every `try_state::<T>()` in `src`.
    ///
    /// The needle is assembled at runtime, never written out as one literal:
    /// this module is part of the file it scans, so an inline needle would make
    /// the scan match itself.
    fn state_types(src: &str) -> std::collections::BTreeSet<String> {
        let needle = format!("try_{}::<", "state");
        src.match_indices(&needle)
            .map(|(at, _)| {
                let rest = &src[at + needle.len()..];
                let end = rest
                    .find(">()")
                    .expect("unterminated `try_state` turbofish");
                rest[..end].to_string()
            })
            .collect()
    }

    /// `SRC` from `start` up to `end` (or EOF), both assembled by the caller.
    fn region(start: &str, end: &str) -> &'static str {
        let from = SRC
            .find(start)
            .unwrap_or_else(|| panic!("`{start}` no longer appears in commands/data.rs"));
        let rest = &SRC[from..];
        &rest[..rest.find(end).unwrap_or(rest.len())]
    }

    /// WIRING PIN. A store must be on BOTH sides or neither: one that
    /// `build_bundle` exports but `data_import` never routes back writes a
    /// section into every backup that is silently dropped on restore — the
    /// user's data is in the file and never comes home. `PipelineRunStore` is
    /// asserted by name so deleting it from both sides fails too.
    ///
    /// Rustfmt assumption: a `try_state::<T>()` turbofish stays on one line.
    #[test]
    fn every_store_the_bundle_exports_is_also_restored() {
        let exported = state_types(region(
            &format!("fn build_{}(", "bundle"),
            &format!("pub async fn data_{}(", "export"),
        ));
        let imported = state_types(region(
            &format!("pub async fn data_{}(", "import"),
            &format!("#[cfg({})]", "test"),
        ));

        for (side, types) in [("exported", &exported), ("imported", &imported)] {
            assert!(
                types.iter().any(|t| t.contains("PipelineRunStore")),
                "the pipeline run store is not {side} — its history never leaves \
                 (or never returns to) a backup"
            );
        }
        assert_eq!(
            exported, imported,
            "a store on only one side of the backup is a silent one-way trip"
        );
    }

    /// WIRING PIN. Every section the pre-pass validates must also be ROUTED in
    /// `data_import` — a key that is validated but never imported is a section
    /// that looks supported and restores nothing.
    ///
    /// The scan matches the ROUTING FORM each section actually uses, not a bare
    /// quoted key: every `DataStore` section must go through the
    /// `import_into("<key>", …)` closure, and `interactions` — the ONE section
    /// whose store is not a `DataStore` — must be read straight off the bundle
    /// with `stores.get("interactions")`. A bare-key scan would be satisfied by a
    /// comment, a log line, or an error message naming a section nothing routes.
    /// The form is required PER SECTION rather than as an either/or, because
    /// accepting the bundle-read form for a `DataStore` section would let a lone
    /// `stores.get("<key>")` — a read that inspects the section and imports
    /// nothing — stand in for the missing `import`.
    /// (There are no `match` arms on the key; if routing ever becomes a `match`,
    /// this pin must learn that form too rather than being loosened back.)
    ///
    /// Both needles are assembled at RUNTIME: this module is part of the file it
    /// scans, so a written-out form could let the test satisfy its own scan if
    /// the scanned region ever widened past `#[cfg(test)]`.
    #[test]
    fn every_validated_section_is_routed_on_import() {
        let importer = region(
            &format!("pub async fn data_{}(", "import"),
            &format!("#[cfg({})]", "test"),
        );
        let via_closure = format!("{}_into(", "import");
        let via_bundle = format!("stores.{}(", "get");
        for key in ARRAY_SECTIONS.iter().chain(OBJECT_SECTIONS) {
            let needle = if *key == "interactions" {
                format!("{via_bundle}\"{key}\")")
            } else {
                format!("{via_closure}\"{key}\",")
            };
            assert!(
                importer.contains(&needle),
                "section '{key}' is validated but never routed in data_import — \
                 expected the `{needle}` routing form"
            );
        }
    }
}
