use super::super::super::test_support::*;
use super::super::*;

// ── Slug validation guard ─────────────────────────────────────────────────────

/// `is_valid_personio_slug` must accept normal lowercase slugs and reject
/// values that could alter the URL authority (SSRF guard).
#[test]
fn slug_validation_accepts_valid_slugs() {
    assert!(is_valid_personio_slug("clark"));
    assert!(is_valid_personio_slug("my-company"));
    assert!(is_valid_personio_slug("acme123"));
    assert!(is_valid_personio_slug("a1b2-c3d4"));
}

#[test]
fn slug_validation_rejects_ssrf_slugs() {
    // IP with port — the classic SSRF vector for subdomain-based URLs.
    assert!(!is_valid_personio_slug("127.0.0.1:8443"));
    // Path injection.
    assert!(!is_valid_personio_slug("127.0.0.1/foo"));
    // Dot in label (would split subdomain or allow IP).
    assert!(!is_valid_personio_slug("dotted.host"));
    // Colon (port injection).
    assert!(!is_valid_personio_slug("host:8080"));
    // Leading hyphen (invalid DNS label).
    assert!(!is_valid_personio_slug("-leading"));
    // Trailing hyphen (invalid DNS label).
    assert!(!is_valid_personio_slug("trailing-"));
    // Empty string.
    assert!(!is_valid_personio_slug(""));
    // Exceeds 63-char DNS label limit.
    assert!(!is_valid_personio_slug(&"a".repeat(64)));
}

/// Every curated `ats_seed` slug for this board must pass the production
/// hostname guard — regression guard against a seed entry silently drifting
/// out of validator-compatible shape.
#[test]
fn ats_seed_personio_slugs_pass_the_guard() {
    let entries: Vec<_> = crate::scraping::boards::ats_seed::by_ats("personio").collect();
    assert!(!entries.is_empty(), "personio must have seed entries");
    for e in entries {
        assert!(
            is_valid_personio_slug(e.slug),
            "seed slug '{}' ({}) must pass is_valid_personio_slug",
            e.slug,
            e.company
        );
    }
}

/// An invalid slug must be rejected without any network request (SSRF guard).
/// A run where EVERY slug is rejected now surfaces a distinct board error
/// instead of a silent zero (claude review #597).
#[tokio::test]
async fn all_invalid_slugs_error_without_network() {
    let scraper = PersonioScraper;

    let make_input = |companies: Vec<String>| BoardSearchInput {
        companies,
        ..default_search_input()
    };
    let make_ctx = default_ctx;

    // Each all-invalid-slug run rejects pre-fetch (no network) AND now returns a
    // distinct board error rather than a silent empty result.
    for slug in ["127.0.0.1:8443", "dotted.host", "127.0.0.1/foo"] {
        let err = scraper
            .search(make_input(vec![slug.to_string()]), make_ctx())
            .await
            .expect_err("an all-invalid-slug run must be a board error, not a silent zero");
        assert!(
            err.to_string().contains("slug(s) invalid"),
            "error for '{slug}' must name the invalid-slug reason, got: {err}"
        );
    }
}

/// trust-H item 3: an all-invalid-slug run is a whole-board FAILURE (Err), not a
/// partial — so it must NOT emit a `slugs-invalid` partial note. Wires an
/// `on_note` sink and asserts it stayed empty. Network-free (every slug is
/// rejected pre-fetch, so `successful_fetches == 0` and `ats_partial_note`
/// returns `None`).
#[tokio::test]
async fn all_invalid_slugs_emits_no_note_and_errors() {
    let scraper = PersonioScraper;
    let notes = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink = notes.clone();
    let input = BoardSearchInput {
        companies: vec!["dotted.host".to_string()],
        ..default_search_input()
    };
    let ctx = ScrapeContext {
        signal: tokio_util::sync::CancellationToken::new(),
        on_progress: None,
        on_item: None,
        on_truncation: None,
        on_note: Some(std::sync::Arc::new(move |n: String| {
            sink.lock().unwrap().push(n);
        })),
    };
    let result = scraper.search(input, ctx).await;
    assert!(
        result.is_err(),
        "an all-invalid-slug run must be a board error"
    );
    assert!(
        notes.lock().unwrap().is_empty(),
        "an all-reject run must NOT emit a partial note — it's an error, not a partial"
    );
}

/// Personio now shares the `ats_finish_search` finish shape (trust-H item 1) but
/// still cancels inline in its host loop. Pins the round-1 regression for this
/// board specifically: a cancel firing right after an invalid slug is rejected
/// (via the `on_progress` callback the reject branch already calls) must return
/// `Ok`, not the all-slugs-invalid error.
#[tokio::test]
async fn cancel_after_reject_before_next_slug_returns_ok_not_all_invalid_error() {
    let scraper = PersonioScraper;
    let signal = tokio_util::sync::CancellationToken::new();
    let cancel_on_progress = signal.clone();
    let ctx = ScrapeContext {
        signal: signal.clone(),
        on_progress: Some(Box::new(move |_p: f32| cancel_on_progress.cancel())),
        on_item: None,
        on_truncation: None,
        on_note: None,
    };
    let input = BoardSearchInput {
        companies: vec!["dotted.host".to_string(), "clark".to_string()],
        ..default_search_input()
    };
    let result = scraper.search(input, ctx).await;
    assert!(
        result.is_ok(),
        "a cancel firing right after a reject must return Ok (interrupted), not the \
         all-slugs-invalid error — got {:?}",
        result.err()
    );
}
