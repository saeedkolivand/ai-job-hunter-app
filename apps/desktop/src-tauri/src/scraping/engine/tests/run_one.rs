//! `ScraperEngine::run_one` tests: the `amount` cap, cap-reaching
//! cancellation recovery, and cancellation-token wiring.

use super::super::*;
use super::support::*;

// ── run_one tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn central_amount_cap_truncates_stream_and_return() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let fake = FakeScraper::http(50);

    // Count how many items the renderer-facing callback actually receives.
    let streamed = std::sync::Arc::new(AtomicUsize::new(0));
    let streamed_cb = streamed.clone();
    let on_item: Box<dyn Fn(JobPosting) + Send> = Box::new(move |_item| {
        streamed_cb.fetch_add(1, Ordering::SeqCst);
    });

    let token = CancellationToken::new();
    let items = ScraperEngine::run_one(
        "fake",
        Ok(&fake as &dyn crate::scraping::types::Scraper),
        fake_input(20),
        token,
        None,
        Some(on_item),
        None,
        None,
        None,
    )
    .await
    .expect("capped scrape recovers as success");

    assert_eq!(
        streamed.load(Ordering::SeqCst),
        20,
        "exactly `amount` items forwarded to the renderer"
    );
    assert_eq!(items.len(), 20, "returned Vec truncated to `amount`");
}

#[tokio::test]
async fn cancel_reaches_a_pre_registered_token() {
    let engine = ScraperEngine::new();
    let token = tokio_util::sync::CancellationToken::new();

    // Autopilot registers its own token for the whole run, then scrape_boards
    // reuses (does not overwrite) that slot — so the clone the run keeps for
    // its post-scrape phase must flip when a tray/UI cancel hits the job.
    engine.register_token("run-1", token.clone()).await;
    assert!(!token.is_cancelled());
    engine.cancel("run-1").await;
    assert!(token.is_cancelled());
}

/// The lost-cancel regression guard: a cancel that lands BEFORE the run reaches
/// the engine must still be honored — no items streamed, `Err("scrape
/// cancelled")` returned.
///
/// `cancel` used to `remove()` the slot before cancelling it, so a run starting
/// afterwards found no slot, MINTED a fresh un-cancelled token, and scraped on
/// as if nothing had happened — its streamed items then clobbering whatever the
/// user started next. The window is real: callers do async work between
/// `register_token` and the engine call (`commands::scrape` awaits a geocode
/// backfill) and the engine itself waits on `sem.acquire_owned()`.
///
/// Drives the REAL path (`scrape_boards_with_resolver` + `FakeScraper`), not the
/// map bookkeeping — a token registered after a cancel is un-cancelled by
/// construction, so asserting that would prove nothing.
#[tokio::test]
async fn a_cancel_before_the_run_reaches_the_engine_is_honored() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static FAKE: std::sync::LazyLock<FakeScraper> =
        std::sync::LazyLock::new(|| FakeScraper::http(25));

    let engine = ScraperEngine::new();
    let token = CancellationToken::new();
    engine
        .register_token("job-cancel-before-engine", token.clone())
        .await;

    // The user hits Stop while the command is still in its pre-scrape phase.
    engine.cancel("job-cancel-before-engine").await;
    assert!(token.is_cancelled(), "the caller's own clone must flip");

    let streamed = std::sync::Arc::new(AtomicUsize::new(0));
    let streamed_cb = streamed.clone();

    let result = engine
        .scrape_boards_with_resolver(
            &["board-x".to_string()],
            fake_input(25),
            "job-cancel-before-engine".to_string(),
            None,
            Some(std::sync::Arc::new(move |_item| {
                streamed_cb.fetch_add(1, Ordering::SeqCst);
            })),
            std::path::Path::new("."),
            |_id| Ok(&*FAKE as &'static dyn crate::scraping::types::Scraper),
        )
        .await;

    assert_eq!(
        streamed.load(Ordering::SeqCst),
        0,
        "a run the user already cancelled must stream ZERO items — anything \
         streamed here lands in the postings cache and wipes the search the \
         user started instead"
    );
    let err = result.expect_err("a fully-cancelled run must report Err, not a silent empty Ok");
    assert!(
        err.to_string().contains("cancelled"),
        "the error must name cancellation, got: {err}"
    );
}

/// `cancel` is IDEMPOTENT: a second cancel for the same job — a double-click on
/// Stop, or a tray cancel racing the UI — still resolves to the same token rather
/// than silently doing nothing, and the slot survives so the owner's
/// `unregister_token` stays its single remover.
///
/// Scope note: this asserts idempotence only. The *semantics* that keeping the
/// slot in place buys — a cancel arriving before the run reaches the engine is
/// still honoured (rather than lost to a freshly minted token) — are guarded by
/// `a_cancel_before_the_run_reaches_the_engine_is_honored` above, not here.
#[tokio::test]
async fn cancel_leaves_the_slot_in_place_and_is_idempotent() {
    let engine = ScraperEngine::new();
    let token = CancellationToken::new();
    engine
        .register_token("job-double-cancel", token.clone())
        .await;

    engine.cancel("job-double-cancel").await;
    engine.cancel("job-double-cancel").await;
    assert!(token.is_cancelled());

    // The slot survives, so the owner's `unregister_token` is still the single
    // remover — the invariant the no-leak argument in `cancel`'s doc rests on.
    engine.unregister_token("job-double-cancel").await;
    let late = CancellationToken::new();
    engine
        .register_token("job-double-cancel", late.clone())
        .await;
    engine.unregister_token("job-double-cancel").await;
    assert!(
        !late.is_cancelled(),
        "a fresh registration after the owner released the slot must be clean"
    );
}

/// The engine's three cancel verbs now DELEGATE to the shared
/// `jobs::cancel::CancelRegistry`, and `lib.rs` manages the very handle
/// `cancel_registry()` returns as app state. That sharing is what keeps a
/// non-scraping run cancellable: `commands::resume_pipeline::resume_pipeline_run`
/// registers through the registry while `jobs_cancel` still dispatches
/// through `engine.cancel`. If the two ever became separate maps, a résumé
/// pipeline run's Stop button would silently do nothing — so pin BOTH
/// directions.
#[tokio::test]
async fn the_engine_and_its_exposed_registry_are_the_same_map() {
    let engine = ScraperEngine::new();
    let registry = engine.cancel_registry();

    // Registered like an agent run (registry side) → cancelled like any job
    // (engine side, which is what `jobs_cancel` calls).
    let agent_token = CancellationToken::new();
    registry.register("job-agent", agent_token.clone()).await;
    engine.cancel("job-agent").await;
    assert!(
        agent_token.is_cancelled(),
        "a registry-side registration must be reachable by engine.cancel"
    );

    // ...and the reverse: an engine-side registration is visible to the registry.
    let scrape_token = CancellationToken::new();
    engine
        .register_token("job-scrape", scrape_token.clone())
        .await;
    registry.cancel("job-scrape").await;
    assert!(scrape_token.is_cancelled());

    // Both slots live in ONE map, and either side can release them.
    assert_eq!(registry.live_count().await, 2);
    engine.unregister_token("job-agent").await;
    registry.unregister("job-scrape").await;
    assert_eq!(registry.live_count().await, 0);
}
