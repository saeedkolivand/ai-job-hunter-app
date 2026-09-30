use super::super::*;
use super::metadata_and_search::{make_ctx, make_input};

// ---------------------------------------------------------------------------
// parse_breezy_response — fixture-based parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_breezy_response_happy_path_with_posted_at() {
    let json = r#"[
        {
            "name": "Product Designer",
            "url": "https://acme.breezy.hr/p/abc123",
            "published_date": "2024-03-15T00:00:00Z",
            "location": { "name": null, "city": "Austin", "state": "TX", "country": {"name": "USA"}, "is_remote": false }
        }
    ]"#;
    let postings_in: Vec<BzPosting> = serde_json::from_str(json).expect("fixture must parse");
    let (postings, format_drops) = parse_breezy_response(postings_in, "acme", 1_700_000_000_000);

    assert_eq!(postings.len(), 1);
    assert_eq!(
        format_drops, 0,
        "a fully-valid row must not count as a format drop"
    );
    let p = &postings[0];
    assert_eq!(p.title, "Product Designer");
    assert_eq!(p.url, "https://acme.breezy.hr/p/abc123");
    assert_eq!(p.company, "acme");
    assert_eq!(p.location, Some("Austin, TX, USA".to_string()));
    assert_eq!(p.id, format!("breezy:{}", p.url));
    assert_eq!(p.external_id, Some(p.url.clone()));
    assert_eq!(p.source, "breezy");
    assert_eq!(p.captured_at, 1_700_000_000_000);

    let expected_posted_at = chrono::DateTime::parse_from_rfc3339("2024-03-15T00:00:00Z")
        .unwrap()
        .timestamp_millis();
    assert_eq!(p.posted_at, Some(expected_posted_at));
}

/// Live drift (verified 2026-07-11): `location.state` and `location.country`
/// arrive as OBJECTS (`{id, name}`), not bare strings. `BzStateField` +
/// `BzCountry` must accept the object shape so the row deserializes AND the
/// state/country names still contribute to the composed location string.
#[test]
fn parse_breezy_response_accepts_object_state_and_country() {
    let json = r#"[
        {
            "name": "Backend Engineer",
            "url": "https://acme.breezy.hr/p/obj-state",
            "published_date": "2024-02-15T14:37:22.684Z",
            "location": {
                "name": null,
                "city": "Sandy Hook",
                "state": {"id": "CT", "name": "Connecticut"},
                "country": {"name": "United States", "id": "US"},
                "is_remote": false
            }
        }
    ]"#;
    let postings_in: Vec<BzPosting> =
        serde_json::from_str(json).expect("object-shaped state/country must deserialize");
    let (postings, _) = parse_breezy_response(postings_in, "acme", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(
        postings[0].location,
        Some("Sandy Hook, Connecticut, United States".to_string()),
        "object state/country names must still compose into the location"
    );
}

/// A bare-string `state` (older/other tenants) must still deserialize — the
/// untagged `BzStateField` accepts both shapes.
#[test]
fn parse_breezy_response_accepts_string_state() {
    let json = r#"[
        {"name": "X", "url": "https://acme.breezy.hr/p/str-state", "published_date": null,
         "location": {"name": null, "city": "Austin", "state": "TX", "country": null, "is_remote": false}}
    ]"#;
    let postings_in: Vec<BzPosting> = serde_json::from_str(json).unwrap();
    let (postings, _) = parse_breezy_response(postings_in, "acme", 0);
    assert_eq!(postings[0].location, Some("Austin, TX".to_string()));
}

/// `rows_to_jobs` drops a row that can't deserialize (e.g. a bare string where
/// an object is required) and keeps the valid rows — one drifted row no longer
/// zeros the whole company (the exact failure the live object-`state` shape
/// caused under the old atomic `Vec<BzPosting>` deserialize).
#[test]
fn rows_to_jobs_drops_undeserializable_rows_keeps_valid() {
    let values: Vec<serde_json::Value> = serde_json::from_str(
        r#"[
        {"name": "Valid", "url": "https://acme.breezy.hr/p/valid", "published_date": null, "location": null},
        "not-an-object",
        {"name": "Object State", "url": "https://acme.breezy.hr/p/obj", "published_date": null,
         "location": {"state": {"id": "CT", "name": "Connecticut"}}}
    ]"#,
    )
    .unwrap();
    let jobs = rows_to_jobs(values);
    assert_eq!(
        jobs.len(),
        2,
        "the bare-string row is dropped; both object rows (incl. object-state) survive"
    );
}

/// Round-2 review finding: when EVERY row in a batch fails to deserialize
/// (not just a stray one), `rows_to_jobs` must still return an empty `Vec` —
/// this is the exact signal `search()` uses (`raw_row_count > 0 &&
/// rows_to_jobs(..).is_empty()`) to treat the company as a FETCH FAILURE
/// (recorded into `first_fetch_error`) instead of a silent success-with-zero
/// — the same failure class the `location.state` object drift caused before
/// `rows_to_jobs` existed, but now for a retype `rows_to_jobs` also can't
/// parse at all.
#[test]
fn rows_to_jobs_all_rows_undeserializable_returns_empty() {
    let values: Vec<serde_json::Value> =
        serde_json::from_str(r#"["not-an-object", 42, null, ["also", "not", "valid"]]"#).unwrap();
    let jobs = rows_to_jobs(values);
    assert!(
        jobs.is_empty(),
        "every row failing to deserialize must yield an empty Vec (the all-drift signal)"
    );
}

#[test]
fn parse_breezy_response_accepts_bare_date_published_date() {
    let json = r#"[
        {"name": "X", "url": "https://acme.breezy.hr/p/bare-date", "published_date": "2024-03-15", "location": null}
    ]"#;
    let postings_in: Vec<BzPosting> = serde_json::from_str(json).unwrap();
    let (postings, _) = parse_breezy_response(postings_in, "acme", 0);
    assert_eq!(postings.len(), 1);
    assert!(
        postings[0].posted_at.is_some(),
        "bare YYYY-MM-DD date must parse"
    );
}

/// `is_remote` merge branch: no other location info → bare "Remote".
#[test]
fn parse_breezy_response_is_remote_true_with_no_other_info_yields_remote() {
    let json = r#"[
        {
            "name": "Support Engineer",
            "url": "https://acme.breezy.hr/p/support-remote",
            "published_date": null,
            "location": { "name": null, "city": null, "state": null, "country": null, "is_remote": true }
        }
    ]"#;
    let postings_in: Vec<BzPosting> = serde_json::from_str(json).unwrap();
    let (postings, _) = parse_breezy_response(postings_in, "acme", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(postings[0].location, Some("Remote".to_string()));
    assert_eq!(
        postings[0].extra.get("workType").and_then(|v| v.as_str()),
        Some("remote")
    );
}

/// `is_remote:false` and an absent `location` block must both write nothing —
/// missing/false is not evidence of on-site, only "not flagged remote".
#[test]
fn parse_breezy_response_is_remote_false_or_absent_writes_nothing() {
    let json = r#"[
        {
            "name": "Office Role",
            "url": "https://acme.breezy.hr/p/office-role",
            "published_date": null,
            "location": { "name": "Austin", "city": "Austin", "state": null, "country": null, "is_remote": false }
        },
        {
            "name": "No Location Block",
            "url": "https://acme.breezy.hr/p/no-location",
            "published_date": null,
            "location": null
        }
    ]"#;
    let postings_in: Vec<BzPosting> = serde_json::from_str(json).unwrap();
    let (postings, _) = parse_breezy_response(postings_in, "acme", 0);
    assert_eq!(postings.len(), 2);
    for p in &postings {
        assert!(
            !p.extra.contains_key("workType"),
            "false/absent is_remote must not write a guessed workType, got {:?}",
            p.extra.get("workType")
        );
    }
}

/// `is_remote` merge branch: city/state present → base gets ", Remote"
/// appended (not replaced).
#[test]
fn parse_breezy_response_is_remote_true_appends_to_city_state() {
    let json = r#"[
        {
            "name": "Support Engineer",
            "url": "https://acme.breezy.hr/p/support-hybrid",
            "published_date": null,
            "location": { "name": null, "city": "Austin", "state": "TX", "country": null, "is_remote": true }
        }
    ]"#;
    let postings_in: Vec<BzPosting> = serde_json::from_str(json).unwrap();
    let (postings, _) = parse_breezy_response(postings_in, "acme", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(postings[0].location, Some("Austin, TX, Remote".to_string()));
}

/// `is_remote` merge branch: `location.name` already contains "remote"
/// (case-insensitive) → must not duplicate the word.
#[test]
fn parse_breezy_response_is_remote_true_does_not_duplicate_existing_remote_text() {
    let json = r#"[
        {
            "name": "Support Engineer",
            "url": "https://acme.breezy.hr/p/already-remote",
            "published_date": null,
            "location": { "name": "Remote - US", "city": null, "state": null, "country": null, "is_remote": true }
        }
    ]"#;
    let postings_in: Vec<BzPosting> = serde_json::from_str(json).unwrap();
    let (postings, _) = parse_breezy_response(postings_in, "acme", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(postings[0].location, Some("Remote - US".to_string()));
}

#[test]
fn parse_breezy_response_empty_array_returns_empty_vec() {
    let postings_in: Vec<BzPosting> = serde_json::from_str("[]").unwrap();
    let (postings, format_drops) = parse_breezy_response(postings_in, "acme", 0);
    assert!(
        postings.is_empty(),
        "empty array must parse to an empty Vec, not an error"
    );
    assert_eq!(format_drops, 0);
}

/// Missing/empty title and missing/malformed url each drop the row; valid
/// rows in the same payload must still come through. CodeRabbit follow-up
/// (PR #604): each of these 4 drops is a FORMAT drop (missing/blank title or
/// unusable url) — the exact class `rows-dropped:<n>` must count — so
/// `format_drops` pins the boundary at 4, not 0.
#[test]
fn parse_breezy_response_drops_malformed_rows() {
    let json = r#"[
        {"name": "Valid One", "url": "https://acme.breezy.hr/p/valid-one", "published_date": null, "location": null},
        {"name": null, "url": "https://acme.breezy.hr/p/missing-title", "published_date": null, "location": null},
        {"name": "", "url": "https://acme.breezy.hr/p/empty-title", "published_date": null, "location": null},
        {"name": "Missing URL", "url": null, "published_date": null, "location": null},
        {"name": "Malformed URL", "url": "not-a-url", "published_date": null, "location": null},
        {"name": "Valid Two", "url": "https://acme.breezy.hr/p/valid-two", "published_date": null, "location": null}
    ]"#;
    let postings_in: Vec<BzPosting> = serde_json::from_str(json).unwrap();
    let (postings, format_drops) = parse_breezy_response(postings_in, "acme", 0);
    let titles: Vec<&str> = postings.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Valid One", "Valid Two"],
        "malformed rows must be dropped without panicking, valid rows kept: {titles:?}"
    );
    assert_eq!(
        format_drops, 4,
        "all 4 title/url drops must count as format drops (2 title + 2 url)"
    );
}

/// Breezy has no stable job id — the (deduped) posting URL doubles as the
/// id/dedup key: two rows sharing a url dedupe to one, distinct urls are kept.
/// CodeRabbit follow-up (PR #604): a duplicate-url drop is normal multi-listing
/// hygiene, NOT drift — `format_drops` must stay `0` even though a row was
/// dropped, so `rows-dropped:<n>` never fires on a perfectly healthy response.
#[test]
fn parse_breezy_response_dedupes_by_url_distinct_urls_kept() {
    let json = r#"[
        {"name": "First Listing", "url": "https://acme.breezy.hr/p/dup", "published_date": null, "location": null},
        {"name": "Duplicate Listing", "url": "https://acme.breezy.hr/p/dup", "published_date": null, "location": null},
        {"name": "Distinct Listing", "url": "https://acme.breezy.hr/p/other", "published_date": null, "location": null}
    ]"#;
    let postings_in: Vec<BzPosting> = serde_json::from_str(json).unwrap();
    let (postings, format_drops) = parse_breezy_response(postings_in, "acme", 0);
    assert_eq!(
        postings.len(),
        2,
        "duplicate url must be deduped, distinct url kept"
    );
    assert_eq!(
        postings[0].title, "First Listing",
        "first-seen row wins the dedupe"
    );
    assert_eq!(postings[1].url, "https://acme.breezy.hr/p/other");
    assert_eq!(
        format_drops, 0,
        "a duplicate-url drop must NOT count as a format drop — it's hygiene, not drift"
    );
}

/// Regression: a `https://user:pass@evil.example/job` url must be dropped —
/// the userinfo-rejecting URL sanity check applies inside the parser too, not
/// just at the network layer. CodeRabbit follow-up (PR #604): an invalid-url
/// drop IS a format drop.
#[test]
fn parse_breezy_response_rejects_userinfo_url() {
    let json = r#"[
        {"name": "Phishy Listing", "url": "https://user:pass@evil.example/job", "published_date": null, "location": null},
        {"name": "Legit Listing", "url": "https://acme.breezy.hr/p/legit", "published_date": null, "location": null}
    ]"#;
    let postings_in: Vec<BzPosting> = serde_json::from_str(json).unwrap();
    let (postings, format_drops) = parse_breezy_response(postings_in, "acme", 0);
    assert_eq!(
        postings.len(),
        1,
        "userinfo url must be dropped, legit row kept"
    );
    assert_eq!(
        format_drops, 1,
        "the invalid-url drop must count as a format drop"
    );
    assert_eq!(postings[0].title, "Legit Listing");
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = BreezyScraper;
    let input = make_input(vec!["breezy".to_string()]);
    let ctx = make_ctx();
    let results = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        scraper.search(input, ctx),
    )
    .await
    .expect("live search timed out");
    assert!(results.is_ok(), "search failed: {:?}", results.err());
    let postings = results.unwrap();
    assert!(!postings.is_empty(), "expected >=1 posting, got 0");
    let first = &postings[0];
    assert!(!first.title.is_empty(), "first posting has empty title");
    assert!(!first.url.is_empty(), "first posting has empty url");
    println!("breezy: {} results", postings.len());
    println!("first: {:?}", first.title);
}
