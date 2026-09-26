//! `fetch_web_notes` — delegates to `commands::ai::research_answer_core`
//! (same fake-searcher pattern as that function's own tests).

use crate::commands::ai::AnswerSearcher;
use crate::error::{AppError, AppResult};

use super::super::grounding::fetch_web_notes;
use super::support::app_with_salary;

struct FakeAnswerSearcher {
    supports_web_search: bool,
    response: &'static str,
    calls: std::sync::atomic::AtomicUsize,
}

impl AnswerSearcher for FakeAnswerSearcher {
    fn research_available(&self) -> bool {
        self.supports_web_search
    }

    async fn research_answer(
        &self,
        question: &str,
        _role: &str,
        _company: &str,
    ) -> AppResult<String> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(format!("{}:{question}", self.response))
    }
}

#[tokio::test]
async fn fetch_web_notes_skips_the_charge_for_a_non_searchable_provider() {
    let limiter = crate::limits::Limiter::new();
    let searcher = FakeAnswerSearcher {
        supports_web_search: false,
        response: "notes",
        calls: std::sync::atomic::AtomicUsize::new(0),
    };

    let notes = fetch_web_notes(&searcher, &limiter, "openai", "question?", None).await;

    assert_eq!(notes, "");
    assert_eq!(
        searcher.calls.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "the search itself must never run for a non-searchable provider"
    );
    assert!(
        limiter.charge_provider_daily("openai", 1).is_ok(),
        "skipping a non-searchable provider must not consume the daily budget"
    );
}

#[tokio::test]
async fn fetch_web_notes_charges_the_daily_budget_then_returns_the_matched_role_and_company() {
    let limiter = crate::limits::Limiter::new();
    let searcher = FakeAnswerSearcher {
        supports_web_search: true,
        response: "notes",
        calls: std::sync::atomic::AtomicUsize::new(0),
    };
    let app_ctx = app_with_salary(None, None, None); // title "Rust Engineer", company "Acme"

    let notes = fetch_web_notes(&searcher, &limiter, "openai", "question?", Some(&app_ctx)).await;

    assert_eq!(notes, "notes:question?");
    assert_eq!(searcher.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(
        limiter.charge_provider_daily("openai", 1).is_err(),
        "a successful search must charge the daily budget exactly once"
    );
}

#[tokio::test]
async fn fetch_web_notes_degrades_to_empty_when_the_search_fails() {
    struct ErrSearcher;
    impl AnswerSearcher for ErrSearcher {
        fn research_available(&self) -> bool {
            true
        }
        async fn research_answer(
            &self,
            _question: &str,
            _role: &str,
            _company: &str,
        ) -> AppResult<String> {
            Err(AppError::Provider("search failed".to_string()))
        }
    }

    let limiter = crate::limits::Limiter::new();
    let notes = fetch_web_notes(&ErrSearcher, &limiter, "openai", "question?", None).await;

    assert_eq!(notes, "");
}
