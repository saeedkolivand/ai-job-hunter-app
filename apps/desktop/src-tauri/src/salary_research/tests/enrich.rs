use super::{support::*, *};

// ── cache round-trip (the same namespace/TTL constants `enrich` uses) ────

#[test]
fn cache_round_trips_a_validated_range_under_the_salary_namespace() {
    let (_dir, cache) = open_cache();
    let range = salary(65000, 80000, "EUR");
    let json = serde_json::to_string(&range).expect("serialize");

    cache.set(CACHE_NS, "backend engineer|acme|berlin", &json);

    let cached = cache
        .get(CACHE_NS, "backend engineer|acme|berlin", TTL_SECS)
        .expect("cache hit");
    assert_eq!(parse_and_validate(&cached), Some(range));
}

// ── enrich (fake SalarySearcher — reaches the parse-SUCCESS/cache paths
// without a live `AppHandle`/network) ────────────────────────────────────

/// `searcher` must yield no range for the Berlin lookup and leave the cache untouched.
async fn assert_none_and_uncached<S: SalarySearcher>(searcher: S) {
    let (_dir, cache) = open_cache();

    let result = run_enrich(&searcher, &cache, "Berlin", "", "").await;

    assert_eq!(result, None);
    let key = cache_key("Backend Engineer", "Acme", "Berlin", "");
    assert_eq!(cache.get(CACHE_NS, &key, TTL_SECS), None);
}

#[tokio::test]
async fn enrich_returns_a_range_on_valid_json_and_writes_through_the_cache() {
    let (_dir, cache) = open_cache();
    let searcher = FakeSearcher(r#"{"min":65000,"max":80000,"currency":"EUR"}"#);

    let result = run_enrich(&searcher, &cache, "Berlin", "", "").await;

    assert_eq!(
        result,
        Some(SalaryRange {
            min: 65000,
            max: 80000,
            currency: "EUR".to_string()
        })
    );
    let key = cache_key("Backend Engineer", "Acme", "Berlin", "");
    assert!(
        cache.get(CACHE_NS, &key, TTL_SECS).is_some(),
        "a successful lookup must write through the cache"
    );
}

#[tokio::test]
async fn enrich_returns_none_and_does_not_cache_on_no_reliable_data() {
    assert_none_and_uncached(FakeSearcher("{}")).await;
}

#[tokio::test]
async fn enrich_returns_none_and_does_not_cache_on_malformed_output() {
    assert_none_and_uncached(FakeSearcher("not json at all")).await;
}

#[tokio::test]
async fn enrich_returns_none_and_does_not_cache_on_a_searcher_error() {
    assert_none_and_uncached(ErrSearcher).await;
}

#[tokio::test(start_paused = true)]
async fn enrich_returns_none_and_does_not_cache_when_the_searcher_exceeds_the_timeout() {
    assert_none_and_uncached(SlowSearcher).await;
}

// ── currency grounding (the bug fix): enrich with an expected currency ────

#[tokio::test]
async fn enrich_drops_a_hallucinated_currency_when_the_expected_currency_is_known() {
    // The bug this fixes: a German role with a weak location previously let
    // the model report USD. A stray USD slipping past the prompt-level pin
    // must never be relabeled EUR (wrong numbers under the right symbol) —
    // it's dropped instead, degrading to the C1 fallback like any other
    // failed lookup.
    let (_dir, cache) = open_cache();
    let searcher = FakeSearcher(r#"{"min":65000,"max":80000,"currency":"USD"}"#);

    let result = run_enrich(&searcher, &cache, "Berlin", "DE", "EUR").await;

    assert_eq!(result, None);
    // A dropped (mismatched) range must never be cached.
    let key = cache_key("Backend Engineer", "Acme", "Berlin", "EUR");
    assert_eq!(cache.get(CACHE_NS, &key, TTL_SECS), None);
}

#[tokio::test]
async fn enrich_self_heals_a_stale_mismatched_cache_entry_via_a_fresh_fetch() {
    // Simulates a stale cache entry written before this fix shipped (wrong
    // currency baked in). It must not be returned as-is — `enrich` falls
    // through to a fresh fetch, which (once it succeeds in the right
    // currency) overwrites the stale row.
    let (_dir, cache) = open_cache();
    let key = cache_key("Backend Engineer", "Acme", "Berlin", "EUR");
    cache.set(
        CACHE_NS,
        &key,
        r#"{"min":65000,"max":80000,"currency":"USD"}"#,
    );
    let searcher = FakeSearcher(r#"{"min":70000,"max":90000,"currency":"EUR"}"#);

    let result = run_enrich(&searcher, &cache, "Berlin", "DE", "EUR").await;

    assert_eq!(
        result,
        Some(SalaryRange {
            min: 70000,
            max: 90000,
            currency: "EUR".to_string()
        })
    );
    let cached = cache.get(CACHE_NS, &key, TTL_SECS).expect("cached");
    assert_eq!(parse_and_validate(&cached).unwrap().currency, "EUR");
}

#[tokio::test]
async fn enrich_returns_none_when_a_stale_cache_entry_mismatches_and_the_fresh_fetch_fails() {
    // Fail-safe end-to-end: a stale mismatched cache entry is never
    // returned, even when the fallback fresh fetch also comes up empty.
    let (_dir, cache) = open_cache();
    let key = cache_key("Backend Engineer", "Acme", "Berlin", "EUR");
    cache.set(
        CACHE_NS,
        &key,
        r#"{"min":65000,"max":80000,"currency":"USD"}"#,
    );

    let result = run_enrich(&ErrSearcher, &cache, "Berlin", "DE", "EUR").await;

    assert_eq!(result, None);
}

#[tokio::test]
async fn enrich_leaves_the_currency_untouched_when_the_country_is_unknown() {
    // Unknown-country guard end-to-end: empty country/currency must not
    // change today's behavior at all.
    let (_dir, cache) = open_cache();
    let searcher = FakeSearcher(r#"{"min":65000,"max":80000,"currency":"USD"}"#);

    let result = run_enrich(&searcher, &cache, "", "", "").await;

    assert_eq!(
        result,
        Some(SalaryRange {
            min: 65000,
            max: 80000,
            currency: "USD".to_string()
        })
    );
}
