use serde_json::json;
use serde_json::Value;
use tauri::AppHandle;
use tauri::Manager;

#[tauri::command]
pub async fn job_preferences_get(app: AppHandle) -> Value {
    let store = app.state::<crate::job_preferences::JobPreferencesStore>();
    let prefs = store.get();
    json!(prefs)
}

/// The legacy wire names [`JobPreferences`](crate::job_preferences::JobPreferences)
/// still accepts (`#[serde(alias)]`, #1149) mapped to their canonical keys.
/// [`merge_over_stored`] drops the stored row's canonical spelling whenever the
/// caller addressed that same field under its old name — and drops the CALLER's
/// legacy key instead when the body carries both spellings itself (canonical
/// wins): serde treats a rename and its alias as ONE field and rejects both
/// spellings as a duplicate, so the merged object must never carry them
/// together, whichever side contributed the second one.
const LEGACY_WIRE_NAMES: &[(&str, &str)] = &[("tech_stack", "techStack")];

/// Overlays the caller's PRESENT keys onto the serialized stored row, which is
/// what turns `job_preferences_set` from a full-row overwrite into a merge —
/// see [`set_job_preferences`] for the rule and why it is load-bearing. Absent
/// keys survive because the base contributes them; a key present as JSON `null`
/// wins the overlay and deserializes to `None`, clearing the column.
///
/// Only fields the store actually holds appear in the base: every
/// [`JobPreferences`](crate::job_preferences::JobPreferences) field is
/// `skip_serializing_if = "Option::is_none"`, so an unset column contributes no
/// key and behaves exactly as it does today.
fn merge_over_stored(
    stored: &crate::job_preferences::JobPreferences,
    mut incoming: serde_json::Map<String, Value>,
) -> serde_json::Map<String, Value> {
    let mut merged = match serde_json::to_value(stored) {
        Ok(Value::Object(map)) => map,
        // Unreachable for this struct (plain `Option` fields, no map keys that
        // can fail to serialize); an empty base degrades to the pre-#1149
        // overwrite instead of panicking.
        _ => serde_json::Map::new(),
    };
    for (legacy, canonical) in LEGACY_WIRE_NAMES {
        if !incoming.contains_key(*legacy) {
            continue;
        }
        if incoming.contains_key(*canonical) {
            // The caller sent BOTH spellings of one field. Serde would refuse
            // the pair as a duplicate — the same refusal, reported as
            // "missing or wrong type", that the stored-key removal below
            // exists to avoid. The canonical key wins and the alias is
            // dropped: a body carrying both is a caller mid-migration, and
            // the new name is the one it means.
            incoming.remove(*legacy);
        } else {
            merged.remove(*canonical);
        }
    }
    merged.extend(incoming);
    merged
}

/// Merges a `job_preferences_set` body over the stored row and deserializes
/// the result, REPORTING a failure instead of substituting a default (#1133).
/// The previous `unwrap_or(<all-None>)` made a
/// malformed body indistinguishable from a real save:
/// [`JobPreferencesStore::set`](crate::job_preferences::JobPreferencesStore::set)
/// is a full-row `UPDATE`, so the substituted default NULLed location, country,
/// tech stack, salary and agency list while the reply still said
/// `{"success": true}` — and this command is `Effect::Reversible` in the agent
/// CLI's policy table, so no confirmation step stands between a caller and that.
/// `DataStore::import` already propagates the identical error for this same
/// struct with `?`; this is the command-input mirror of that.
///
/// The message deliberately does NOT carry `serde_json`'s own text: that quotes
/// the offending value back (`invalid type: string "…"`), which would echo the
/// caller's body — a location, a salary figure — into a reply that agent/MCP
/// clients log verbatim. What it reports is derived from the body's JSON *type*
/// alone, which is enough to fix the call: every [`JobPreferences`](crate::job_preferences::JobPreferences)
/// field is `Option<_>` and unknown keys are ignored, so an object can only
/// fail by carrying a wrongly-typed field (or one missing a required sub-field,
/// e.g. a `techStack` entry without its `category` — hence "missing or"), and a
/// non-object can only fail by not being an object.
///
/// Pure (no `AppHandle`, no store — the caller passes the stored row in) so
/// every branch is unit-testable: this crate has no `tauri::test` mock-app
/// harness (see the test module below).
fn parse_job_preferences(
    stored: &crate::job_preferences::JobPreferences,
    prefs: Value,
) -> crate::error::AppResult<crate::job_preferences::JobPreferences> {
    let detail = match &prefs {
        Value::Object(_) => "a field is missing or has the wrong type",
        Value::Null => "expected an object, got null",
        Value::Bool(_) => "expected an object, got a boolean",
        Value::Number(_) => "expected an object, got a number",
        Value::String(_) => "expected an object, got a string",
        Value::Array(_) => "expected an object, got an array",
    };
    let body = match prefs {
        Value::Object(incoming) => Value::Object(merge_over_stored(stored, incoming)),
        // Not an object: passed through untouched so the deserialize below
        // fails and reports `detail` — there is nothing to merge onto.
        other => other,
    };
    serde_json::from_value(body)
        .map_err(|_| crate::error::AppError::Validation(format!("invalid prefs: {detail}")))
}

/// The body → store step of [`job_preferences_set`], split out so the
/// malformed-body and merge branches can be driven against a REAL store
/// in-process (the command itself needs an `AppHandle` this crate cannot mock).
/// The wire shape is the sibling setters' one, unchanged for the renderer:
/// `{"success": true}` on a write, `{"error": …}` on a refusal or a store
/// failure.
///
/// **Merge semantics — an absent key KEEPS the stored value, an explicit JSON
/// `null` CLEARS it.** [`JobPreferencesStore::set`](crate::job_preferences::JobPreferencesStore::set)
/// is a full-row `UPDATE` and every [`JobPreferences`](crate::job_preferences::JobPreferences)
/// field is `Option<_>`, so a well-formed body still parsed fine while NULLing
/// every column it omitted — and answered `{"success": true}`. #1133 only
/// closed the *malformed*-body half of that; this closes the rest by overlaying
/// the caller's present keys onto the stored row ([`merge_over_stored`]) before
/// deserializing. Both real callers land correctly under this rule:
/// - the renderer sends a full-row spread (`{...jobPrefs, location}`), so every
///   key is present and it overwrites exactly as before — including the
///   `techStack` it could not even see before the #1149 rename, whose absence
///   from that spread nulled the user's saved tech stack on every location save;
/// - the agent/MCP tier sends partial bodies (`{"location": "Lisbon"}`), which
///   now touch only what they name — this command is `Effect::Reversible` in
///   the agent CLI policy table, so no confirmation step stands between a
///   caller and a wipe.
///
/// Read-modify-write on the store's single settings row: the read and the write
/// take the connection lock separately, so two concurrent `set`s can interleave
/// (last writer wins per column). That is the pre-existing shape of every
/// setter here and is unreachable in practice — one desktop user, one row.
///
/// The `{"error": …}` reply exists for the agent/MCP tier. The renderer cannot
/// produce it: its `set()` is typed to the shared `JobPreferences` contract, so
/// any body it sends parses. It is also the same hazard
/// `job_preferences_set_semantic_scoring` documents
/// below — a `Value` return RESOLVES the invoke promise even for an error
/// object, so a renderer `onError`/`.catch` would never run. That is tolerable
/// here only because the renderer has no reachable path to the error branch.
fn set_job_preferences(store: &crate::job_preferences::JobPreferencesStore, prefs: Value) -> Value {
    let job_prefs = match parse_job_preferences(&store.get(), prefs) {
        Ok(job_prefs) => job_prefs,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    match store.set(&job_prefs) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
pub async fn job_preferences_set(app: AppHandle, prefs: Value) -> Value {
    let store = app.state::<crate::job_preferences::JobPreferencesStore>();
    set_job_preferences(&store, prefs)
}

/// Single-column extra-agency-companies write (ADR-029 §i) — mirrors
/// `job_preferences_set_salary_expectation`: it delegates to
/// `JobPreferencesStore::set_extra_agency_companies`, touching ONLY that column,
/// so a Settings edit of the agency list can never NULL the user's saved
/// location/tech stack/country/salary via a stale full-row payload (PR #695).
#[tauri::command]
pub async fn job_preferences_set_extra_agency_companies(
    app: AppHandle,
    companies: Option<Vec<String>>,
) -> Value {
    let store = app.state::<crate::job_preferences::JobPreferencesStore>();
    match store.set_extra_agency_companies(companies) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Single-column salary-expectation write (review fix, PR #695) — mirrors
/// `job_preferences_set` but delegates to
/// `JobPreferencesStore::set_salary_expectation`, which touches ONLY that
/// column. Callers (`ApplicantDetailsSection`'s onChange, the boot-time sync
/// hook) that don't have a freshly-read `location`/`tech_stack`/`country_code`
/// on hand are safe either way now that `set_job_preferences` merges: a body
/// that omits a key KEEPS the stored value, one carrying an explicit `null`
/// CLEARS it. These single-column setters remain the explicit way to touch one
/// column — no full-row body, so no stale-payload question to reason about.
#[tauri::command]
pub async fn job_preferences_set_salary_expectation(
    app: AppHandle,
    salary_expectation: Option<String>,
) -> Value {
    let store = app.state::<crate::job_preferences::JobPreferencesStore>();
    match store.set_salary_expectation(salary_expectation) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Single-column semantic-scoring write (ADR-020 addendum) — the renderer's
/// `semanticScoring` preference lives in the webview's `localStorage`, which no
/// Rust code can read, so the headless Autopilot scheduler needs this mirror to
/// know whether to run the semantic re-rank. Same single-column discipline as
/// `job_preferences_set_salary_expectation`: it can never NULL another column.
///
/// Returns `AppResult<()>` — the `email_watch_*` shape — NOT the sibling
/// setters' `Value`-with-an-`error`-key. That difference is load-bearing: a
/// `Value` return RESOLVES the invoke promise even for `{"error": …}`, so the
/// renderer's `onError` / `.catch` never runs and a failed write is invisible.
/// This particular write is the one whose silent failure diverges two scoring
/// surfaces (the user turns semantic scoring OFF, the mirror write fails, and
/// the headless scheduler keeps embedding), so it must REJECT. `AppError`
/// serializes as a plain string, so the rejection carries the store's message.
#[tauri::command]
pub async fn job_preferences_set_semantic_scoring(
    app: AppHandle,
    enabled: bool,
) -> crate::error::AppResult<()> {
    let store = app.state::<crate::job_preferences::JobPreferencesStore>();
    store.set_semantic_scoring(enabled)
}

#[cfg(test)]
mod test {
    use tempfile::TempDir;

    use super::*;
    use crate::job_preferences::{JobPreferences, JobPreferencesStore, TechStackItem};

    /// A store with a full row of real preferences saved — the state #1133's
    /// silent default-substitution wiped. Every column is populated, so a
    /// full-row `UPDATE` slipping through is visible in any of them.
    fn store_with_saved_preferences() -> (TempDir, JobPreferencesStore) {
        let dir = TempDir::new().unwrap();
        let store = JobPreferencesStore::open(&dir.path().to_path_buf()).unwrap();
        store
            .set(&JobPreferences {
                location: Some("Berlin".to_string()),
                country_code: Some("DE".to_string()),
                tech_stack: Some(vec![TechStackItem {
                    name: "Rust".to_string(),
                    category: "language".to_string(),
                }]),
                salary_expectation: Some("€75,000".to_string()),
                extra_agency_companies: Some(vec!["Hays".to_string()]),
            })
            .unwrap();
        (dir, store)
    }

    /// #1133: the reported repro — `prefs` is a bare string, not an object. It
    /// must be REFUSED, and (the part that made this data loss rather than a
    /// bad message) the saved row must survive untouched. Before the fix this
    /// returned `{"success": true}` after NULLing all five columns.
    #[test]
    fn a_malformed_body_is_refused_and_leaves_every_saved_column_intact() {
        let (_dir, store) = store_with_saved_preferences();

        let reply = set_job_preferences(&store, json!("this-is-a-malformed-string-not-an-object"));

        assert!(
            reply.get("success").is_none(),
            "a body that did not parse must never report success: {reply}"
        );
        let error = reply["error"].as_str().expect("an error string");
        assert!(error.starts_with("invalid prefs:"), "got {error}");

        let after = store.get();
        assert_eq!(after.location.as_deref(), Some("Berlin"));
        assert_eq!(after.country_code.as_deref(), Some("DE"));
        assert_eq!(
            after.tech_stack.map(|ts| ts.len()),
            Some(1),
            "the saved tech stack must not be cleared by a refused write"
        );
        assert_eq!(after.salary_expectation.as_deref(), Some("€75,000"));
        assert_eq!(after.extra_agency_companies, Some(vec!["Hays".to_string()]));
    }

    /// The other half of the refusal path: an object whose field carries the
    /// wrong type (the likelier real-world shape — a client-side serialization
    /// bug). Also pins the hardening: the message is built from the body's JSON
    /// type, so it can never echo the caller's own values back into a reply
    /// that agent/MCP clients log.
    #[test]
    fn a_wrongly_typed_field_is_refused_without_echoing_the_body_back() {
        let (_dir, store) = store_with_saved_preferences();

        let reply = set_job_preferences(
            &store,
            json!({ "location": 4_242_424_242_i64, "salaryExpectation": "SECRET-SALARY-90000" }),
        );

        let error = reply["error"].as_str().expect("an error string");
        assert!(
            !error.contains("SECRET-SALARY-90000") && !error.contains("4242424242"),
            "the reply must not echo the caller's body: {error}"
        );
        assert_eq!(
            store.get().location.as_deref(),
            Some("Berlin"),
            "nothing may be written when the body is refused"
        );
    }

    /// The renderer-facing contract: a well-formed body still resolves with the
    /// exact `{"success": true}` object it has always returned (the renderer's
    /// `useSetJobPreferences` mutation and the agent CLI both read this shape),
    /// and the values reach the store. Its `tech_stack` key is deliberate — it
    /// is the pre-#1149 wire name, so this also pins that the `serde(alias)`
    /// still parses an older caller's body end to end.
    #[test]
    fn a_valid_body_saves_and_returns_the_unchanged_success_shape() {
        let dir = TempDir::new().unwrap();
        let store = JobPreferencesStore::open(&dir.path().to_path_buf()).unwrap();

        let reply = set_job_preferences(
            &store,
            json!({
                "location": "Lisbon",
                "countryCode": "PT",
                "tech_stack": [{ "name": "Rust", "category": "language" }],
                "salaryExpectation": "€75,000",
                "extraAgencyCompanies": ["Hays"],
            }),
        );

        assert_eq!(reply, json!({ "success": true }));
        let saved = store.get();
        assert_eq!(saved.location.as_deref(), Some("Lisbon"));
        assert_eq!(saved.country_code.as_deref(), Some("PT"));
        assert_eq!(
            saved.tech_stack.as_ref().map(|ts| ts[0].name.as_str()),
            Some("Rust")
        );
        assert_eq!(saved.salary_expectation.as_deref(), Some("€75,000"));
        assert_eq!(saved.extra_agency_companies, Some(vec!["Hays".to_string()]));
    }

    /// Merge semantics, the KEEP half — the security review's HIGH. A body
    /// that names one field must touch only that column. Before the merge this
    /// NULLed the tech stack, salary, country and agency list and still
    /// answered `{"success": true}`, and it is precisely what the renderer
    /// produced: `job_preferences_get` returned `tech_stack` while the UI read
    /// and re-sent `techStack` (#1149), so every location save dropped the
    /// user's saved tech stack. Revert `parse_job_preferences` to the plain
    /// full-row `serde_json::from_value(prefs)` and this fails.
    #[test]
    fn a_partial_body_keeps_every_column_it_does_not_name() {
        let (_dir, store) = store_with_saved_preferences();

        let reply = set_job_preferences(&store, json!({ "location": "Lisbon" }));

        assert_eq!(reply, json!({ "success": true }));
        let after = store.get();
        assert_eq!(
            after.location.as_deref(),
            Some("Lisbon"),
            "the one named column is written"
        );
        assert_eq!(
            after.tech_stack.as_ref().map(|ts| ts[0].name.as_str()),
            Some("Rust"),
            "an absent techStack must keep the stored tech stack, not NULL it"
        );
        assert_eq!(
            after.salary_expectation.as_deref(),
            Some("€75,000"),
            "an absent salaryExpectation must keep the stored salary"
        );
        assert_eq!(after.country_code.as_deref(), Some("DE"));
        assert_eq!(after.extra_agency_companies, Some(vec!["Hays".to_string()]));
    }

    /// Merge semantics, the CLEAR half — "absent" and "explicit null" must not
    /// collapse into one meaning, or a caller would have no way to say "clear
    /// this" (the UI overwrites with an explicit `techStack: []`; an agent body
    /// sends `null`). The null clears its own column and nothing else.
    #[test]
    fn an_explicit_null_clears_only_the_column_it_names() {
        let (_dir, store) = store_with_saved_preferences();

        let reply = set_job_preferences(&store, json!({ "techStack": null }));

        assert_eq!(reply, json!({ "success": true }));
        let after = store.get();
        assert_eq!(
            after.tech_stack, None,
            "an explicit null must clear the column it names"
        );
        assert_eq!(after.location.as_deref(), Some("Berlin"));
        assert_eq!(after.country_code.as_deref(), Some("DE"));
        assert_eq!(after.salary_expectation.as_deref(), Some("€75,000"));
        assert_eq!(after.extra_agency_companies, Some(vec!["Hays".to_string()]));
    }

    /// The renderer's own shape — a full-row spread — must still overwrite
    /// EVERY column over a populated row. The merge preserves absent keys; it
    /// must never prefer a stored value over one the caller actually sent.
    #[test]
    fn a_full_body_still_overwrites_every_column() {
        let (_dir, store) = store_with_saved_preferences();

        let reply = set_job_preferences(
            &store,
            json!({
                "location": "Lisbon",
                "countryCode": "PT",
                "techStack": [{ "name": "TypeScript", "category": "language" }],
                "salaryExpectation": "€90,000",
                "extraAgencyCompanies": ["Michael Page"],
            }),
        );

        assert_eq!(reply, json!({ "success": true }));
        let after = store.get();
        assert_eq!(after.location.as_deref(), Some("Lisbon"));
        assert_eq!(after.country_code.as_deref(), Some("PT"));
        assert_eq!(
            after.tech_stack.as_ref().map(|ts| ts[0].name.as_str()),
            Some("TypeScript")
        );
        assert_eq!(after.salary_expectation.as_deref(), Some("€90,000"));
        assert_eq!(
            after.extra_agency_companies,
            Some(vec!["Michael Page".to_string()])
        );
    }

    /// The alias must survive the MERGE, not just a bare deserialize: a
    /// pre-#1149 caller sends `tech_stack` while the stored row now serializes
    /// as `techStack`, and serde treats a rename and its alias as one field —
    /// so leaving both in the merged object makes it refuse the body as a
    /// duplicate. Delete the `LEGACY_WIRE_NAMES` loop in `merge_over_stored`
    /// and this fails (the other alias test runs against an empty store, where
    /// no stored key exists to collide with).
    #[test]
    fn a_legacy_tech_stack_key_overwrites_a_stored_tech_stack() {
        let (_dir, store) = store_with_saved_preferences();

        let reply = set_job_preferences(
            &store,
            json!({ "tech_stack": [{ "name": "Go", "category": "language" }] }),
        );

        assert_eq!(
            reply,
            json!({ "success": true }),
            "a legacy body must not be refused as a duplicate field: {reply}"
        );
        let after = store.get();
        assert_eq!(
            after.tech_stack.as_ref().map(|ts| ts[0].name.as_str()),
            Some("Go")
        );
        assert_eq!(
            after.location.as_deref(),
            Some("Berlin"),
            "…and it is still a merge: the unnamed columns survive"
        );
    }

    /// A body carrying BOTH spellings of one field used to be refused outright
    /// ("a field is missing or has the wrong type") — the merge only removed
    /// the STORED canonical key, so the caller's own pair still reached serde,
    /// which sees a rename and its alias as one field. The canonical key wins
    /// and the alias is dropped. Delete the `incoming.remove(*legacy)` branch
    /// and this fails with that refusal.
    #[test]
    fn a_body_carrying_both_spellings_keeps_the_canonical_one() {
        let (_dir, store) = store_with_saved_preferences();

        let reply = set_job_preferences(
            &store,
            json!({
                "techStack": [{ "name": "Rust", "category": "language" }],
                "tech_stack": [{ "name": "Go", "category": "language" }],
            }),
        );

        assert_eq!(
            reply,
            json!({ "success": true }),
            "both spellings in one body must not be refused as a duplicate: {reply}"
        );
        let after = store.get();
        assert_eq!(
            after.tech_stack.as_ref().map(|ts| ts[0].name.as_str()),
            Some("Rust"),
            "the canonical key wins"
        );
        assert_eq!(
            after.location.as_deref(),
            Some("Berlin"),
            "…and it is still a merge: the unnamed columns survive"
        );
    }

    /// A compile-time pin on the wire contract above. Reverting this command to
    /// the sibling `Value` shape (`{"error": …}`) makes the crate's tests fail
    /// to build here — which is the only place the difference is observable
    /// in-process: `invoke`'s resolve-vs-reject behaviour is decided by this
    /// return type, and this crate has no `tauri::test` mock-app harness to
    /// drive the command end to end.
    #[test]
    fn the_semantic_scoring_mirror_rejects_instead_of_resolving_an_error_object() {
        fn assert_rejects_on_failure<F, Fut>(_command: F)
        where
            F: Fn(AppHandle, bool) -> Fut,
            Fut: std::future::Future<Output = crate::error::AppResult<()>>,
        {
        }
        assert_rejects_on_failure(job_preferences_set_semantic_scoring);
    }
}
