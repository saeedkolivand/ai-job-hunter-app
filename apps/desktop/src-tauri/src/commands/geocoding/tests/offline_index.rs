use super::*;

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
