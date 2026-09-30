use super::super::*;
use super::support::*;

// ── Jooble last-resort tier ───────────────────────────────────────────────────
//
// Jooble is a THIRD tier, tried only once Adzuna AND JSearch have both failed
// to produce a decisive result (unconfigured or `Err`) — never merely because
// one of them returned an empty-but-Ok page. See `primary_chain`'s doc comment.

/// Both Adzuna and JSearch unconfigured (a Jooble-only setup) → Jooble is
/// consulted and its results win.
#[tokio::test]
async fn jooble_fires_when_adzuna_and_jsearch_unconfigured() {
    let jooble_posting = sample_posting("j1", "jooble");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::unconfigured("adzuna")),
        Box::new(FakeProvider::unconfigured("jsearch")),
        Box::new(FakeProvider::ok("jooble", vec![jooble_posting.clone()])),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "Seoul",
        "xx",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, jooble_posting.external_id);
}

/// Adzuna AND JSearch both configured-but-erroring (e.g. the unsupported-country
/// case with a JSearch key that's also failing) → Jooble is consulted and its
/// results win.
#[tokio::test]
async fn jooble_fires_after_adzuna_and_jsearch_both_err() {
    let jooble_posting = sample_posting("j2", "jooble");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err(
            "adzuna",
            "adzuna: country 'xx' is not in Adzuna's supported market list",
        )),
        Box::new(FakeProvider::err("jsearch", "jsearch upstream 500")),
        Box::new(FakeProvider::ok("jooble", vec![jooble_posting.clone()])),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "Seoul",
        "xx",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, jooble_posting.external_id);
}

/// Adzuna returns real results → Jooble must NOT be called (the fake Jooble
/// always errors, proving it wasn't reached).
#[tokio::test]
async fn jooble_not_called_when_adzuna_succeeds() {
    let adzuna_posting = sample_posting("a1", "adzuna");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::ok("adzuna", vec![adzuna_posting.clone()])),
        Box::new(FakeProvider::err("jsearch", "should not be called")),
        Box::new(FakeProvider::err("jooble", "should not be called")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, adzuna_posting.external_id);
}

/// Adzuna fails but JSearch returns real results → Jooble must NOT be called.
#[tokio::test]
async fn jooble_not_called_when_jsearch_succeeds() {
    let jsearch_posting = sample_posting("js1", "jsearch");
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err("adzuna", "adzuna down")),
        Box::new(FakeProvider::ok("jsearch", vec![jsearch_posting.clone()])),
        Box::new(FakeProvider::err("jooble", "should not be called")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await
    .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].external_id, jsearch_posting.external_id);
}

/// Adzuna + JSearch + Jooble ALL configured-but-erroring, no sparse-guessed
/// salvage available → the combined diagnostic names EVERY failing provider,
/// not just the first. Regression guard for the "only the first configured
/// failure surfaces, silently dropping the rest" bug.
#[tokio::test]
async fn all_three_configured_and_erroring_combines_every_failure() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err("adzuna", "adzuna down")),
        Box::new(FakeProvider::err("jsearch", "jsearch upstream 500")),
        Box::new(FakeProvider::err("jooble", "jooble upstream 500")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await;

    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("adzuna down"),
        "combined diagnostic must name Adzuna's failure too; got: {msg}"
    );
    assert!(
        msg.contains("jsearch upstream 500"),
        "combined diagnostic must name JSearch's failure; got: {msg}"
    );
    assert!(
        msg.contains("jooble upstream 500"),
        "combined diagnostic must name Jooble's failure — not silently dropped \
         behind Adzuna's/JSearch's; got: {msg}"
    );
    assert!(
        !msg.contains("add a JSearch"),
        "a combined multi-provider failure must not carry the stale single-provider \
         fallback-remedy suffix; got: {msg}"
    );
}

/// Adzuna + Jooble both configured-and-erroring, JSearch UNCONFIGURED — the
/// exact scenario the review flagged: previously only Adzuna's message (with a
/// now-stale "add a JSearch key" suffix) surfaced, and Jooble's real failure
/// was silently dropped. The combined diagnostic must name BOTH.
#[tokio::test]
async fn adzuna_and_jooble_both_failing_combines_both_names() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::err("adzuna", "adzuna down")),
        Box::new(FakeProvider::unconfigured("jsearch")),
        Box::new(FakeProvider::err("jooble", "jooble upstream 500")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await;

    assert!(
        result.is_err(),
        "two configured providers failing must surface Err, not silent Ok(empty)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("adzuna down"),
        "combined diagnostic must name Adzuna's failure; got: {msg}"
    );
    assert!(
        msg.contains("jooble upstream 500"),
        "combined diagnostic must name Jooble's failure, not drop it silently \
         behind Adzuna's; got: {msg}"
    );
    assert!(
        !msg.contains("add a JSearch key in Settings"),
        "must not carry the stale 'add a JSearch key' nudge when Jooble is \
         ALSO configured and already failing; got: {msg}"
    );
}

/// Adzuna + JSearch both UNCONFIGURED (a Jooble-only setup) and Jooble is
/// configured but ERRORS → the aggregator must surface Jooble's error, NOT a
/// silent `Ok(empty)`. Regression guard: a user with only a Jooble key whose
/// Jooble call fails must get an honest diagnostic, not "no jobs found" — the
/// exact silent-empty-failure bug the trust program (PR #597-#604) eliminated
/// for Adzuna/JSearch.
#[tokio::test]
async fn jooble_only_configured_failure_surfaces_error() {
    let providers: Vec<Box<dyn JobProvider>> = vec![
        Box::new(FakeProvider::unconfigured("adzuna")),
        Box::new(FakeProvider::unconfigured("jsearch")),
        Box::new(FakeProvider::err("jooble", "jooble upstream 500")),
    ];

    let result = search_with_providers(
        &providers,
        "engineer",
        "berlin",
        "de",
        false,
        None,
        SearchBudget::items_only(100),
        make_token(),
    )
    .await;

    assert!(
        result.is_err(),
        "a Jooble-only setup with a failing Jooble call must return Err, not silent Ok(empty)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("jooble upstream 500"),
        "the diagnostic must carry Jooble's own error; got: {msg}"
    );
}
