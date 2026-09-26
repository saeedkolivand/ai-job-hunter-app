//! `resolve_salary_range` — the `SalarySearcher` market-lookup path (budget
//! skip + cache reuse).

use crate::error::AppResult;
use crate::salary_research::SalarySearcher;

use super::super::grounding::resolve_salary_range;
use super::support::app_with_salary;

struct FakeSalarySearcher {
    calls: std::sync::atomic::AtomicUsize,
}

impl SalarySearcher for FakeSalarySearcher {
    async fn research_salary(
        &self,
        _role: &str,
        _company: &str,
        _location: &str,
        _country: &str,
        _currency: &str,
    ) -> AppResult<String> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(r#"{"min":1,"max":2,"currency":"USD"}"#.to_string())
    }
}

#[tokio::test]
async fn resolve_salary_range_skips_the_lookup_when_the_daily_budget_is_exhausted() {
    let limiter = crate::limits::Limiter::new();
    // Exhaust the SAME per-provider daily ceiling `resolve_salary_range`
    // itself charges against — a plain in-memory HashMap increment per
    // iteration, so 4,000 of them is sub-millisecond, not a real wait.
    for _ in 0..crate::limits::PROVIDER_DAILY_MAX {
        limiter
            .charge_provider_daily("openai", crate::limits::PROVIDER_DAILY_MAX)
            .expect("charge within the daily ceiling");
    }

    // A role/company but no scraped salary range, so this must reach the
    // budget check rather than short-circuiting on `scraped_salary_range`.
    let app_ctx = app_with_salary(None, None, None);
    let searcher = FakeSalarySearcher {
        calls: std::sync::atomic::AtomicUsize::new(0),
    };

    let range = resolve_salary_range(&searcher, &limiter, "openai", None, Some(&app_ctx)).await;

    assert!(range.is_none());
    assert_eq!(
        searcher.calls.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "the market lookup must never run once the daily budget is exhausted"
    );
}

/// The fix for the "salary-answer re-spends on every Prep click" finding — see
/// `resolve_salary_range`'s own doc for the cache wiring.
#[tokio::test]
async fn resolve_salary_range_reuses_the_cache_instead_of_re_spending_on_a_repeat_lookup() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let cache = crate::pipeline::cache::KvCache::open(dir.path()).expect("open cache");
    let limiter = crate::limits::Limiter::new();
    let app_ctx = app_with_salary(None, None, None);
    let searcher = FakeSalarySearcher {
        calls: std::sync::atomic::AtomicUsize::new(0),
    };

    let first =
        resolve_salary_range(&searcher, &limiter, "openai", Some(&cache), Some(&app_ctx)).await;
    assert!(
        first.is_some(),
        "the fresh (cache-miss) lookup must succeed"
    );
    assert_eq!(searcher.calls.load(std::sync::atomic::Ordering::SeqCst), 1);

    let second =
        resolve_salary_range(&searcher, &limiter, "openai", Some(&cache), Some(&app_ctx)).await;
    assert_eq!(
        second, first,
        "a repeat lookup for the same role/company must return the SAME cached range"
    );
    assert_eq!(
        searcher.calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the second call must hit the 7-day cache, never a second provider round trip"
    );
}

/// The exact regression the review flagged: quota used to be charged BEFORE the cache was
/// consulted, so a cache HIT still burned the daily budget, and once exhausted a value already
/// cached could no longer be read. Prime the cache, THEN exhaust the daily budget, and assert the
/// cached value still comes back.
#[tokio::test]
async fn resolve_salary_range_reads_a_cache_hit_even_once_the_daily_budget_is_exhausted() {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let cache = crate::pipeline::cache::KvCache::open(dir.path()).expect("open cache");
    let limiter = crate::limits::Limiter::new();
    let app_ctx = app_with_salary(None, None, None);
    let searcher = FakeSalarySearcher {
        calls: std::sync::atomic::AtomicUsize::new(0),
    };

    // Prime: the one call in this test allowed to actually spend budget.
    let primed =
        resolve_salary_range(&searcher, &limiter, "openai", Some(&cache), Some(&app_ctx)).await;
    assert!(primed.is_some(), "the priming lookup must succeed");
    assert_eq!(searcher.calls.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Exhaust the rest of the SAME per-provider daily ceiling `resolve_salary_range` charges
    // against (the priming call above already spent one unit of it).
    for _ in 1..crate::limits::PROVIDER_DAILY_MAX {
        limiter
            .charge_provider_daily("openai", crate::limits::PROVIDER_DAILY_MAX)
            .expect("charge within the daily ceiling");
    }
    assert!(
        limiter
            .charge_provider_daily("openai", crate::limits::PROVIDER_DAILY_MAX)
            .is_err(),
        "the daily budget must now be fully exhausted"
    );

    let cached =
        resolve_salary_range(&searcher, &limiter, "openai", Some(&cache), Some(&app_ctx)).await;
    assert_eq!(
        cached, primed,
        "a cache hit must still return the value, even with the daily budget fully exhausted"
    );
    assert_eq!(
        searcher.calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "a cache hit must never touch the searcher — and, per the fix, must never need to \
         charge the (already exhausted) quota either"
    );
}
