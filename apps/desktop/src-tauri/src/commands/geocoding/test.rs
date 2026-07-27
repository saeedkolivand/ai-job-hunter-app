//! Unit tests for the offline GeoNames index and the Photon fallback mapper.
//!
//! Everything here is hermetic: the index tests run against the **real bundled
//! asset** (that is the point — a silently-empty or mis-parsed asset must fail
//! the build, not degrade the picker), the fallback mapper is fed canned Photon
//! GeoJSON, and the two request-shape tests drive a local `wiremock` server.
//! No test reaches the live network.

use std::time::Duration;

use serde_json::{json, Value};

use super::{
    before_comma, dedupe_by_display, geonames, photon_at, photon_suggestions, should_try_online,
    suggest, to_city_country, MAX_ONLINE_QUERY_BYTES, MAX_SUGGESTIONS,
};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn display(v: &Value) -> &str {
    v.get("display")
        .and_then(|d| d.as_str())
        .expect("display field missing")
}

fn country_code(v: &Value) -> Option<&str> {
    v.get("countryCode").and_then(|c| c.as_str())
}

fn lat(v: &Value) -> Option<f64> {
    v.get("lat").and_then(|l| l.as_f64())
}

fn lon(v: &Value) -> Option<f64> {
    v.get("lon").and_then(|l| l.as_f64())
}

fn displays(results: &[Value]) -> Vec<&str> {
    results.iter().map(display).collect()
}

fn search(query: &str) -> Vec<Value> {
    geonames::search(query, MAX_SUGGESTIONS).suggestions
}

/// Offline lookup including the match-quality flag the online fallback gates on.
fn search_hits(query: &str) -> geonames::Hits {
    geonames::search(query, MAX_SUGGESTIONS)
}

/// The country code of the first suggestion — what `commands::autopilot`'s
/// `country_code_from_suggestions` actually persists.
fn first_country_code(results: &[Value]) -> Option<&str> {
    results.first().and_then(country_code)
}

// ===========================================================================
// 1. The bundled asset actually parses
// ===========================================================================

#[test]
fn bundled_asset_parses_into_a_populated_index() {
    let (cities, countries) = geonames::row_counts();
    // cities15000 carries ~34k rows and countryInfo ~250. Assert generous
    // floors: this fails loudly if the gz asset is corrupt/empty or the column
    // layout drifted, instead of silently returning zero suggestions forever.
    assert!(
        cities > 20_000,
        "bundled city index parsed only {cities} rows — asset corrupt or column layout changed"
    );
    assert!(
        countries >= 200,
        "bundled country index parsed only {countries} rows"
    );
}

// ===========================================================================
// 2. Ranking
// ===========================================================================

#[test]
fn prefix_hits_rank_by_population() {
    // "ber" prefixes Berlin (3.4M), Bergen (294k), Berbera (242k)… — the
    // biggest must lead. It must ALSO beat the country Bermuda, which prefixes
    // "ber" too: country *prefixes* share the tier and lose on population.
    let results = search("ber");
    assert_eq!(
        display(&results[0]),
        "Berlin, Germany",
        "biggest prefix match must lead, got {:?}",
        displays(&results)
    );
    assert!(
        !displays(&results).contains(&"Bermuda"),
        "a small country prefix must not displace big cities: {:?}",
        displays(&results)
    );
}

#[test]
fn exact_name_outranks_a_longer_prefix_match() {
    // Berlin-Köpenick / Berlin Hohenschönhausen also start with "berlin"; the
    // exact-name tier keeps the city the user obviously meant on top.
    let results = search("berlin");
    assert_eq!(display(&results[0]), "Berlin, Germany");
    assert_eq!(country_code(&results[0]), Some("DE"));
}

#[test]
fn exact_name_collision_ranks_the_bigger_city_first() {
    // Two real "York"s (GB 156k, US 44k) — same tier, population decides.
    let hits = search("york");
    let results = displays(&hits);
    let gb = results.iter().position(|d| *d == "York, United Kingdom");
    let us = results.iter().position(|d| *d == "York, United States");
    assert!(
        gb.is_some() && us.is_some(),
        "both Yorks expected: {results:?}"
    );
    assert!(gb < us, "the larger York must rank first: {results:?}");
}

// ===========================================================================
// 3. Alternate names + diacritic folding
// ===========================================================================

#[test]
fn german_endonym_and_its_ascii_digraph_both_find_the_city() {
    // GeoNames' primary name is "Munich"; "München" is an alternate. Both the
    // umlaut spelling and its ASCII digraph must fold onto it (shared `fold`).
    for query in ["münchen", "muenchen", "MÜNCHEN", " München "] {
        let results = search(query);
        assert_eq!(
            display(&results[0]),
            "Munich, Germany",
            "query {query:?} must resolve to Munich, got {:?}",
            displays(&results)
        );
        assert_eq!(country_code(&results[0]), Some("DE"));
    }
}

#[test]
fn diacritic_primary_name_matches_its_ascii_form() {
    // Reverse direction: the primary name carries the umlaut ("Köln"), the
    // asciiname is the digraph form ("Koeln"). Either spelling must hit.
    for query in ["köln", "koeln", "Köln"] {
        let results = search(query);
        assert_eq!(
            display(&results[0]),
            "Köln, Germany",
            "query {query:?} must resolve to Köln, got {:?}",
            displays(&results)
        );
    }
}

#[test]
fn exact_alternate_name_beats_a_bigger_city_prefix() {
    // "wien" is exactly Vienna's German alternate; "Wiener Neustadt" merely
    // starts with it. Both land in the prefix tier, so Vienna's population wins.
    let results = search("wien");
    assert_eq!(
        display(&results[0]),
        "Vienna, Austria",
        "got {:?}",
        displays(&results)
    );
}

// ===========================================================================
// 4. Country-level queries
// ===========================================================================

#[test]
fn country_name_query_returns_a_country_level_suggestion() {
    let results = search("germany");
    let first = &results[0];
    assert_eq!(display(first), "Germany");
    assert_eq!(country_code(first), Some("DE"));
    // countryInfo carries no coordinates — lat/lon must be explicit JSON nulls
    // (a missing key would break the `{display,lat,lon,countryCode}` contract).
    assert!(
        first.get("lat").map(Value::is_null).unwrap_or(false),
        "country lat must be an explicit null"
    );
    assert!(
        first.get("lon").map(Value::is_null).unwrap_or(false),
        "country lon must be an explicit null"
    );
}

// ---------------------------------------------------------------------------
// Country CODES and endonyms — the prefix-accident regressions.
//
// Before ISO codes and the alias table, each of these prefix-matched an
// unrelated city with five confident-looking rows: `usa`/`us` → Uşak, TR ·
// `uk` → Ukraine · `de` → DR Congo · `gb` → Gboko, NG · `schweiz` →
// Schweizer-Reneke, ZA. `commands::autopilot` persists suggestion[0]'s country
// code, so "Schweiz" used to save `za` — a live Adzuna market, i.e. a silent
// switch to South African jobs.
// ---------------------------------------------------------------------------

#[test]
fn iso2_and_iso3_country_codes_resolve_to_their_country() {
    for (query, expected) in [
        ("us", "US"),
        ("usa", "US"),
        ("de", "DE"),
        ("deu", "DE"),
        ("gb", "GB"),
        ("gbr", "GB"),
        ("ch", "CH"),
        ("che", "CH"),
    ] {
        let results = search(query);
        assert_eq!(
            first_country_code(&results),
            Some(expected),
            "query {query:?} must resolve to {expected}, got {:?}",
            displays(&results)
        );
        assert!(
            search_hits(query).exact,
            "a typed country code is an exact hit, not a guess ({query})"
        );
    }
}

#[test]
fn country_codes_are_matched_exactly_never_as_a_prefix() {
    // If codes were prefix-matched, every one-letter query would resolve to a
    // pile of countries and "u" would outrank every city on earth.
    let results = search("u");
    assert!(
        !results.is_empty(),
        "a one-letter query still returns city prefixes"
    );
    assert!(
        !search_hits("u").exact,
        "one letter must never be an exact country-code hit"
    );
}

#[test]
fn curated_endonyms_resolve_to_the_right_country() {
    for (query, expected) in [
        ("deutschland", "DE"),
        ("österreich", "AT"),
        ("oesterreich", "AT"),
        ("schweiz", "CH"),
        ("suisse", "CH"),
        ("svizzera", "CH"),
        ("frankreich", "FR"),
        ("uk", "GB"),
        ("españa", "ES"),
        ("espana", "ES"),
        ("nederland", "NL"),
        ("polska", "PL"),
    ] {
        let results = search(query);
        assert_eq!(
            first_country_code(&results),
            Some(expected),
            "endonym {query:?} must resolve to {expected}, got {:?}",
            displays(&results)
        );
    }
}

#[test]
fn schweiz_never_persists_south_africa() {
    // The exact failure the alias table + quality gate exist to prevent.
    let results = search("schweiz");
    assert_ne!(
        first_country_code(&results),
        Some("ZA"),
        "Schweizer-Reneke, ZA must never be the answer for 'Schweiz': {:?}",
        displays(&results)
    );
    assert_eq!(first_country_code(&results), Some("CH"));
}

#[test]
fn an_unlisted_endonym_is_reported_as_inexact_so_photon_can_answer() {
    // The alias table is deliberately tiny; anything outside it must NOT be
    // guessed from a prefix accident — it must be flagged inexact so
    // `suggest` consults Photon instead of persisting a wrong country.
    for query in ["belgique", "sverige", "danmark"] {
        assert!(
            !search_hits(query).exact,
            "{query} is not an index key — it must not claim an exact match"
        );
        assert!(
            should_try_online(query, !search(query).is_empty()),
            "{query} must be allowed to reach Photon"
        );
    }
}

#[test]
fn alias_keys_are_stored_pre_folded() {
    // Matching happens on folded queries, so an alias written with an umlaut
    // ("österreich") would silently never match. Pin the invariant.
    for (alias, cc) in geonames::aliases() {
        assert_eq!(
            &crate::scraping::cluster::normalize::fold(alias),
            alias,
            "alias {alias:?} (for {cc}) must already be in folded form"
        );
        assert_eq!(cc.len(), 2, "alias target must be an ISO-2 code");
    }
}

#[test]
fn full_country_name_outranks_every_city() {
    // "Luxembourg" is both a country and a city; the exact-country tier wins.
    let results = search("luxembourg");
    assert_eq!(
        display(&results[0]),
        "Luxembourg",
        "an exactly-spelled country must lead, got {:?}",
        displays(&results)
    );
    assert_eq!(country_code(&results[0]), Some("LU"));
}

// ===========================================================================
// 5. Shape parity with the pre-existing contract
// ===========================================================================

#[test]
fn results_keep_the_suggestion_contract() {
    for query in ["a", "san", "new", "united", "berlin"] {
        let results = search(query);
        assert!(
            results.len() <= MAX_SUGGESTIONS,
            "query {query:?} returned {} suggestions",
            results.len()
        );
        let mut labels = Vec::new();
        for suggestion in &results {
            for key in ["display", "lat", "lon", "countryCode"] {
                assert!(
                    suggestion.get(key).is_some(),
                    "query {query:?}: missing key {key} in {suggestion}"
                );
            }
            let cc = country_code(suggestion).expect("countryCode must be a string");
            assert!(
                cc.len() == 2 && cc.chars().all(|c| c.is_ascii_uppercase()),
                "countryCode must be uppercase ISO-2, got {cc:?}"
            );
            labels.push(display(suggestion));
        }
        let mut deduped = labels.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(
            deduped.len(),
            labels.len(),
            "query {query:?} returned duplicate labels: {labels:?}"
        );
    }
}

#[test]
fn no_match_returns_nothing() {
    assert!(search("zx").is_empty(), "'zx' matches no bundled place");
    assert!(search("   ").is_empty(), "whitespace-only query");
    assert!(search("").is_empty(), "empty query");
}

// ===========================================================================
// 6. suggest() orchestration — offline first, network only on a real miss
// ===========================================================================

#[tokio::test]
async fn suggest_answers_from_the_offline_index() {
    // An exact offline hit must be returned verbatim — byte-equal to what the
    // index produced, which is only possible if the online branch never ran.
    let results = suggest("berlin").await;
    assert_eq!(display(&results[0]), "Berlin, Germany");
    assert!(results.len() <= MAX_SUGGESTIONS);
    assert_eq!(
        results,
        search("berlin"),
        "an exact offline hit must be served as-is, with no network round trip"
    );
}

#[tokio::test]
async fn suggest_reresolves_the_label_it_wrote_back_offline() {
    // The picker writes "City, Country" into the field, so the NEXT open queries
    // that whole string — which matches no single index key. Without the
    // before-comma retry every re-open would be a Photon request.
    for query in ["Berlin, Germany", "Munich, Germany", "Vienna, Austria"] {
        let hits = search_hits(query);
        assert!(
            !hits.exact,
            "precondition: the full label is not itself an index key ({query})"
        );

        let results = suggest(query).await;
        assert_eq!(
            display(&results[0]),
            query,
            "re-opening the picker must resolve its own label offline"
        );
    }
}

#[test]
fn before_comma_takes_the_city_half_only_when_there_is_one() {
    assert_eq!(before_comma("Berlin, Germany"), Some("Berlin"));
    assert_eq!(before_comma("Berlin,Germany"), Some("Berlin"));
    assert_eq!(before_comma("Berlin"), None, "no comma → nothing to retry");
    assert_eq!(before_comma(", Germany"), None, "empty head → nothing");
}

#[tokio::test]
async fn suggest_empty_query_never_looks_anything_up() {
    assert!(suggest("").await.is_empty());
    assert!(suggest("   ").await.is_empty());
}

#[test]
fn short_queries_never_reach_the_network() {
    // Fair use: an offline miss on a half-typed word must not fire a request at
    // the free community endpoint. `weak = false` is the zero-hit case.
    assert!(!should_try_online("b", false));
    assert!(!should_try_online("zx", false));
    // Counted in chars, not bytes — "köl" is 3 chars but 4 bytes.
    assert!(should_try_online("köl", false));
    assert!(should_try_online("berlin", false));
}

#[test]
fn an_inexact_hit_raises_the_online_floor_instead_of_vetoing_it() {
    // weak = the index returned rows but matched nothing exactly. Mid-typing
    // ("ber" → Berlin) must stay offline; a settled word that only prefix-
    // matched ("schweiz" → Schweizer-Reneke) must be allowed to ask Photon.
    assert!(!should_try_online("ber", true), "mid-typing stays offline");
    assert!(
        !should_try_online("berli", true),
        "mid-typing stays offline"
    );
    assert!(should_try_online("schweiz", true));
    assert!(should_try_online("amsterda", true));
}

#[test]
fn an_absurdly_long_query_is_never_sent_to_a_third_party() {
    let long = "a".repeat(MAX_ONLINE_QUERY_BYTES + 1);
    assert!(!should_try_online(&long, false));
    assert!(!should_try_online(&long, true));
    assert!(
        should_try_online(&"a".repeat(MAX_ONLINE_QUERY_BYTES), false),
        "the limit itself is still allowed"
    );
}

// ===========================================================================
// 7. Photon fallback mapper — canned GeoJSON, no network
// ===========================================================================

fn photon_feature(properties: Value, coordinates: Option<[f64; 2]>) -> Value {
    match coordinates {
        Some([lon, lat]) => json!({
            "type": "Feature",
            "geometry": { "type": "Point", "coordinates": [lon, lat] },
            "properties": properties,
        }),
        None => json!({ "type": "Feature", "properties": properties }),
    }
}

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
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(600))
                .set_body_json(json!({ "features": [city("Berlin")] })),
        )
        .mount(&server)
        .await;
    let endpoint = format!("{}/api/", server.uri());

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
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_body_string("<html>rate limited</html>"))
        .mount(&server)
        .await;

    let results = photon_at(
        &format!("{}/api/", server.uri()),
        "berlin",
        Duration::from_secs(5),
    )
    .await;
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
