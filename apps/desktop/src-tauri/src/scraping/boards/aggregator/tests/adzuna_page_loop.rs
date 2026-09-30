use super::super::*;
use super::support::*;

// ── Amount-bounded aggregator page loop (WS11) ───────────────────────────────
//
// The loop is driven by the requested AMOUNT, never by `BoardSearchInput::pages`:
// the manual search path hardcodes `pages = MAX_PAGE_BUDGET`, and Adzuna's free
// tier is a DAILY call quota, so a pages-driven loop would multiply the quota
// cost of every search. The tests below pin each clause of that contract.

/// `adzuna_page_budget` maps the requested amount onto a bounded page count.
/// The clamp is the quota guard: nothing the UI can request (amount is capped at
/// 100 upstream) may cost more than `ADZUNA_MAX_PAGES` calls, and no amount —
/// including 0 — may cost zero (that would silently return no jobs).
#[test]
fn adzuna_page_budget_is_amount_driven_and_clamped() {
    assert_eq!(adzuna_page_budget(None), 1, "no target → cheapest form");
    assert_eq!(adzuna_page_budget(Some(0)), 1, "0 must never mean 0 calls");
    assert_eq!(adzuna_page_budget(Some(1)), 1);
    assert_eq!(
        adzuna_page_budget(Some(ADZUNA_PAGE_SIZE as u32)),
        1,
        "a full single page must not spill into a second request"
    );
    assert_eq!(
        adzuna_page_budget(Some(ADZUNA_PAGE_SIZE as u32 + 1)),
        2,
        "one item past a page is what buys the second call"
    );
    assert_eq!(adzuna_page_budget(Some(100)), 2);
    assert_eq!(
        adzuna_page_budget(Some(u32::MAX)),
        ADZUNA_MAX_PAGES,
        "an absurd amount is clamped, never unbounded paging"
    );
}

/// THE quota-neutral guarantee: an amount that fits in one page issues exactly
/// ONE request — even when that page comes back FULL (which is precisely when a
/// pages-driven loop would keep going). The second-page mock's `.expect(0)` is
/// verified when the `MockServer` drops.
#[tokio::test]
async fn adzuna_amount_within_one_page_issues_exactly_one_request() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
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

    let items = fetch_adzuna_pages(adzuna_req(&server.uri()), Some(50), make_token())
        .await
        .unwrap();

    assert_eq!(items.len(), 50, "the single page's results are returned");
}

/// A larger amount pages, and a SHORT page ends the loop: page 2 comes back under
/// `ADZUNA_PAGE_SIZE`, so the loop stops there. Page 2 also REPEATS one of page 1's
/// postings (the `sort_by=date` window shifts as new jobs land between requests),
/// which must be de-duplicated rather than returned twice.
#[tokio::test]
async fn adzuna_pages_until_a_short_page_and_dedupes_across_pages() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(path(adzuna_path(1)))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(1, 50)))
        .expect(1)
        .mount(&server)
        .await;
    // ids 50, 51, 52 — id 50 repeats page 1's last posting.
    Mock::given(path(adzuna_path(2)))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(50, 3)))
        .expect(1)
        .mount(&server)
        .await;

    let items = fetch_adzuna_pages(adzuna_req(&server.uri()), Some(100), make_token())
        .await
        .unwrap();

    assert_eq!(
        items.len(),
        52,
        "50 from page 1 + 2 NEW from page 2 (the repeat is dropped)"
    );
    let unique: std::collections::HashSet<&str> = items.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(unique.len(), items.len(), "no duplicate ids may survive");
    assert!(
        items
            .iter()
            .any(|p| p.external_id.as_deref() == Some("adzuna-52")),
        "page 2's new postings must be merged in"
    );
}

/// Mid-loop failure FAILS OPEN — page 2 returning 500 keeps page 1's results
/// instead of discarding a page of real jobs (same policy as the broaden retry).
#[tokio::test]
async fn adzuna_mid_loop_failure_keeps_the_pages_already_collected() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(path(adzuna_path(1)))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(1, 50)))
        .mount(&server)
        .await;
    Mock::given(path(adzuna_path(2)))
        .respond_with(ResponseTemplate::new(500).set_body_string("upstream boom"))
        .mount(&server)
        .await;

    let items = fetch_adzuna_pages(adzuna_req(&server.uri()), Some(100), make_token())
        .await
        .expect("a later page's failure must not fail the whole search");

    assert_eq!(items.len(), 50, "page 1 survives page 2's failure");
}

/// The fail-open policy must NOT extend to page 1: it IS the provider's result,
/// and `primary_chain` relies on that `Err` to fall through to JSearch. A page-1
/// failure that silently returned `Ok(vec![])` would be the silent-empty bug.
#[tokio::test]
async fn adzuna_first_page_failure_still_propagates_as_an_error() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(path(adzuna_path(1)))
        .respond_with(ResponseTemplate::new(403).set_body_string("bad key"))
        .mount(&server)
        .await;

    let err = fetch_adzuna_pages(adzuna_req(&server.uri()), Some(100), make_token())
        .await
        .expect_err("page 1 failure must surface, not degrade to an empty Ok");
    assert!(
        err.to_string().contains("403"),
        "the HTTP status must be carried; got: {err}"
    );
}

/// Cancellation landing BETWEEN pages: the responder cancels the token while
/// serving page 1, so the loop sees a cancelled signal immediately after that
/// page lands. The run must resolve to `Ok(page 1)` — a user pressing Stop keeps
/// what was already found and spends no further quota, rather than getting an
/// error or an empty result.
///
/// The in-loop cancellation check is load-bearing, not belt-and-braces: without
/// it the next iteration would reach `fetch_json`, get `AppError::Cancelled`
/// back, and fall into the mid-loop fail-open arm — which logs a misleading
/// `"adzuna page 2 failed"` warning for what is a clean, deliberate stop.
#[tokio::test]
async fn adzuna_cancellation_between_pages_keeps_page_one_and_stops() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    /// Cancels the shared token as page 1 is served — deterministic, unlike a
    /// timer race.
    struct CancelWhileServing {
        token: tokio_util::sync::CancellationToken,
        body: serde_json::Value,
    }
    impl Respond for CancelWhileServing {
        fn respond(&self, _req: &Request) -> ResponseTemplate {
            self.token.cancel();
            ResponseTemplate::new(200).set_body_json(self.body.clone())
        }
    }

    let signal = make_token();
    let server = MockServer::start().await;
    Mock::given(path(adzuna_path(1)))
        .respond_with(CancelWhileServing {
            token: signal.clone(),
            body: adzuna_body(1, 50),
        })
        .expect(1)
        .mount(&server)
        .await;
    // Never reached: the loop breaks on the cancelled signal. (`fetch_json` also
    // refuses to send on a cancelled token — this mock pins that NO request is
    // issued either way, i.e. a Stop can never cost another quota call.)
    Mock::given(path(adzuna_path(2)))
        .respond_with(ResponseTemplate::new(200).set_body_json(adzuna_body(51, 50)))
        .expect(0)
        .mount(&server)
        .await;

    let items = fetch_adzuna_pages(adzuna_req(&server.uri()), Some(100), signal)
        .await
        .expect("a cancel between pages is a clean stop, not a failure");

    assert_eq!(items.len(), 50, "page 1's results are kept on cancel");
}

/// Cancellation landing MID-FLIGHT inside the PAGE-1 fetch is still a clean stop.
///
/// The between-pages check above cannot see this one: the token flips while page 1
/// is in the air, so `fetch_json` returns `AppError::Cancelled` and the page-1 arm
/// used to convert a deliberate Stop into a provider `Err` — which `primary_chain`
/// then logged as `"adzuna error, attempting jsearch fallback"`, naming a fallback
/// its own cancel guard would never let run.
#[tokio::test]
async fn adzuna_cancellation_during_the_first_page_is_ok_not_err() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    /// Cancels the shared token and then fails the request — the shape a real
    /// mid-flight Stop produces (the in-flight send is torn down, surfacing as a
    /// transport error on an already-cancelled token).
    struct CancelThenFail {
        token: tokio_util::sync::CancellationToken,
    }
    impl Respond for CancelThenFail {
        fn respond(&self, _req: &Request) -> ResponseTemplate {
            self.token.cancel();
            ResponseTemplate::new(500).set_body_string("torn down")
        }
    }

    let signal = make_token();
    let server = MockServer::start().await;
    Mock::given(path(adzuna_path(1)))
        .respond_with(CancelThenFail {
            token: signal.clone(),
        })
        .expect(1)
        .mount(&server)
        .await;

    let items = fetch_adzuna_pages(adzuna_req(&server.uri()), Some(100), signal)
        .await
        .expect("a cancel during page 1 is a clean stop, not an adzuna failure");

    assert!(
        items.is_empty(),
        "nothing was collected before the stop, so an empty Ok is the honest answer"
    );
}

/// A 429 (the metered API's over-quota signal) must cost exactly ONE daily-quota
/// call, not three.
///
/// `fetch_text` re-sends on 429/503 up to `FetchOptions::retries`, which defaults
/// to 2 — so the pre-fix code answered "you are over quota" by spending two more
/// of the quota that just ran out, and made the worst case of one search 9 calls
/// instead of 3. The `.expect(1)` (verified on `MockServer` drop) is the assertion.
#[tokio::test]
async fn adzuna_does_not_retry_a_429_into_extra_quota_calls() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(path(adzuna_path(1)))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .expect(1)
        .mount(&server)
        .await;

    let err = fetch_adzuna_pages(adzuna_req(&server.uri()), Some(100), make_token())
        .await
        .expect_err("an over-quota page 1 is still a provider failure");
    assert!(
        err.to_string().contains("429"),
        "the status must be carried so the user learns it was a rate limit; got: {err}"
    );
}
