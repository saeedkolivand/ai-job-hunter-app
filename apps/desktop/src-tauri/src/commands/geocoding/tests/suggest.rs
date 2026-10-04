use super::*;

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
    assert_is_weak_and_online_eligible("rotterda");

    let (_server, endpoint) = photon_mock(ResponseTemplate::new(200).set_body_json(json!({
        "features": [photon_feature(
            json!({ "type": "city", "name": "Rotterdam", "country": "Netherlands", "countrycode": "NL" }),
            Some([4.47, 51.92]),
        )]
    })))
    .await;

    let results = suggest_at(&endpoint, "rotterda").await;

    assert_eq!(
        displays(&results),
        vec!["Rotterdam, Netherlands"],
        "a real geocoder's answer must REPLACE the prefix accident, not append to it"
    );
    assert_eq!(first_country_code(&results), Some("NL"));
}

#[tokio::test]
async fn an_empty_photon_answer_leaves_the_offline_rows_alone() {
    assert_is_weak_and_online_eligible("barcelo");
    let offline = search("barcelo");

    let (_server, endpoint) =
        photon_mock(ResponseTemplate::new(200).set_body_json(json!({ "features": [] }))).await;

    let results = suggest_at(&endpoint, "barcelo").await;

    assert_eq!(
        results, offline,
        "Photon finding nothing must never destroy the rows we already had"
    );
    assert!(!results.is_empty());
}

#[tokio::test]
async fn a_failed_photon_lookup_leaves_the_offline_rows_alone() {
    assert_is_weak_and_online_eligible("stockhol");
    let offline = search("stockhol");

    let (_server, endpoint) =
        photon_mock(ResponseTemplate::new(503).set_body_string("rate limited")).await;

    let results = suggest_at(&endpoint, "stockhol").await;

    assert_eq!(
        results, offline,
        "a 5xx must degrade to the offline rows, not to nothing"
    );
}

#[tokio::test]
async fn repeated_lookups_are_served_from_the_memo_cache() {
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
