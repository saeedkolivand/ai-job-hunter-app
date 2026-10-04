use super::*;

// ── backfill_country_code (pre-scrape cancellation) ──────────────────────
//
// Driven through the injected-lookup seam so every case is hermetic: no
// geocode round trip, no `AppHandle`, no timing sleeps.

fn input_with(location: Option<&str>, country_code: Option<&str>) -> BoardSearchInput {
    BoardSearchInput {
        query: "rust".to_string(),
        location: location.map(str::to_string),
        amount: 25,
        pages: 1,
        provider_amount: None,
        date_filter: None,
        job_type: None,
        work_types: None,
        experience_level: None,
        easy_apply: None,
        actively_hiring: None,
        verified: None,
        sort_by: None,
        country_code: country_code.map(str::to_string),
        latitude: None,
        longitude: None,
        radius_km: None,
        companies: Vec::new(),
    }
}

/// Already cancelled when the task wakes → abandon the run AND never poll the
/// lookup. The `biased;` ordering is what guarantees the second half: an
/// unbiased select picks a ready branch at random and could fire the geocode
/// request for a run nobody is waiting on.
#[tokio::test]
async fn pre_cancelled_run_is_abandoned_without_polling_the_lookup() {
    let token = tokio_util::sync::CancellationToken::new();
    token.cancel();
    let mut input = input_with(Some("Germany"), None);
    let polled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = polled.clone();

    let proceed = backfill_country_code_with(&token, &mut input, async move {
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
        Some("de".to_string())
    })
    .await;

    assert!(!proceed, "a cancelled run must not proceed to the scrape");
    assert!(
        !polled.load(std::sync::atomic::Ordering::SeqCst),
        "the geocode lookup must never be issued for an already-cancelled run"
    );
    assert!(input.country_code.is_none());
}

/// A cancel landing WHILE the lookup is in flight wins immediately — the run
/// is abandoned instead of waiting out the 2s geocode cap. `pending()` stands
/// in for a hung Photon-fallback call: without the select the test would hang.
#[tokio::test]
async fn cancel_during_the_lookup_abandons_the_run() {
    let token = tokio_util::sync::CancellationToken::new();
    let mut input = input_with(Some("Amsterdam"), None);

    let canceller = token.clone();
    tokio::spawn(async move {
        tokio::task::yield_now().await;
        canceller.cancel();
    });

    let proceed =
        backfill_country_code_with(&token, &mut input, std::future::pending::<Option<String>>())
            .await;

    assert!(
        !proceed,
        "a cancel during the lookup must abandon the run, not wait it out"
    );
    assert!(
        input.country_code.is_none(),
        "an interrupted lookup must leave the field absent"
    );
}

/// The happy path: the lookup resolves, its country is written, and the run
/// proceeds.
#[tokio::test]
async fn a_resolved_lookup_fills_the_country_and_proceeds() {
    let token = tokio_util::sync::CancellationToken::new();
    let mut input = input_with(Some("Amsterdam"), None);

    let proceed =
        backfill_country_code_with(&token, &mut input, std::future::ready(Some("nl".into()))).await;

    assert!(proceed);
    assert_eq!(input.country_code.as_deref(), Some("nl"));
}

/// A country the user PICKED is authoritative: no lookup is polled and the
/// value is never overwritten (the backfill is for typed locations only).
#[tokio::test]
async fn an_existing_country_code_is_kept_and_costs_no_lookup() {
    let token = tokio_util::sync::CancellationToken::new();
    let mut input = input_with(Some("Austin, United States"), Some("us"));
    let polled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = polled.clone();

    let proceed = backfill_country_code_with(&token, &mut input, async move {
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
        Some("de".to_string())
    })
    .await;

    assert!(proceed);
    assert_eq!(
        input.country_code.as_deref(),
        Some("us"),
        "a picked country must never be clobbered by the backfill"
    );
    assert!(
        !polled.load(std::sync::atomic::Ordering::SeqCst),
        "no geocode request may be issued when the country is already known"
    );
}
