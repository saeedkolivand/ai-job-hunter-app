use super::super::*;
use super::metadata_and_search::{make_ctx, make_input};

// ---------------------------------------------------------------------------
// parse_bamboohr_response — fixture-based parsing
// ---------------------------------------------------------------------------

#[test]
fn parse_bamboohr_response_happy_path() {
    let json = r#"{
        "result": [
            {
                "id": 1,
                "jobOpeningName": "DevOps Engineer",
                "location": { "city": "Austin", "state": "TX" },
                "isRemote": false
            }
        ]
    }"#;
    let resp: BhResponse = serde_json::from_str(json).expect("fixture must parse");
    let postings = parse_bamboohr_response(resp, "acme", 1_700_000_000_000);

    assert_eq!(postings.len(), 1);
    let p = &postings[0];
    assert_eq!(p.title, "DevOps Engineer");
    assert_eq!(p.company, "acme");
    assert_eq!(p.location, Some("Austin, TX".to_string()));
    assert_eq!(p.url, "https://acme.bamboohr.com/careers/1");
    assert_eq!(p.id, "bamboohr:acme:1");
    assert_eq!(p.external_id, Some("1".to_string()));
    assert_eq!(p.source, "bamboohr");
    assert_eq!(p.captured_at, 1_700_000_000_000);
}

#[test]
fn parse_bamboohr_response_empty_result_returns_empty_vec() {
    let resp: BhResponse = serde_json::from_str(r#"{"result": []}"#).unwrap();
    assert!(
        parse_bamboohr_response(resp, "acme", 0).is_empty(),
        "empty result array must parse to an empty Vec, not an error"
    );
}

/// Missing/blank id and missing/empty title each drop the row; valid rows in
/// the same payload must still come through.
#[test]
fn parse_bamboohr_response_drops_malformed_rows() {
    let json = r#"{
        "result": [
            {"id": 1, "jobOpeningName": "Valid One", "location": null, "isRemote": null},
            {"id": null, "jobOpeningName": "Missing ID", "location": null, "isRemote": null},
            {"id": "", "jobOpeningName": "Blank ID", "location": null, "isRemote": null},
            {"id": 2, "jobOpeningName": null, "location": null, "isRemote": null},
            {"id": 3, "jobOpeningName": "", "location": null, "isRemote": null},
            {"id": 4, "jobOpeningName": "Valid Two", "location": null, "isRemote": null}
        ]
    }"#;
    let resp: BhResponse = serde_json::from_str(json).unwrap();
    let postings = parse_bamboohr_response(resp, "acme", 0);
    let titles: Vec<&str> = postings.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        vec!["Valid One", "Valid Two"],
        "malformed rows must be dropped without panicking, valid rows kept: {titles:?}"
    );
}

/// `isRemote: true` appends "Remote" to the joined location string.
#[test]
fn parse_bamboohr_response_is_remote_true_appends_remote_to_location() {
    let json = r#"{
        "result": [
            {
                "id": 1,
                "jobOpeningName": "Support Engineer",
                "location": { "city": "Austin", "state": "TX" },
                "isRemote": true
            }
        ]
    }"#;
    let resp: BhResponse = serde_json::from_str(json).unwrap();
    let postings = parse_bamboohr_response(resp, "acme", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(postings[0].location, Some("Austin, TX, Remote".to_string()));
}

/// `isRemote: null` must not add "Remote" — the happy-path test above already
/// covers the explicit `false` case.
#[test]
fn parse_bamboohr_response_is_remote_null_omits_remote() {
    let json = r#"{
        "result": [
            {
                "id": 1,
                "jobOpeningName": "Support Engineer",
                "location": { "city": "Austin", "state": "TX" },
                "isRemote": null
            }
        ]
    }"#;
    let resp: BhResponse = serde_json::from_str(json).unwrap();
    let postings = parse_bamboohr_response(resp, "acme", 0);
    assert_eq!(postings.len(), 1);
    assert_eq!(
        postings[0].location,
        Some("Austin, TX".to_string()),
        "null isRemote must not add Remote"
    );
}

/// Data-shape unknown: `id` observed as both a JSON number and a JSON string
/// across tenants — both must be accepted and normalise to the same id.
#[test]
fn parse_bamboohr_response_id_number_and_string_forms_both_accepted() {
    let json_number = r#"{"result":[{"id":1,"jobOpeningName":"DevOps Engineer","location":null,"isRemote":null}]}"#;
    let json_string = r#"{"result":[{"id":"1","jobOpeningName":"DevOps Engineer","location":null,"isRemote":null}]}"#;

    let resp_number: BhResponse = serde_json::from_str(json_number).unwrap();
    let resp_string: BhResponse = serde_json::from_str(json_string).unwrap();

    let out_number = parse_bamboohr_response(resp_number, "acme", 0);
    let out_string = parse_bamboohr_response(resp_string, "acme", 0);

    assert_eq!(out_number.len(), 1);
    assert_eq!(out_string.len(), 1);
    assert_eq!(out_number[0].id, "bamboohr:acme:1");
    assert_eq!(out_string[0].id, "bamboohr:acme:1");
    assert_eq!(out_number[0].external_id, Some("1".to_string()));
    assert_eq!(out_string[0].external_id, Some("1".to_string()));
}

/// Regression (HIGH fix): the same raw job id from two different tenants must
/// produce two distinct `JobPosting.id` values (`bamboohr:acme:1` vs
/// `bamboohr:globex:1`) — neither must overwrite the other in a dedup layer.
#[test]
fn parse_bamboohr_response_cross_tenant_ids_do_not_collide() {
    let json =
        r#"{"result":[{"id":1,"jobOpeningName":"Engineer","location":null,"isRemote":null}]}"#;

    let resp_acme: BhResponse = serde_json::from_str(json).unwrap();
    let resp_globex: BhResponse = serde_json::from_str(json).unwrap();

    let out_acme = parse_bamboohr_response(resp_acme, "acme", 0);
    let out_globex = parse_bamboohr_response(resp_globex, "globex", 0);

    assert_eq!(out_acme.len(), 1);
    assert_eq!(out_globex.len(), 1);
    assert_ne!(
        out_acme[0].id, out_globex[0].id,
        "same raw job id=1 from different tenants must produce distinct JobPosting.id"
    );
    assert_eq!(out_acme[0].id, "bamboohr:acme:1");
    assert_eq!(out_globex[0].id, "bamboohr:globex:1");
}

// ---------------------------------------------------------------------------
// bamboohr_location_type_to_work_type — the inferred "0"/"1"/"2" mapping
// ---------------------------------------------------------------------------

#[test]
fn location_type_maps_the_three_known_codes() {
    assert_eq!(
        bamboohr_location_type_to_work_type("0"),
        Some(WorkType::OnSite)
    );
    assert_eq!(
        bamboohr_location_type_to_work_type("1"),
        Some(WorkType::Remote)
    );
    assert_eq!(
        bamboohr_location_type_to_work_type("2"),
        Some(WorkType::Hybrid)
    );
}

#[test]
fn location_type_unrecognised_code_is_none_not_a_default() {
    assert_eq!(bamboohr_location_type_to_work_type("3"), None);
    assert_eq!(bamboohr_location_type_to_work_type(""), None);
}

/// End-to-end through `parse_bamboohr_response`: `locationType` maps into
/// `extra.workType`; an absent field writes nothing.
#[test]
fn parse_bamboohr_response_location_type_maps_to_extra_work_type() {
    let json = r#"{
        "result": [
            {"id": 1, "jobOpeningName": "Remote Role", "location": null, "isRemote": null, "locationType": "1"},
            {"id": 2, "jobOpeningName": "Hybrid Role", "location": null, "isRemote": null, "locationType": "2"},
            {"id": 3, "jobOpeningName": "Onsite Role", "location": null, "isRemote": null, "locationType": "0"},
            {"id": 4, "jobOpeningName": "Undeclared Role", "location": null, "isRemote": null}
        ]
    }"#;
    let resp: BhResponse = serde_json::from_str(json).unwrap();
    let postings = parse_bamboohr_response(resp, "acme", 0);

    let work_type = |external_id: &str| -> Option<String> {
        postings
            .iter()
            .find(|p| p.external_id.as_deref() == Some(external_id))
            .and_then(|p| p.extra.get("workType"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    assert_eq!(work_type("1"), Some("remote".to_string()));
    assert_eq!(work_type("2"), Some("hybrid".to_string()));
    assert_eq!(work_type("3"), Some("on-site".to_string()));
    assert_eq!(
        work_type("4"),
        None,
        "an absent locationType must write nothing, not a guessed value"
    );
}

#[tokio::test]
#[ignore = "live network"]
async fn live_search_returns_results() {
    let scraper = BambooHrScraper;
    let input = make_input(vec!["bamboohr".to_string()]);
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
    println!("bamboohr: {} results", postings.len());
    println!("first: {:?}", first.title);
}
