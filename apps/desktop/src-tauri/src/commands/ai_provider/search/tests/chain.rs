//! #1515: the fall-through rule. A backend that fails with billing or auth hands the pass to the
//! next backend in `search_order`, once each; every other outcome (an empty result, rate limit,
//! outage, unusable output) ends the pass where it is.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use super::*;

/// A searcher with a scripted outcome that counts its calls.
struct Counting {
    outcome: fn() -> AppResult<Vec<SearchResult>>,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl WebSearcher for Counting {
    async fn try_search(&self, _q: &str, _limit: usize) -> AppResult<Vec<SearchResult>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        (self.outcome)()
    }
}

fn scripted(
    outcome: fn() -> AppResult<Vec<SearchResult>>,
) -> (Option<Box<dyn WebSearcher>>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let searcher = Counting {
        outcome,
        calls: calls.clone(),
    };
    (Some(Box::new(searcher)), calls)
}

fn hit() -> AppResult<Vec<SearchResult>> {
    Ok(vec![SearchResult {
        title: "t".into(),
        snippet: "s".into(),
        url: "https://example.com".into(),
    }])
}
fn billing() -> AppResult<Vec<SearchResult>> {
    Err(AppError::Billing("out of credit".into()))
}
fn auth() -> AppResult<Vec<SearchResult>> {
    Err(AppError::Config(
        "ollama: invalid or unauthorized API key.".into(),
    ))
}
fn rate_limited() -> AppResult<Vec<SearchResult>> {
    Err(AppError::RateLimited("slow down".into()))
}
fn empty() -> AppResult<Vec<SearchResult>> {
    Ok(Vec::new())
}

fn calls(c: &Arc<AtomicUsize>) -> usize {
    c.load(Ordering::SeqCst)
}

#[test]
fn only_billing_and_auth_fall_through() {
    assert!(falls_through(&AppError::Billing("x".into())));
    assert!(falls_through(&AppError::Config(
        "ollama: invalid or unauthorized API key.".into()
    )));
    // A 402/401 that only reached us as text on a Network/Provider error still counts.
    assert!(falls_through(&AppError::Provider("returned 402".into())));
    assert!(!falls_through(&AppError::RateLimited("x".into())));
    assert!(!falls_through(&AppError::Network(
        "connection refused".into()
    )));
    assert!(!falls_through(&AppError::Network(
        "service error (503).".into()
    )));
    assert!(!falls_through(&AppError::Timeout("x".into())));
    assert!(!falls_through(&AppError::Parse("x".into())));
    assert!(!falls_through(&AppError::Validation("x".into())));
}

/// Native out of credit, SearXNG configured: SearXNG serves, Exa is never touched.
#[tokio::test]
async fn native_billing_falls_through_to_searxng() {
    let (native, n) = scripted(billing);
    let (searxng, s) = scripted(hit);
    let (exa, e) = scripted(hit);
    let chain = SearchChain::new(native, searxng, exa);
    assert_eq!(chain.try_search("q", 5).await.unwrap().len(), 1);
    assert_eq!((calls(&n), calls(&s), calls(&e)), (1, 1, 0));
    assert_eq!(chain.served.get(), Some(SearchBackend::Searxng));
}

/// Native key rejected, no SearXNG: Exa serves.
#[tokio::test]
async fn native_auth_falls_through_to_exa_when_there_is_no_searxng() {
    let (native, n) = scripted(auth);
    let (exa, e) = scripted(hit);
    let chain = SearchChain::new(native, None, exa);
    assert_eq!(chain.try_search("q", 5).await.unwrap().len(), 1);
    assert_eq!((calls(&n), calls(&e)), (1, 1));
    assert_eq!(chain.served.get(), Some(SearchBackend::Exa));
}

/// Mutation check: widen `falls_through` to rate limits and the fallbacks get called.
#[tokio::test]
async fn native_rate_limited_does_not_fall_through() {
    let (native, n) = scripted(rate_limited);
    let (searxng, s) = scripted(hit);
    let (exa, e) = scripted(hit);
    let chain = SearchChain::new(native, searxng, exa);
    assert!(matches!(
        chain.try_search("q", 5).await,
        Err(AppError::RateLimited(_))
    ));
    assert_eq!((calls(&n), calls(&s), calls(&e)), (1, 0, 0));
    assert_eq!(chain.served.get(), Some(SearchBackend::Native));
}

/// ADR 0023: an empty result is an answer, never retried elsewhere.
#[tokio::test]
async fn native_empty_does_not_fall_through() {
    let (native, n) = scripted(empty);
    let (searxng, s) = scripted(hit);
    let (exa, e) = scripted(hit);
    let chain = SearchChain::new(native, searxng, exa);
    assert!(chain.try_search("q", 5).await.unwrap().is_empty());
    assert_eq!((calls(&n), calls(&s), calls(&e)), (1, 0, 0));
}

/// No native search, SearXNG and Exa both configured: SearXNG, and Exa stays untouched.
#[tokio::test]
async fn without_native_searxng_serves_before_exa() {
    let (searxng, s) = scripted(hit);
    let (exa, e) = scripted(hit);
    let chain = SearchChain::new(None, searxng, exa);
    assert_eq!(chain.try_search("q", 5).await.unwrap().len(), 1);
    assert_eq!((calls(&s), calls(&e)), (1, 0));
    assert_eq!(chain.served.get(), Some(SearchBackend::Searxng));
}

/// Every backend fails on billing: each is tried exactly once and the LAST error is reported,
/// with the last backend named.
#[tokio::test]
async fn each_backend_is_tried_once_and_the_last_failure_is_reported() {
    let (native, n) = scripted(billing);
    let (searxng, s) = scripted(auth);
    let (exa, e) = scripted(billing);
    let chain = SearchChain::new(native, searxng, exa);
    assert!(matches!(
        chain.try_search("q", 5).await,
        Err(AppError::Billing(_))
    ));
    assert_eq!((calls(&n), calls(&s), calls(&e)), (1, 1, 1));
    assert_eq!(chain.served.get(), Some(SearchBackend::Exa));
}

/// A lone backend failing on billing has nowhere to go: its own error is returned.
#[tokio::test]
async fn a_single_backend_failure_is_returned_as_is() {
    let (native, n) = scripted(billing);
    let chain = SearchChain::new(native, None, None);
    assert!(matches!(
        chain.try_search("q", 5).await,
        Err(AppError::Billing(_))
    ));
    assert_eq!(calls(&n), 1);
}

/// SearXNG failing on billing/auth-shaped refusal (403, JSON disabled) hands on to Exa;
/// its 5xx does not.
#[tokio::test]
async fn a_searxng_403_falls_through_but_its_5xx_does_not() {
    let refused = super::super::searxng::searxng_status_error(reqwest::StatusCode::FORBIDDEN);
    assert!(falls_through(&refused));
    let down = super::super::searxng::searxng_status_error(reqwest::StatusCode::BAD_GATEWAY);
    assert!(!falls_through(&down));
    assert!(!falls_through(
        &super::super::searxng::searxng_status_error(reqwest::StatusCode::TOO_MANY_REQUESTS)
    ));
}
