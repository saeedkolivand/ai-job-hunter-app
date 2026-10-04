use super::*;

// ===========================================================================
// 7. Photon fallback mapper — canned GeoJSON, no network
// ===========================================================================

#[test]
fn photon_city_feature_maps_to_city_country() {
    let feature = photon_feature(
        json!({
            "type": "city",
            "name": "Berlin",
            "country": "Germany",
            "countrycode": "DE",
            "state": "Berlin"
        }),
        Some([13.3888, 52.5170]),
    );
    let result = to_city_country(&feature).expect("city feature must map");

    assert_eq!(display(&result), "Berlin, Germany");
    assert_eq!(country_code(&result), Some("DE"));
    // GeoJSON order is [lon, lat] — a swap here would put jobs in the wrong
    // hemisphere, so assert both explicitly.
    assert_eq!(lat(&result), Some(52.5170));
    assert_eq!(lon(&result), Some(13.3888));
}

#[test]
fn photon_lowercase_country_code_is_uppercased() {
    let feature = photon_feature(
        json!({ "type": "city", "name": "Paris", "country": "France", "countrycode": "fr" }),
        Some([2.3, 48.9]),
    );
    let result = to_city_country(&feature).expect("city feature must map");
    assert_eq!(country_code(&result), Some("FR"));
}

#[test]
fn photon_sub_city_feature_collapses_to_its_parent_city() {
    // A street/house hit carries the containing city — the picker shows the
    // city, never the street (same reduction the Nominatim path used).
    let feature = photon_feature(
        json!({
            "type": "street",
            "name": "Unter den Linden",
            "city": "Berlin",
            "country": "Germany",
            "countrycode": "DE"
        }),
        Some([13.39, 52.51]),
    );
    let result = to_city_country(&feature).expect("street with a parent city must map");
    assert_eq!(display(&result), "Berlin, Germany");
}

#[test]
fn photon_locality_and_district_use_their_own_name() {
    for feature_type in ["locality", "district"] {
        let feature = photon_feature(
            json!({
                "type": feature_type,
                "name": "Kreuzberg",
                "country": "Germany",
                "countrycode": "DE"
            }),
            Some([13.4, 52.5]),
        );
        let result = to_city_country(&feature)
            .unwrap_or_else(|| panic!("a {feature_type} feature must map"));
        assert_eq!(display(&result), "Kreuzberg, Germany");
    }
}

#[test]
fn photon_country_feature_maps_to_the_country_name() {
    let feature = photon_feature(
        json!({ "type": "country", "name": "Germany", "country": "Germany", "countrycode": "DE" }),
        Some([10.4, 51.1]),
    );
    let result = to_city_country(&feature).expect("country feature must map");
    assert_eq!(display(&result), "Germany");
    assert_eq!(country_code(&result), Some("DE"));
}

#[test]
fn photon_country_feature_without_a_name_falls_back_to_the_code() {
    let feature = photon_feature(json!({ "type": "country", "countrycode": "de" }), None);
    let result = to_city_country(&feature).expect("country code alone is still a usable label");
    assert_eq!(display(&result), "DE");
    assert_eq!(country_code(&result), Some("DE"));
}

#[test]
fn photon_rejects_hits_with_no_city_and_no_country_level() {
    for properties in [
        json!({ "type": "house", "name": "42", "country": "Germany", "countrycode": "DE" }),
        json!({ "type": "street", "name": "Main St", "country": "Germany", "countrycode": "DE" }),
        json!({ "type": "state", "name": "Brandenburg", "country": "Germany", "countrycode": "DE" }),
        json!({ "type": "county", "name": "Kent", "country": "United Kingdom", "countrycode": "GB" }),
        json!({ "type": "other", "name": "Brandenburg Gate", "country": "Germany", "countrycode": "DE" }),
    ] {
        let feature = photon_feature(properties.clone(), Some([13.0, 52.0]));
        assert!(
            to_city_country(&feature).is_none(),
            "must be rejected: {properties}"
        );
    }
}

#[test]
fn photon_city_without_a_country_has_no_trailing_comma() {
    let feature = photon_feature(
        json!({ "type": "city", "name": "Homs" }),
        Some([36.7, 34.7]),
    );
    let result = to_city_country(&feature).expect("city without country must still map");
    assert_eq!(display(&result), "Homs");
    assert!(
        result
            .get("countryCode")
            .map(Value::is_null)
            .unwrap_or(false),
        "countryCode must be an explicit null"
    );
}

#[test]
fn photon_missing_geometry_yields_null_coordinates() {
    let feature = photon_feature(
        json!({ "type": "city", "name": "Oslo", "country": "Norway", "countrycode": "NO" }),
        None,
    );
    let result = to_city_country(&feature).expect("no geometry is not a reason to drop a city");
    assert_eq!(display(&result), "Oslo, Norway");
    assert!(lat(&result).is_none() && lon(&result).is_none());
    assert!(result.get("lat").map(Value::is_null).unwrap_or(false));
    assert!(result.get("lon").map(Value::is_null).unwrap_or(false));
}

// ===========================================================================
// 8. Photon response mapping — dedupe, cap, malformed bodies
// ===========================================================================

fn city(name: &str) -> Value {
    photon_feature(
        json!({ "type": "city", "name": name, "country": "Germany", "countrycode": "DE" }),
        Some([13.0, 52.0]),
    )
}

#[test]
fn photon_response_dedupes_and_caps_at_five() {
    let body = json!({
        "type": "FeatureCollection",
        "features": [
            city("Berlin"), city("Berlin"), city("Hamburg"), city("Munich"),
            city("Cologne"), city("Frankfurt"), city("Stuttgart"), city("Bremen"),
        ]
    });
    let results = photon_suggestions(&body);

    assert_eq!(results.len(), MAX_SUGGESTIONS, "must cap at 5");
    assert_eq!(
        displays(&results),
        vec![
            "Berlin, Germany",
            "Hamburg, Germany",
            "Munich, Germany",
            "Cologne, Germany",
            "Frankfurt, Germany",
        ],
        "duplicates drop out and the upstream order is preserved"
    );
}

#[test]
fn photon_response_skips_unmappable_features_without_dropping_the_rest() {
    let body = json!({
        "features": [
            photon_feature(json!({ "type": "state", "name": "Bavaria", "countrycode": "DE" }), Some([11.0, 48.0])),
            city("Munich"),
        ]
    });
    let results = photon_suggestions(&body);
    assert_eq!(displays(&results), vec!["Munich, Germany"]);
}

#[test]
fn photon_malformed_body_degrades_to_no_suggestions() {
    for body in [
        json!({}),
        json!({ "features": null }),
        json!({ "features": "nope" }),
        json!({ "features": [] }),
        json!({ "message": "rate limited" }),
        Value::Null,
    ] {
        assert!(
            photon_suggestions(&body).is_empty(),
            "malformed body must degrade to empty, not panic: {body}"
        );
    }
}

// ===========================================================================
// 9. The real request path — local mock server, never the live endpoint
// ===========================================================================

#[tokio::test]
async fn photon_request_carries_the_agreed_url_shape_and_user_agent() {
    use wiremock::matchers::{header, path, query_param};

    let server = MockServer::start().await;
    // Every matcher below is part of the assertion: a mismatch makes wiremock
    // answer 404, the body fails to map, and the suggestion list comes back
    // empty — so a green assert proves the URL shape AND the UA header.
    Mock::given(method("GET"))
        .and(path("/api/"))
        .and(query_param("q", "berlin brandenburg"))
        .and(query_param("limit", "10"))
        .and(query_param("lang", "en"))
        .and(header("user-agent", "ai-job-hunter/1.0"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "features": [city("Berlin")] })),
        )
        .mount(&server)
        .await;

    let results = photon_at(
        &format!("{}/api/", server.uri()),
        "berlin brandenburg",
        Duration::from_secs(5),
    )
    .await;

    assert_eq!(displays(&results), vec!["Berlin, Germany"]);
}

#[tokio::test]
async fn photon_timeout_degrades_to_no_suggestions() {
    let (_server, endpoint) = photon_mock(
        ResponseTemplate::new(200)
            .set_delay(Duration::from_millis(600))
            .set_body_json(json!({ "features": [city("Berlin")] })),
    )
    .await;

    // Same server, same delay — only the timeout differs. The generous run
    // proves the response itself is fine, so the tight run's empty result can
    // only be the timeout being applied to the request.
    let patient = photon_at(&endpoint, "berlin", Duration::from_secs(5)).await;
    assert_eq!(displays(&patient), vec!["Berlin, Germany"]);

    let impatient = photon_at(&endpoint, "berlin", Duration::from_millis(50)).await;
    assert!(
        impatient.is_empty(),
        "a timed-out lookup degrades to no suggestions, got {:?}",
        displays(&impatient)
    );
}

#[tokio::test]
async fn photon_non_json_body_degrades_to_no_suggestions() {
    let (_server, endpoint) =
        photon_mock(ResponseTemplate::new(503).set_body_string("<html>rate limited</html>")).await;

    let results = photon_at(&endpoint, "berlin", Duration::from_secs(5)).await;
    assert!(results.is_empty(), "a 503 HTML page must not panic or leak");
}

#[test]
fn dedupe_keeps_first_of_each_label_and_stops_at_the_limit() {
    let items = vec![
        json!({ "display": "Berlin, Germany", "lat": 1.0 }),
        json!({ "display": "Berlin, Germany", "lat": 2.0 }),
        json!({ "display": "Bern, Switzerland" }),
        // No display → cannot be labelled, so it is dropped, not counted.
        json!({ "lat": 3.0 }),
        json!({ "display": "Bremen, Germany" }),
    ];
    let results = dedupe_by_display(items.into_iter(), 2);

    assert_eq!(
        displays(&results),
        vec!["Berlin, Germany", "Bern, Switzerland"]
    );
    assert_eq!(
        lat(&results[0]),
        Some(1.0),
        "the FIRST occurrence of a label must survive"
    );
}
