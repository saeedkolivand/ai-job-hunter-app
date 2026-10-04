use super::{support::*, *};

// ── salary_expectation (Task #30) ─────────────────────────────────────────────

#[test]
fn test_salary_expectation_round_trips() {
    let (_dir, store) = open_store();

    store
        .set(&JobPreferences {
            salary_expectation: Some("80k DOE".to_string()),
            ..blank()
        })
        .unwrap();

    assert_eq!(store.get().salary_expectation, Some("80k DOE".to_string()));
}

#[test]
fn test_salary_expectation_defaults_to_none() {
    let (_dir, store) = open_store();
    assert_eq!(store.get().salary_expectation, None);
}

/// A pathological/oversized value is clamped server-side, never trusted as
/// the only write path (mirrors the byte caps `extension_bridge`'s own verbs
/// enforce on untrusted strings).
#[test]
fn test_salary_expectation_is_byte_clamped() {
    let (_dir, store) = open_store();

    // A multi-byte (UTF-8) string well over the 200-byte cap.
    let oversized: String = "€".repeat(150); // 150 * 2 bytes = 300 bytes
    store
        .set(&JobPreferences {
            salary_expectation: Some(oversized),
            ..blank()
        })
        .unwrap();

    let stored = store.get().salary_expectation.unwrap();
    assert!(
        stored.len() <= MAX_SALARY_EXPECTATION_BYTES,
        "stored value must be clamped to the byte cap, got {} bytes",
        stored.len()
    );
    // Never split a multi-byte char — the clamped string must stay valid UTF-8
    // (guaranteed by `String`'s invariant; this call would panic otherwise).
    assert!(stored.is_char_boundary(stored.len()));
}

// ── set_salary_expectation (review fix, PR #695 — single-column write) ───────

/// The whole point of `set_salary_expectation`: unlike `set()`'s full-row
/// write, it must NEVER touch location/tech_stack/country_code — proven here
/// by seeding all three, then calling ONLY `set_salary_expectation` (as if a
/// caller's `useJobPreferences` query hadn't loaded yet, so it has no fresh
/// copy of those fields to spread) and asserting they all survive untouched.
#[test]
fn test_set_salary_expectation_never_clears_other_fields() {
    let (_dir, store) = open_store();

    store
        .set(&JobPreferences {
            location: Some("Berlin".to_string()),
            country_code: Some("de".to_string()),
            tech_stack: Some(vec![TechStackItem {
                name: "Rust".to_string(),
                category: "language".to_string(),
            }]),
            ..blank()
        })
        .unwrap();

    store
        .set_salary_expectation(Some("€75,000".to_string()))
        .unwrap();

    let retrieved = store.get();
    assert_eq!(
        retrieved.location,
        Some("Berlin".to_string()),
        "location must survive a salary-only set"
    );
    assert_eq!(
        retrieved.country_code,
        Some("de".to_string()),
        "country_code must survive a salary-only set"
    );
    assert_eq!(
        retrieved.tech_stack.as_ref().map(Vec::len),
        Some(1),
        "tech_stack must survive a salary-only set"
    );
    assert_eq!(retrieved.salary_expectation, Some("€75,000".to_string()));
}

#[test]
fn test_set_salary_expectation_can_clear_to_none_without_touching_other_fields() {
    let (_dir, store) = open_store();

    store
        .set(&JobPreferences {
            location: Some("Munich".to_string()),
            salary_expectation: Some("€75,000".to_string()),
            ..blank()
        })
        .unwrap();

    store.set_salary_expectation(None).unwrap();

    let retrieved = store.get();
    assert_eq!(retrieved.location, Some("Munich".to_string()));
    assert_eq!(retrieved.salary_expectation, None);
}

#[test]
fn test_set_salary_expectation_is_byte_clamped() {
    let (_dir, store) = open_store();

    let oversized: String = "€".repeat(150); // 300 bytes
    store.set_salary_expectation(Some(oversized)).unwrap();

    let stored = store.get().salary_expectation.unwrap();
    assert!(stored.len() <= MAX_SALARY_EXPECTATION_BYTES);
    assert!(stored.is_char_boundary(stored.len()));
}

// ── extra_agency_companies (ADR-029 §i) ───────────────────────────────────────

#[test]
fn test_extra_agency_companies_round_trip() {
    let (_dir, store) = open_store();

    store
        .set(&JobPreferences {
            extra_agency_companies: Some(vec![
                "Talent Partners".to_string(),
                "Local Recruiters".to_string(),
            ]),
            ..blank()
        })
        .unwrap();

    assert_eq!(
        store.get().extra_agency_companies,
        Some(vec![
            "Talent Partners".to_string(),
            "Local Recruiters".to_string()
        ])
    );
}

#[test]
fn test_extra_agency_companies_clamps_and_drops_blanks() {
    let (_dir, store) = open_store();

    let oversized = "€".repeat(150); // 300 bytes, over the per-entry cap
    store
        .set(&JobPreferences {
            extra_agency_companies: Some(vec![
                "  Padded Agency  ".to_string(), // trimmed
                "   ".to_string(),               // blank → dropped
                oversized,                       // byte-clamped
            ]),
            ..blank()
        })
        .unwrap();

    let stored = store.get().extra_agency_companies.unwrap();
    assert_eq!(stored.len(), 2, "blank entries must be dropped");
    assert_eq!(stored[0], "Padded Agency", "entries are trimmed");
    assert!(
        stored[1].len() <= MAX_AGENCY_COMPANY_BYTES,
        "oversized entry must be byte-clamped"
    );
}

#[test]
fn test_extra_agency_companies_list_length_is_capped() {
    let (_dir, store) = open_store();

    // A list well over the length cap → truncated to MAX_EXTRA_AGENCY_COMPANIES,
    // bounding the single JSON column against a looping/XSS'd renderer.
    let many: Vec<String> = (0..MAX_EXTRA_AGENCY_COMPANIES + 25)
        .map(|i| format!("agency{i}"))
        .collect();
    store.set_extra_agency_companies(Some(many)).unwrap();

    let stored = store.get().extra_agency_companies.unwrap();
    assert_eq!(
        stored.len(),
        MAX_EXTRA_AGENCY_COMPANIES,
        "the extra-agency list must be capped at the server limit"
    );
}

#[test]
fn test_extra_agency_companies_empty_list_stores_as_none() {
    let (_dir, store) = open_store();

    store
        .set(&JobPreferences {
            extra_agency_companies: Some(vec!["   ".to_string()]),
            ..blank()
        })
        .unwrap();

    assert_eq!(
        store.get().extra_agency_companies,
        None,
        "an all-blank list collapses to None (SQL NULL), not an empty array"
    );
}

/// Single-column write must never clobber the other fields (PR #695 pattern).
#[test]
fn test_set_extra_agency_companies_never_clears_other_fields() {
    let (_dir, store) = open_store();

    store
        .set(&JobPreferences {
            location: Some("Berlin".to_string()),
            country_code: Some("de".to_string()),
            salary_expectation: Some("€75,000".to_string()),
            ..blank()
        })
        .unwrap();

    store
        .set_extra_agency_companies(Some(vec!["Hays".to_string()]))
        .unwrap();

    let retrieved = store.get();
    assert_eq!(retrieved.location, Some("Berlin".to_string()));
    assert_eq!(retrieved.country_code, Some("de".to_string()));
    assert_eq!(retrieved.salary_expectation, Some("€75,000".to_string()));
    assert_eq!(
        retrieved.extra_agency_companies,
        Some(vec!["Hays".to_string()])
    );
}
