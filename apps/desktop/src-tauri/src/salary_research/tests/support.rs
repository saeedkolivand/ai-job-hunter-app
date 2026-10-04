//! Fixtures shared by the `salary_research` tests: the fake [`SalarySearcher`]s,
//! the injected deadline, and thin wrappers over [`KvCache`] and `enrich`.

use tempfile::TempDir;

use super::*;
use crate::error::AppError;

/// The bound every test injects into `enrich`. Arbitrary — production
/// derives its own from the request's reasoning effort.
pub(super) const TEST_DEADLINE: Duration = Duration::from_secs(25);

/// A fresh cache in a temp dir. Hold the guard for as long as the cache is used.
pub(super) fn open_cache() -> (TempDir, KvCache) {
    let dir = TempDir::new().expect("tempdir");
    let cache = KvCache::open(dir.path()).expect("open cache");
    (dir, cache)
}

pub(super) fn salary(min: u32, max: u32, currency: &str) -> SalaryRange {
    SalaryRange {
        min,
        max,
        currency: currency.to_string(),
    }
}

/// `enrich` for the "Backend Engineer" at "Acme" posting every test looks up.
pub(super) async fn run_enrich<S: SalarySearcher>(
    searcher: &S,
    cache: &KvCache,
    location: &str,
    country: &str,
    currency: &str,
) -> Option<SalaryRange> {
    SalaryResearch
        .enrich(
            searcher,
            Some(cache),
            "Backend Engineer",
            "Acme",
            location,
            country,
            currency,
            TEST_DEADLINE,
        )
        .await
}

pub(super) struct FakeSearcher(pub(super) &'static str);

impl SalarySearcher for FakeSearcher {
    async fn research_salary(
        &self,
        _role: &str,
        _company: &str,
        _location: &str,
        _country: &str,
        _currency: &str,
    ) -> AppResult<String> {
        Ok(self.0.to_string())
    }
}

pub(super) struct ErrSearcher;

impl SalarySearcher for ErrSearcher {
    async fn research_salary(
        &self,
        _role: &str,
        _company: &str,
        _location: &str,
        _country: &str,
        _currency: &str,
    ) -> AppResult<String> {
        Err(AppError::Provider("search failed".to_string()))
    }
}

pub(super) struct SlowSearcher;

impl SalarySearcher for SlowSearcher {
    async fn research_salary(
        &self,
        _role: &str,
        _company: &str,
        _location: &str,
        _country: &str,
        _currency: &str,
    ) -> AppResult<String> {
        // Outsleeps TEST_DEADLINE; under `start_paused = true` this resolves
        // the moment `enrich`'s own timer fires rather than blocking for
        // real. Derived from the same constant the call sites pass, so
        // changing it can never silently turn this test into a no-op.
        tokio::time::sleep(TEST_DEADLINE + Duration::from_secs(5)).await;
        Ok(r#"{"min":1,"max":2,"currency":"USD"}"#.to_string())
    }
}
