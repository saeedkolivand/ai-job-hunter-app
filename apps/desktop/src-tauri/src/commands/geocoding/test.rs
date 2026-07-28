//! Unit tests for the offline GeoNames index, the Photon fallback mapper, and
//! the server-side `country_code` backfill helpers
//! (`should_derive_country_code`, `country_code_from_suggestions`,
//! `derive_country_code`) — the backfill helpers live here, next to `suggest`,
//! because the manual scrape path and the autopilot save path share them.
//!
//! Everything here is hermetic: the index tests run against the **real bundled
//! asset** (that is the point — a silently-empty or mis-parsed asset must fail
//! the build, not degrade the picker), the fallback mapper is fed canned Photon
//! GeoJSON, and the request-shape tests drive a local `wiremock` server.
//! No test reaches the live network.
//!
//! This child module can reach the private parent helpers via `super::…`.

use std::time::Duration;

use serde_json::{json, Value};

use super::{
    before_comma, country_code_from_suggestions, dedupe_by_display, derive_country_code, geonames,
    is_indexable_script, photon_at, photon_suggestions, should_derive_country_code,
    should_try_online, suggest, suggest_at, to_city_country, MAX_ONLINE_QUERY_BYTES,
    MAX_SUGGESTIONS,
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
fn every_alias_is_pre_folded_and_actually_resolves() {
    // Two invariants over the SAME table, so neither can drift from a
    // hand-written expectation list elsewhere in this file:
    //   1. keys are stored folded — matching happens on folded queries, so an
    //      alias written with an umlaut ("österreich") would never match;
    //   2. every entry really does resolve to its target country through the
    //      full search path (not just exist in the table).
    for (alias, cc) in geonames::aliases() {
        assert_eq!(
            &crate::scraping::cluster::normalize::fold(alias),
            alias,
            "alias {alias:?} (for {cc}) must already be in folded form"
        );
        assert!(
            cc.len() == 2 && cc.chars().all(|c| c.is_ascii_uppercase()),
            "alias target {cc:?} must be an upper-case ISO-2 code"
        );
        let results = search(alias);
        assert_eq!(
            first_country_code(&results),
            Some(*cc),
            "alias {alias:?} must resolve to {cc}, got {:?}",
            displays(&results)
        );
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
    // Structural no-live-egress guarantee: assert the query IS an exact index
    // hit, which is what forces the early return. Asserting only on the result
    // would silently start making live requests if the asset ever drifted.
    assert!(
        search_hits("berlin").exact,
        "'berlin' must be an exact index hit — otherwise this test reaches the network"
    );
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
        // Structural no-live-egress guarantee: the retry head must itself be an
        // exact hit, which is what makes `suggest` return before Photon. If the
        // asset ever drifted so it wasn't, this test would quietly start making
        // live requests instead of failing.
        let head = before_comma(query).expect("the label has a comma");
        assert!(
            search_hits(head).exact,
            "the before-comma head {head:?} must be an exact index hit"
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
    assert!(should_try_online("rotterda", true));
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

// ---------------------------------------------------------------------------
// country_code_from_suggestions / should_derive_country_code
//
// The server-side `country_code` backfill for a location that never went
// through the picker. Shared by the autopilot save path and the manual scrape
// path, so these live here (next to `suggest`) rather than in one command
// module.
// ---------------------------------------------------------------------------

#[test]
fn should_derive_country_code_requires_a_real_location() {
    assert!(should_derive_country_code(Some("London")));
    // Whitespace-only or absent location → nothing to geocode.
    assert!(!should_derive_country_code(Some("   ")));
    assert!(!should_derive_country_code(Some("")));
    assert!(!should_derive_country_code(None));
}

#[test]
fn country_code_from_suggestions_takes_first_hit_lowercased() {
    let suggestions = vec![
        json!({ "display": "London, United Kingdom", "countryCode": "GB" }),
        json!({ "display": "London, Canada", "countryCode": "CA" }),
    ];
    assert_eq!(
        country_code_from_suggestions(&suggestions),
        Some("gb".to_string()),
        "must take the first (best-ranked) suggestion and lower-case it to \
         match BoardSearchInput::country_code's convention"
    );
}

#[test]
fn country_code_from_suggestions_empty_or_missing_field_yields_none() {
    assert_eq!(country_code_from_suggestions(&[]), None);
    // A suggestion missing `countryCode` entirely (e.g. an ambiguous hit).
    let no_country = vec![json!({ "display": "Atlantis" })];
    assert_eq!(country_code_from_suggestions(&no_country), None);
}

#[test]
fn country_code_from_suggestions_skips_a_leading_hit_with_no_country_code() {
    // The first (best-ranked) hit has no countryCode (absent AND explicit
    // null) — must not block a usable later suggestion.
    let absent_then_present = vec![
        json!({ "display": "Ambiguous place" }),
        json!({ "display": "Munich, Germany", "countryCode": "DE" }),
    ];
    assert_eq!(
        country_code_from_suggestions(&absent_then_present),
        Some("de".to_string()),
        "an absent countryCode on the first hit must not block a later, \
         usable suggestion"
    );

    let null_then_present = vec![
        json!({ "display": "Ambiguous place", "countryCode": null }),
        json!({ "display": "Munich, Germany", "countryCode": "DE" }),
    ];
    assert_eq!(
        country_code_from_suggestions(&null_then_present),
        Some("de".to_string()),
        "an explicit null countryCode on the first hit must not block a \
         later, usable suggestion"
    );
}

#[test]
fn country_code_from_suggestions_skips_malformed_country_codes() {
    // A geocoded value written server-side bypasses the IPC schema's
    // `^[A-Za-z]{2}$` guard, so a leading 3-letter ("USA") or non-alpha
    // ("1a") countryCode must be SKIPPED in favour of a later valid hit —
    // not accepted (it would fail BoardSearchInput's 2-letter contract).
    let malformed_then_present = vec![
        json!({ "display": "United States", "countryCode": "USA" }),
        json!({ "display": "Nowhere", "countryCode": "1a" }),
        json!({ "display": "Munich, Germany", "countryCode": "DE" }),
    ];
    assert_eq!(
        country_code_from_suggestions(&malformed_then_present),
        Some("de".to_string()),
        "malformed leading countryCodes (3-letter, non-alpha) must be \
         skipped in favour of a valid 2-letter hit"
    );

    // All candidates malformed → None (nothing usable to backfill).
    let all_malformed = vec![
        json!({ "display": "United States", "countryCode": "USA" }),
        json!({ "display": "Nowhere", "countryCode": "1a" }),
        json!({ "display": "Solo", "countryCode": "G" }),
    ];
    assert_eq!(
        country_code_from_suggestions(&all_malformed),
        None,
        "an all-malformed list yields no country code"
    );
}

#[tokio::test]
async fn derive_country_code_skips_geocode_when_location_absent() {
    // No location at all → must resolve to None WITHOUT attempting a
    // network call (a real HTTP hit here would make this test flaky/slow).
    assert_eq!(derive_country_code(None).await, None);
    assert_eq!(derive_country_code(Some("")).await, None);
    assert_eq!(derive_country_code(Some("   ")).await, None);
}

// ===========================================================================
// 10. suggest()'s merge rule — the offline/Photon seam, against a mock server
//
// Every OTHER suggest() test returns at the offline early-return, so without
// these the fallback branch and the merge rule are asserted nowhere. Each test
// uses a DISTINCT weak query so the memo cache can't couple them.
// ===========================================================================

/// A query with offline rows but no exact hit, long enough to clear the
/// weak-hit floor — the only shape that reaches the fallback.
fn assert_is_weak_and_online_eligible(query: &str) {
    let hits = search_hits(query);
    assert!(
        !hits.suggestions.is_empty() && !hits.exact,
        "precondition: {query:?} must be a non-exact offline hit"
    );
    assert!(
        should_try_online(query, true),
        "precondition: {query:?} must clear the weak-hit online floor"
    );
}

#[tokio::test]
async fn photon_replaces_a_weak_offline_hit() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    assert_is_weak_and_online_eligible("rotterda");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "features": [photon_feature(
                json!({ "type": "city", "name": "Rotterdam", "country": "Netherlands", "countrycode": "NL" }),
                Some([4.47, 51.92]),
            )]
        })))
        .mount(&server)
        .await;

    let results = suggest_at(&format!("{}/api/", server.uri()), "rotterda").await;

    assert_eq!(
        displays(&results),
        vec!["Rotterdam, Netherlands"],
        "a real geocoder's answer must REPLACE the prefix accident, not append to it"
    );
    assert_eq!(first_country_code(&results), Some("NL"));
}

#[tokio::test]
async fn an_empty_photon_answer_leaves_the_offline_rows_alone() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    assert_is_weak_and_online_eligible("barcelo");
    let offline = search("barcelo");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "features": [] })))
        .mount(&server)
        .await;

    let results = suggest_at(&format!("{}/api/", server.uri()), "barcelo").await;

    assert_eq!(
        results, offline,
        "Photon finding nothing must never destroy the rows we already had"
    );
    assert!(!results.is_empty());
}

#[tokio::test]
async fn a_failed_photon_lookup_leaves_the_offline_rows_alone() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    assert_is_weak_and_online_eligible("stockhol");
    let offline = search("stockhol");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503).set_body_string("rate limited"))
        .mount(&server)
        .await;

    let results = suggest_at(&format!("{}/api/", server.uri()), "stockhol").await;

    assert_eq!(
        results, offline,
        "a 5xx must degrade to the offline rows, not to nothing"
    );
}

#[tokio::test]
async fn repeated_lookups_are_served_from_the_memo_cache() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    assert_is_weak_and_online_eligible("copenhag");

    let server = MockServer::start().await;
    // `expect(1)` is the assertion: MockServer verifies it on drop, so a second
    // outbound request (i.e. a cache miss) fails the test. The picker re-issues
    // the same query on every dropdown re-open, so this is what keeps a weak
    // hit from re-asking a free community endpoint on every tick.
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "features": [photon_feature(
                json!({ "type": "city", "name": "Copenhagen", "country": "Denmark", "countrycode": "DK" }),
                Some([12.57, 55.68]),
            )]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let endpoint = format!("{}/api/", server.uri());
    let first = suggest_at(&endpoint, "copenhag").await;
    let second = suggest_at(&endpoint, "copenhag").await;

    assert_eq!(displays(&first), vec!["Copenhagen, Denmark"]);
    assert_eq!(second, first, "the memoized answer must be identical");
}

#[tokio::test]
async fn derive_country_code_resolves_typed_locations_from_the_offline_index() {
    // The backfill now rides on the offline index, so the freehand locations
    // that motivated it resolve with no network at all — including the label
    // the picker writes back, and a typed country code. Anything here reaching
    // the network would blow the 2s BACKFILL_GEOCODE_TIMEOUT in CI and fail.
    for (location, expected) in [
        ("Germany", "de"),
        ("Amsterdam", "nl"),
        ("Berlin, Germany", "de"),
        ("München", "de"),
        ("usa", "us"),
        ("Schweiz", "ch"),
    ] {
        assert_eq!(
            derive_country_code(Some(location)).await,
            Some(expected.to_string()),
            "{location:?} must backfill {expected:?} offline"
        );
    }
}

// ===========================================================================
// 11. Index construction — the degradation paths
//
// Every arm below is a `continue`/fallback that keeps a partially-broken asset
// usable instead of panicking the app. They are only reachable through
// `parse`/`decode_cities` with hand-built input, which is why those seams exist.
// ===========================================================================

/// `name · asciiname · cc · lat · lon · population · alternates`
const GOOD_CITY: &str = "Berlin\t\tDE\t52.5244\t13.4105\t3426354\tBerlino";
/// `cc · ISO3 · name · population`
const GOOD_COUNTRY: &str = "DE\tDEU\tGermany\t82927922";

#[test]
fn a_corrupt_city_asset_degrades_to_an_empty_city_list() {
    // Not gzip at all, and a truncated-but-valid header — neither may panic.
    assert!(geonames::decode_cities_for_test(b"this is not gzip").is_empty());
    assert!(geonames::decode_cities_for_test(&[0x1f, 0x8b, 0x08, 0x00]).is_empty());

    // …and the resulting index is still usable for countries.
    let index = geonames::parse_for_test("", GOOD_COUNTRY);
    assert_eq!(index.counts(), (0, 1));
    assert_eq!(
        first_country_code(&index.lookup("germany", MAX_SUGGESTIONS).suggestions),
        Some("DE"),
        "a broken city asset must not take the country index down with it"
    );
}

#[test]
fn malformed_city_rows_are_skipped_not_fatal() {
    let cities = [
        GOOD_CITY,
        "Truncated\t\tDE\t52.0",                      // too few columns
        "Extra\t\tDE\t52.0\t13.0\t100\talt\tsurplus", // too many columns
        "BadLat\t\tDE\tnorth\t13.0\t100\t",           // non-numeric lat
        "BadLon\t\tDE\t52.0\teast\t100\t",            // non-numeric lon
        "\t\tDE\t52.0\t13.0\t100\t",                  // empty name
        "BadCc\t\tD\t52.0\t13.0\t100\t",              // 1-char code
        "BadCc2\t\t1A\t52.0\t13.0\t100\t",            // non-alpha code
        "BadCc3\t\té\t52.0\t13.0\t100\t",             // 2 BYTES, 1 char
        "NoPop\t\tDE\t52.0\t13.0\tlots\t",            // unparseable population
    ]
    .join("\n");

    let index = geonames::parse_for_test(&cities, GOOD_COUNTRY);

    assert_eq!(
        index.counts().0,
        2,
        "only the well-formed rows (Berlin + NoPop) may survive"
    );
    assert_eq!(
        displays(&index.lookup("nopop", MAX_SUGGESTIONS).suggestions),
        vec!["NoPop, Germany"],
        "an unparseable population defaults to 0 — the row itself is still usable"
    );
    for gone in ["truncated", "badlat", "badlon", "badcc", "extra"] {
        assert!(
            index.lookup(gone, MAX_SUGGESTIONS).suggestions.is_empty(),
            "{gone} must not be indexed"
        );
    }
}

#[test]
fn malformed_country_rows_are_skipped_not_fatal() {
    let countries = [
        GOOD_COUNTRY,
        "FR\tFRA\tFrance",        // too few columns
        "F\tFRA\tShort\t1",       // 1-char cc
        "1A\tXXX\tNumeric\t1",    // non-alpha cc
        "IT\tI\tItaly\t60000000", // malformed ISO3 — row kept, bad code dropped
        "ES\tESP\t\t46000000",    // empty name
    ]
    .join("\n");

    let index = geonames::parse_for_test(GOOD_CITY, &countries);

    assert_eq!(index.counts().1, 2, "only DE and IT are well-formed enough");
    assert_eq!(
        first_country_code(&index.lookup("italy", MAX_SUGGESTIONS).suggestions),
        Some("IT"),
        "a bad ISO3 costs the code key, not the whole country"
    );
    // "Italy" legitimately PREFIX-matches "i", so its presence proves nothing.
    // The invariant is that the malformed 1-letter ISO3 was never indexed as an
    // exact key — which is exactly what `exact` reports.
    assert!(
        !index.lookup("i", MAX_SUGGESTIONS).exact,
        "a 1-letter ISO3 must never become an exact key"
    );
}

#[test]
fn a_duplicate_country_code_keeps_the_first_row_and_its_aliases() {
    // A second `DE` row must not re-point the code index at an entry the alias
    // pass then decorates — that would strand "deutschland" on an unreachable
    // duplicate and silently break the endonym.
    let countries = [GOOD_COUNTRY, "DE\tDEU\tGermany (duplicate)\t1"].join("\n");
    let index = geonames::parse_for_test(GOOD_CITY, &countries);

    assert_eq!(index.counts().1, 1, "the duplicate row is dropped");
    let hits = index.lookup("deutschland", MAX_SUGGESTIONS);
    assert!(hits.exact, "the alias must still be an exact hit");
    assert_eq!(displays(&hits.suggestions), vec!["Germany"]);
}

#[test]
fn an_alias_for_a_missing_country_is_dropped_silently() {
    // The asset carries no CH row here, so the schweiz/suisse/svizzera aliases
    // have nothing to attach to. That must be a no-op, not a panic.
    let index = geonames::parse_for_test(GOOD_CITY, GOOD_COUNTRY);
    assert_eq!(index.counts().1, 1);
    assert!(
        index
            .lookup("schweiz", MAX_SUGGESTIONS)
            .suggestions
            .is_empty(),
        "an orphan alias resolves to nothing rather than to the wrong country"
    );
    // The alias that DOES have a country still works.
    assert!(index.lookup("deutschland", MAX_SUGGESTIONS).exact);
}

#[test]
fn confidence_comes_only_from_the_rows_the_caller_can_see() {
    // An exact ALTERNATE hit that ranks below the cap is invisible to the
    // caller, so it must not set `exact` — otherwise one unseen row vetoes the
    // Photon fallback (the whole point of the quality gate).
    let mut rows: Vec<String> = (0..MAX_SUGGESTIONS + 2)
        .map(|i| {
            // High population, name-prefix matches "zeta" → fills every slot.
            format!("Zetaville{i}\t\tDE\t1.0\t1.0\t{}\t", 9_000_000 - i as u64)
        })
        .collect();
    // Tiny town whose ALTERNATE is exactly "zeta" — ranks last on population.
    rows.push("Tinytown\t\tDE\t2.0\t2.0\t1\tzeta".to_string());
    let index = geonames::parse_for_test(&rows.join("\n"), GOOD_COUNTRY);

    let hits = index.lookup("zeta", MAX_SUGGESTIONS);
    assert_eq!(hits.suggestions.len(), MAX_SUGGESTIONS);
    assert!(
        !displays(&hits.suggestions).contains(&"Tinytown, Germany"),
        "precondition: the exact-alternate row must be pushed off the list"
    );
    assert!(
        !hits.exact,
        "an exact hit nobody was shown must not claim confidence — it would \
         veto the fallback on the strength of an invisible row"
    );
}

// ===========================================================================
// 12. Asset provenance
// ===========================================================================

#[test]
fn embedded_assets_match_their_recorded_digests() {
    use sha2::{Digest, Sha256};

    let (cities, countries) = geonames::embedded_assets();
    // `Sha256::digest` yields a byte array with no hex `Display`; fold it the
    // same way `documents::text_hash` does.
    let digest = |bytes: &[u8]| {
        Sha256::digest(bytes)
            .iter()
            .fold(String::with_capacity(64), |mut acc, b| {
                use std::fmt::Write;
                let _ = write!(acc, "{b:02x}");
                acc
            })
    };

    assert_eq!(
        digest(cities),
        geonames::CITIES_SHA256,
        "cities.tsv.gz does not match its recorded digest — regenerate the \
         provenance record in geonames.rs + geodata/README.md (pnpm gen:geonames \
         prints both), or investigate a corrupted checkout"
    );
    assert_eq!(
        digest(countries.as_bytes()),
        geonames::COUNTRIES_SHA256,
        "countries.tsv does not match its recorded digest"
    );
}

// ===========================================================================
// 13. Scripts the bundled index structurally cannot hold
// ===========================================================================

#[test]
fn non_latin_queries_bypass_the_length_floors() {
    // The asset is Latin-script only, so these can NEVER match offline. Holding
    // them to the "half-typed word" floors would make them permanently
    // unanswerable instead of merely online-only.
    for query in ["東京", "москва", "北京", "서울"] {
        assert!(
            !is_indexable_script(query),
            "{query} is not representable in the bundled index"
        );
        assert!(
            should_try_online(query, false),
            "{query} must be allowed to reach Photon despite being short"
        );
        assert!(search(query).is_empty(), "{query} cannot match offline");
    }
}

#[test]
fn latin_queries_still_obey_the_length_floors() {
    // Includes the accented forms the asset DOES carry — they must not be
    // mistaken for a foreign script and skip the fair-use floors.
    for query in ["be", "münchen", "köln", "españa"] {
        assert!(
            is_indexable_script(query),
            "{query} is representable in the bundled index"
        );
    }
    assert!(
        !should_try_online("bé", false),
        "2 Latin chars stays offline"
    );
    // Latin Extended-Additional is dropped by the asset filter, so it counts as
    // non-indexable even though it is Latin script.
    assert!(!is_indexable_script("Đà Nẵng"));
}
