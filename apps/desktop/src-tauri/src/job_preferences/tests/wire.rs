use super::*;

// ── Wire shape (#1149) ────────────────────────────────────────────────────────

/// The serialized key set IS the renderer contract: `job_preferences_get` hands
/// this straight to the UI, and `JobPreferencesSchema` (packages/shared)
/// declares exactly these camelCase names. Pinning the whole set catches a
/// field that ships without its `#[serde(rename)]` — the #1149 defect, where
/// `tech_stack` went out under a name the UI never read, so the UI's full-row
/// save omitted it and `set`'s `UPDATE` NULLed the user's tech stack.
#[test]
fn the_serialized_wire_keys_are_the_camel_case_contract() {
    let prefs = JobPreferences {
        location: Some("Berlin".to_string()),
        country_code: Some("DE".to_string()),
        tech_stack: Some(vec![TechStackItem {
            name: "Rust".to_string(),
            category: "language".to_string(),
        }]),
        salary_expectation: Some("€75,000".to_string()),
        extra_agency_companies: Some(vec!["Hays".to_string()]),
    };

    let wire = serde_json::to_value(&prefs).unwrap();
    let mut keys: Vec<&str> = wire
        .as_object()
        .expect("JobPreferences serializes as an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    let mut expected = vec![
        "location",
        "countryCode",
        "techStack",
        "salaryExpectation",
        "extraAgencyCompanies",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected, "wire keys drifted from the shared contract");

    // …and the same document deserializes back unchanged (the rename applies to
    // both directions, so the renderer's full-row spread round-trips).
    let back: JobPreferences = serde_json::from_value(wire).unwrap();
    assert_eq!(back.location, prefs.location);
    assert_eq!(back.country_code, prefs.country_code);
    assert_eq!(back.tech_stack, prefs.tech_stack);
    assert_eq!(back.salary_expectation, prefs.salary_expectation);
    assert_eq!(back.extra_agency_companies, prefs.extra_agency_companies);
}

/// Back-compat for the #1149 rename: a body written against the OLD snake_case
/// wire name must still parse — an agent/MCP caller built before the rename,
/// and (the one that would lose data silently) a backup exported before it and
/// restored through `DataStore::import`, which deserializes this same struct.
#[test]
fn the_legacy_tech_stack_wire_name_still_deserializes_via_the_alias() {
    let prefs: JobPreferences = serde_json::from_value(serde_json::json!({
        "location": "Berlin",
        "tech_stack": [{ "name": "Rust", "category": "language" }],
    }))
    .expect("the pre-#1149 wire name must still parse via the serde alias");

    assert_eq!(
        prefs.tech_stack.as_deref(),
        Some(
            [TechStackItem {
                name: "Rust".to_string(),
                category: "language".to_string(),
            }]
            .as_slice()
        )
    );
}
