use super::super::*;
use super::support::*;

// ── Additive merge / dedup ─────────────────────────────────────────────────────

/// Apify results merge ADDITIVELY onto the primary result (not as a fallback) and
/// dedupe by URL: a LinkedIn item sharing the primary's URL is dropped; the
/// primary keeps its first-seen position.
#[tokio::test]
async fn apify_merges_additively_and_dedupes_by_url() {
    let primary = sample_posting("1", "adzuna");
    let li_unique = sample_posting("2", "linkedin");
    // Same URL as the primary, but a different external_id → must dedupe out.
    let mut li_dup = sample_posting("3", "linkedin");
    li_dup.url = primary.url.clone();

    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", vec![primary.clone()])),
        Box::new(FakeProvider::ok(
            "apify_linkedin",
            vec![li_unique.clone(), li_dup.clone()],
        )),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::manual(100),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 2, "primary + unique LinkedIn; dup dropped");
    // Deterministic order: primary first, then LinkedIn.
    assert_eq!(result[0].url, primary.url);
    assert_eq!(result[1].url, li_unique.url);
}

/// Only Apify configured (no Adzuna/JSearch) → its results are returned (primary
/// chain yields keyless-empty, LinkedIn merges onto it).
#[tokio::test]
async fn only_apify_configured_returns_apify_items() {
    let li = sample_posting("x", "linkedin");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::unconfigured("adzuna")),
        Box::new(FakeProvider::unconfigured("jsearch")),
        Box::new(FakeProvider::ok("apify_linkedin", vec![li.clone()])),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::manual(100),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, li.external_id);
}

/// Apify NOT configured → behaviour is identical to the legacy chain: the primary
/// result passes through untouched (here, the Adzuna-failed-no-JSearch diagnostic
/// Err is preserved, NOT swallowed by the merge path).
#[tokio::test]
async fn apify_unconfigured_preserves_primary_diagnostic_err() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err("adzuna", "timeout")),
        Box::new(FakeProvider::unconfigured("jsearch")),
        Box::new(FakeProvider::unconfigured("apify_linkedin")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::manual(100),
        make_token(),
    )
    .await;

    assert!(result.is_err(), "primary diagnostic Err must be preserved");
    assert!(result.unwrap_err().to_string().contains("timeout"));
}

/// Primary fails but Apify is configured and returns results → show the LinkedIn
/// results rather than hide them behind the primary diagnostic.
#[tokio::test]
async fn apify_results_override_primary_error_when_present() {
    let li = sample_posting("li", "linkedin");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err("adzuna", "timeout")),
        Box::new(FakeProvider::unconfigured("jsearch")),
        Box::new(FakeProvider::ok("apify_linkedin", vec![li.clone()])),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::manual(100),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, li.external_id);
}

// ── The paid tier spends `provider_amount`, never `amount` ──────────────────

/// The cost contract of the paid LinkedIn tier, asserted on the DECISION
/// (`apify_cap`) rather than on merged output — output-only assertions are what
/// let the amount-driven gate survive this long.
#[test]
fn apify_cap_is_zero_without_an_upstream_budget() {
    // The autopilot shape: a big OUTPUT cap that is a "don't cap me" SENTINEL,
    // with no spend intent. It must buy nothing, at any primary size.
    assert_eq!(
        apify_cap(SearchBudget::items_only(100), 0),
        0,
        "a scheduled run has no upstream budget — a paid actor run must never be bought"
    );
    assert_eq!(apify_cap(SearchBudget::items_only(100), 7), 0);
    assert_eq!(apify_cap(SearchBudget::items_only(0), 0), 0);

    // A real, user-typed spend target buys only the still-UNMET part of it.
    assert_eq!(apify_cap(SearchBudget::manual(25), 0), 25);
    assert_eq!(apify_cap(SearchBudget::manual(25), 10), 15);
    assert_eq!(
        apify_cap(SearchBudget::manual(25), 25),
        0,
        "budget already satisfied by the free primary → LinkedIn is a fill for UNMET capacity"
    );
    assert_eq!(
        apify_cap(SearchBudget::manual(25), 40),
        0,
        "over-satisfied must saturate at 0, never wrap into a huge cap"
    );
    assert_eq!(
        apify_cap(SearchBudget::manual(100), 0),
        APIFY_MAX_ITEMS,
        "never above the platform-enforced maxItems ceiling"
    );
}

/// REGRESSION GUARD (the cost bug PR #896's review caught).
///
/// The gate used to read `budget.amount`, which `autopilot_helpers` passes as the
/// sentinel `100`. Primary results are essentially never ≥ 100, so the gate always
/// opened and EVERY scheduled run bought a 50-item Apify actor run — while three
/// doc comments claimed `provider_amount` was the only thing buying upstream calls.
///
/// Asserted at the provider ARGUMENT (via [`RecordingProvider`]): a
/// `FakeProvider`-based output assertion cannot tell "never called" from "called
/// and returned nothing".
#[tokio::test]
async fn autopilot_shaped_budget_never_calls_the_paid_apify_tier() {
    let (apify, apify_calls) =
        RecordingProvider::new("apify_linkedin", vec![sample_posting("li", "linkedin")]);
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok(
            "adzuna",
            vec![sample_posting("1", "adzuna")],
        )),
        Box::new(apify),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        // Exactly what `autopilot_helpers::autopilot_scrape` builds.
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .unwrap();

    assert!(
        apify_calls.lock().expect("recorder mutex").is_empty(),
        "a scheduled run must issue NO paid Apify request — it carries no upstream spend budget"
    );
    assert_eq!(
        result.len(),
        1,
        "the free primary result still flows through untouched"
    );
}

/// The other half of the guard above: a MANUAL search does carry a spend target,
/// so the paid tier runs — asked for exactly the unmet remainder, proving the
/// gate is about the missing budget and not a tier that never fires.
#[tokio::test]
async fn manual_shaped_budget_asks_apify_for_the_unmet_remainder_only() {
    let (apify, apify_calls) = RecordingProvider::new("apify_linkedin", vec![]);
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok(
            "adzuna",
            vec![sample_posting("1", "adzuna"), sample_posting("2", "adzuna")],
        )),
        Box::new(apify),
    ];

    search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::manual(10),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(
        apify_calls.lock().expect("recorder mutex").as_slice(),
        [Some(8)],
        "one call, capped at the 8 results the free primary did not cover"
    );
}
