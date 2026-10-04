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
///
/// One pair of fields is coupled rather than independent — see the
/// `location`/`countryCode` rule at the end of the body.
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
    // `countryCode` is not an independent field: it is captured with `location`
    // from ONE picked geocode suggestion and is meaningless without it. So a
    // body that CLEARS the location clears the country with it — otherwise the
    // orphaned code keeps steering scrapes (the aggregator board seeds its
    // country from this row) for a location the user has removed. Narrow on
    // purpose: only an explicit `location: null` triggers it, and only when the
    // body does not address `countryCode` itself — a caller that sends both
    // (the renderer's clear sends two nulls; a geocode pick sends a new
    // location AND its country) keeps exactly what it sent, and setting the
    // location to a new string never touches a stored country.
    if incoming.get("location").is_some_and(Value::is_null) {
        incoming.entry("countryCode").or_insert(Value::Null);
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
/// `location` and `countryCode` clear TOGETHER: a body whose `location` is an
/// explicit `null` and that does not name `countryCode` clears the country too
/// (see [`merge_over_stored`]). They come from one geocode pick, so a country
/// left behind by a cleared location would keep steering scrapes. This is the
/// rule `JobPreferencesContract.set` (packages/shared) points at.
///
/// Read-merge-write on the store's single settings row runs as ONE critical
/// section — [`JobPreferencesStore::update`](crate::job_preferences::JobPreferencesStore::update)
/// holds the connection lock across the read, the merge and the write. Taking
/// the lock twice (`get()` … `set()`) would let two concurrent partial updates
/// merge from the same snapshot, so the later write erases the earlier one's
/// field — reachable from the agent/MCP tier, where several partial bodies can
/// be in flight at once. Nothing is `.await`ed inside, so no lock is ever held
/// across a yield point.
///
/// [`parse_job_preferences`] stays pure and runs INSIDE that critical section:
/// it must not touch the store (the mutex is not reentrant).
///
/// The `{"error": …}` reply exists for the agent/MCP tier. The renderer cannot
/// produce it: its `set()` is typed to the shared `JobPreferences` contract, so
/// any body it sends parses. It is also the same hazard
/// `job_preferences_set_semantic_scoring` documents
/// below — a `Value` return RESOLVES the invoke promise even for an error
/// object, so a renderer `onError`/`.catch` would never run. That is tolerable
/// here only because the renderer has no reachable path to the error branch.
fn set_job_preferences(store: &crate::job_preferences::JobPreferencesStore, prefs: Value) -> Value {
    match store.update(|stored| parse_job_preferences(stored, prefs)) {
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
mod tests;
