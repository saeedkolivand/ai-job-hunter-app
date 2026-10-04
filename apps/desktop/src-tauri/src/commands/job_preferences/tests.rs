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

/// The one coupled pair: `countryCode` is captured with `location` from a
/// single geocode pick, so clearing the location must clear the country
/// with it. A body that names only `location: null` used to leave the
/// stored `DE` behind — and the aggregator board seeds its country from
/// this row, so that orphan kept steering scrapes to a country the user
/// had just removed. Delete the `countryCode` insertion in
/// `merge_over_stored` and this fails.
#[test]
fn clearing_the_location_clears_the_country_code_with_it() {
    let (_dir, store) = store_with_saved_preferences();

    let reply = set_job_preferences(&store, json!({ "location": null }));

    assert_eq!(reply, json!({ "success": true }));
    let after = store.get();
    assert_eq!(after.location, None);
    assert_eq!(
        after.country_code, None,
        "a country left behind by a cleared location keeps steering scrapes"
    );
    assert_eq!(
        after.tech_stack.as_ref().map(|ts| ts[0].name.as_str()),
        Some("Rust"),
        "…and only that pair clears: the unnamed columns still survive"
    );
    assert_eq!(after.salary_expectation.as_deref(), Some("€75,000"));
    assert_eq!(after.extra_agency_companies, Some(vec!["Hays".to_string()]));
}

/// The renderer's real clear sends BOTH nulls, so the same pair must land
/// through the explicit shape too — the insertion above must not turn a
/// caller-supplied key into a duplicate or otherwise disturb the merge.
#[test]
fn an_explicit_location_and_country_pair_of_nulls_clears_both() {
    let (_dir, store) = store_with_saved_preferences();

    let reply = set_job_preferences(&store, json!({ "location": null, "countryCode": null }));

    assert_eq!(reply, json!({ "success": true }));
    let after = store.get();
    assert_eq!(after.location, None);
    assert_eq!(after.country_code, None);
    assert_eq!(after.salary_expectation.as_deref(), Some("€75,000"));
}

/// The other direction — the geocode pick: a body SETTING the location to a
/// new string carries its own country, and both must be written. The
/// clearing rule is scoped to an explicit `location: null`, so it can never
/// fire here; a broader "location present → clear the country" would write
/// a location with no country at all.
#[test]
fn a_new_location_writes_the_country_code_sent_with_it() {
    let (_dir, store) = store_with_saved_preferences();

    let reply = set_job_preferences(&store, json!({ "location": "Lisbon", "countryCode": "PT" }));

    assert_eq!(reply, json!({ "success": true }));
    let after = store.get();
    assert_eq!(after.location.as_deref(), Some("Lisbon"));
    assert_eq!(after.country_code.as_deref(), Some("PT"));
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
