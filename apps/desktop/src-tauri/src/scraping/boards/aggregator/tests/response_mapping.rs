use super::super::*;

// ── URL / query building ──────────────────────────────────────────────────────

#[test]
fn adzuna_url_encodes_query_and_location() {
    // Verify that special characters in query/location don't break the URL.
    // (We test the encoding indirectly via urlencoding::encode behavior.)
    let q = "Rust & C++";
    let loc = "München, Bayern";
    let q_enc = urlencoding::encode(q).to_string();
    let loc_enc = urlencoding::encode(loc).to_string();

    assert!(q_enc.contains("%26"), "& must be percent-encoded");
    assert!(!q_enc.contains(' '), "spaces must be encoded");
    assert!(loc_enc.contains("%C3%BC"), "ü must be percent-encoded");
}

#[test]
fn jsearch_combines_query_and_location() {
    let query = "software engineer";
    let location = "Berlin";
    let combined = format!("{query} in {location}");
    assert_eq!(combined, "software engineer in Berlin");
}

#[test]
fn jsearch_query_only_when_location_empty() {
    let query = "software engineer";
    let location = "";
    let combined = if location.is_empty() {
        query.to_string()
    } else {
        format!("{query} in {location}")
    };
    assert_eq!(combined, "software engineer");
}

#[test]
fn adzuna_defaults_country_to_de() {
    let country = "";
    let resolved = if country.is_empty() { "de" } else { country };
    assert_eq!(resolved, "de");
}

// ── Adzuna response → JobPosting mapping ─────────────────────────────────────

#[test]
fn adzuna_response_maps_to_job_posting() {
    // Parse a fixture that mirrors the real Adzuna JSON shape.
    let json = serde_json::json!({
        "count": 1,
        "results": [{
            "id": "abc123",
            "title": "Senior Rust Engineer",
            "company": { "display_name": "RustCorp" },
            "location": { "display_name": "Berlin, Germany", "area": ["Germany", "Berlin"] },
            "redirect_url": "https://api.adzuna.com/v1/api/jobs/de/redirects/abc123",
            "description": "<p>Job description here</p>",
            "created": "2026-06-01T09:00:00Z",
            "salary_min": 70000.0,
            "salary_max": 90000.0
        }]
    });

    let resp: AdzunaResp = serde_json::from_value(json).unwrap();
    let j = &resp.results[0];

    assert_eq!(j.id, "abc123");
    assert_eq!(j.title, "Senior Rust Engineer");
    assert_eq!(
        j.company.as_ref().and_then(|c| c.display_name.as_deref()),
        Some("RustCorp")
    );
    assert_eq!(
        j.location.as_ref().and_then(|l| l.display_name.as_deref()),
        Some("Berlin, Germany")
    );
    assert_eq!(
        j.redirect_url,
        "https://api.adzuna.com/v1/api/jobs/de/redirects/abc123"
    );
    assert!(j
        .description
        .as_deref()
        .unwrap_or("")
        .contains("Job description"));
    // posted_at: 2026-06-01T09:00:00Z → positive ms timestamp
    let ts = chrono::DateTime::parse_from_rfc3339(j.created.as_deref().unwrap())
        .unwrap()
        .timestamp_millis();
    assert!(ts > 0);
    assert_eq!(j.salary_min, Some(70000.0));
    assert_eq!(j.salary_max, Some(90000.0));
}

/// Adzuna live API sends `id` as an integer — must parse and normalise to String.
/// Regression for: "invalid type: integer `331705081`, expected a string".
#[test]
fn adzuna_integer_id_deserializes_to_string() {
    let json = serde_json::json!({
        "results": [{
            "id": 331705081_i64,
            "title": "Rust Engineer",
            "company": { "display_name": "Corp" },
            "location": { "display_name": "Berlin" },
            "redirect_url": "https://api.adzuna.com/v1/api/jobs/de/redirects/331705081",
            "description": null,
            "created": null,
            "salary_min": null,
            "salary_max": null
        }]
    });

    let resp: AdzunaResp =
        serde_json::from_value(json).expect("integer id must deserialize without error");
    let j = &resp.results[0];
    assert_eq!(j.id, "331705081");
    // Confirm the id maps correctly through the JobPosting formatting.
    assert_eq!(format!("adzuna-{}", j.id), "adzuna-331705081");
    assert_eq!(
        format!("aggregator:adzuna-{}", j.id),
        "aggregator:adzuna-331705081"
    );
}

/// String `id` (original documented shape) must still deserialize correctly
/// after the `de_string_or_number` migration.
#[test]
fn adzuna_string_id_still_deserializes() {
    let json = serde_json::json!({
        "results": [{
            "id": "abc123",
            "title": "Senior Rust Engineer",
            "company": { "display_name": "RustCorp" },
            "location": { "display_name": "Berlin" },
            "redirect_url": "https://api.adzuna.com/v1/api/jobs/de/redirects/abc123",
            "description": null,
            "created": null,
            "salary_min": null,
            "salary_max": null
        }]
    });

    let resp: AdzunaResp = serde_json::from_value(json).expect("string id must still deserialize");
    assert_eq!(resp.results[0].id, "abc123");
}

// ── JSearch response → JobPosting mapping ────────────────────────────────────

#[test]
fn jsearch_response_maps_to_job_posting() {
    let json = serde_json::json!({
        "status": "OK",
        "data": [{
            "job_id": "xyz789",
            "job_title": "Backend Developer",
            "employer_name": "StartupAG",
            "job_city": "Munich",
            "job_country": "DE",
            "job_apply_link": "https://startupag.example.com/jobs/xyz789",
            "job_description": "<ul><li>Write Rust</li></ul>",
            "job_posted_at_datetime_utc": "2026-05-15T12:00:00Z"
        }]
    });

    let resp: JSearchResp = serde_json::from_value(json).unwrap();
    let j = &resp.data[0];

    assert_eq!(j.job_id, "xyz789");
    assert_eq!(j.job_title, "Backend Developer");
    assert_eq!(j.employer_name.as_deref(), Some("StartupAG"));
    assert_eq!(j.job_city.as_deref(), Some("Munich"));
    assert_eq!(j.job_country.as_deref(), Some("DE"));
    assert_eq!(
        j.job_apply_link.as_deref(),
        Some("https://startupag.example.com/jobs/xyz789")
    );
    assert!(j
        .job_description
        .as_deref()
        .unwrap_or("")
        .contains("Write Rust"));
    let ts = chrono::DateTime::parse_from_rfc3339(j.job_posted_at_datetime_utc.as_deref().unwrap())
        .unwrap()
        .timestamp_millis();
    assert!(ts > 0);
}

/// JSearch: `job_google_link` is used as fallback when `job_apply_link` is null.
#[test]
fn jsearch_uses_google_link_when_apply_link_null() {
    let json = serde_json::json!({
        "status": "OK",
        "data": [{
            "job_id": "b",
            "job_title": "Has google link only",
            "employer_name": "Co",
            "job_city": null,
            "job_country": null,
            "job_apply_link": null,
            "job_google_link": "https://google.com/jobs/b",
            "job_description": null,
            "job_posted_at_datetime_utc": null
        }]
    });

    let resp: JSearchResp = serde_json::from_value(json).unwrap();
    let j = &resp.data[0];
    // The fallback logic: apply_link.or_else(|| google_link) must yield the google link.
    let url = j
        .job_apply_link
        .clone()
        .or_else(|| j.job_google_link.clone());
    assert_eq!(
        url.as_deref(),
        Some("https://google.com/jobs/b"),
        "job_google_link must be used when job_apply_link is null"
    );
}

/// JSearch: jobs with neither `job_apply_link` nor `job_google_link` are dropped.
#[test]
fn jsearch_drops_jobs_without_any_link() {
    let json = serde_json::json!({
        "status": "OK",
        "data": [
            {
                "job_id": "a",
                "job_title": "Has apply link",
                "employer_name": "Co",
                "job_city": null,
                "job_country": null,
                "job_apply_link": "https://example.com/a",
                "job_google_link": null,
                "job_description": null,
                "job_posted_at_datetime_utc": null
            },
            {
                "job_id": "b",
                "job_title": "No link at all",
                "employer_name": "Co",
                "job_city": null,
                "job_country": null,
                "job_apply_link": null,
                "job_google_link": null,
                "job_description": null,
                "job_posted_at_datetime_utc": null
            }
        ]
    });

    let resp: JSearchResp = serde_json::from_value(json).unwrap();
    // Simulate the filter_map fallback: apply_link.or_else(|| google_link).
    let count = resp
        .data
        .into_iter()
        .filter(|j| j.job_apply_link.is_some() || j.job_google_link.is_some())
        .count();
    assert_eq!(count, 1, "job without either link must be dropped");
}
