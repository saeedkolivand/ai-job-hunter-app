use super::*;

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
