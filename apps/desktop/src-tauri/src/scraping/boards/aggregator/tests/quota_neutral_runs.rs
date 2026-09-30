use super::super::*;
use super::support::*;

/// INVARIANT: the `FetchOptions` the METERED providers actually construct must
/// carry `retries == 0` — asserted through the same functions production calls
/// (mirrors `apify_fetch_options_must_have_retries_zero`), so deleting either
/// override breaks this test rather than silently tripling a billed call.
#[test]
fn metered_provider_fetch_options_must_have_retries_zero() {
    assert_eq!(
        adzuna_fetch_options().retries,
        0,
        "INVARIANT VIOLATED: Adzuna bills a DAILY CALL quota — a 429/503 retry \
         spends the quota that just ran out"
    );
    assert_eq!(
        jsearch_fetch_options("test-key").retries,
        0,
        "INVARIANT VIOLATED: JSearch bills PER REQUEST (× num_pages) — a retry \
         multiplies an already-multiplied call"
    );
    // The Apify tier's own copy of this invariant lives in
    // `apify_fetch_options_must_have_retries_zero`; all three are the same rule.
}

// ── Scheduled (autopilot) runs stay quota-neutral ────────────────────────────

/// Pins the request → budget TRANSLATION itself, which is the one line the
/// wiremock guards below structurally cannot see: they hand-build a
/// `SearchBudget`, so rewriting `from_input` to spend `amount` would leave them
/// all green. This is the guard that sits exactly on the original bug's line.
///
/// Both real call sites are covered, because the bug was a caller asymmetry
/// (`commands::scrape` sets a real budget; `autopilot_helpers` must not).
#[test]
fn budget_from_autopilot_shaped_input_has_no_provider_spend() {
    // Exactly what `autopilot_helpers::autopilot_scrape` builds: amount = 100 as
    // a "don't cap me" SENTINEL, no upstream spend intent.
    let autopilot = BoardSearchInput {
        amount: 100,
        pages: 3,
        provider_amount: None,
        ..make_input()
    };
    let budget = SearchBudget::from_input(&autopilot);

    assert!(
        budget.provider_amount.is_none(),
        "a scheduled run must carry NO upstream spend target — deriving one from \
         the amount sentinel doubles every autopilot's daily Adzuna quota use"
    );
    assert_eq!(
        budget.amount, 100,
        "the sentinel still applies as the OUTPUT cap"
    );

    // And the manual path's real, user-typed count maps through untouched.
    let manual = BoardSearchInput {
        amount: 25,
        // The manual path pins `pages` to its own sentinel (`MAX_PAGE_BUDGET`,
        // private to commands::scrape) — mirrored literally here to show the
        // budget ignores it.
        pages: 10,
        provider_amount: Some(25),
        ..make_input()
    };
    let budget = SearchBudget::from_input(&manual);

    assert_eq!(
        budget.provider_amount,
        Some(25),
        "a user-typed count is a real spend target and must reach the providers"
    );
    assert_eq!(budget.amount, 25);
}

/// REGRESSION GUARD (the cost bug this workstream nearly shipped).
///
/// `autopilot_helpers` sets `amount: 100` as a "don't cap me" SENTINEL — it has
/// no item-count intent, it expresses its target in pages. If the aggregator read
/// spend intent out of that sentinel, every scheduled run would buy
/// `adzuna_page_budget(Some(100)) == 2` Adzuna calls instead of 1: an hourly
/// autopilot goes 24 → 48 calls/day against a HARD DAILY quota, and exhaustion
/// then compounds (Adzuna `Err` → the JSearch fallback, itself billed 3× under
/// the same sentinel).
///
/// So the budget rides `provider_amount`, which a scheduled run leaves `None`.
/// This pins the observable consequence — the request COUNT through an
/// autopilot-shaped input — rather than the mapping function in isolation.
#[tokio::test]
async fn autopilot_shaped_input_spends_exactly_one_adzuna_request() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    // Page 1 comes back FULL — the case where a budget read off `amount` would
    // keep paging. Only `provider_amount: None` stops it here.
    Mock::given(path(adzuna_path(1)))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(1, 50)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path(adzuna_path(2)))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(51, 50)))
        .expect(0)
        .mount(&server)
        .await;

    // Exactly the shape `autopilot_helpers::autopilot_scrape` builds.
    let autopilot_budget = SearchBudget::new(100, None);
    let provider = wiremock_adzuna(server.uri(), None);

    let items = search_with_providers(
        &[Box::new(provider) as Box<dyn JobProvider>],
        "engineer",
        "Berlin",
        "de",
        false,
        None,
        autopilot_budget,
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(
        items.len(),
        50,
        "the single page's results still flow through unchanged"
    );
    // The `.expect(0)` above is the actual quota assertion, verified on drop.
}

/// The manual search path (`commands::scrape`) DOES set `provider_amount`, so the
/// same provider pages — proving the guard above is about the missing budget, not
/// a loop that never runs through `search_with_providers`.
#[tokio::test]
async fn manual_shaped_input_pages_when_a_provider_budget_is_set() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(path(adzuna_path(1)))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(1, 50)))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path(adzuna_path(2)))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(51, 10)))
        .expect(1)
        .mount(&server)
        .await;

    let provider = wiremock_adzuna(server.uri(), None);

    let items = search_with_providers(
        &[Box::new(provider) as Box<dyn JobProvider>],
        "engineer",
        "Berlin",
        "de",
        false,
        None,
        SearchBudget::new(100, Some(100)),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(items.len(), 60, "both pages are collected");
}
