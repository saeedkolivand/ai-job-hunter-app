use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::error::AppResult;
use crate::limits::Limiter;

// ── admit_research_tests ────────────────────────────────────────────────
// `charge_daily_or_reject` is `admit_research`'s only `AppHandle`-free
// branch (the acquire/rate-limit and provider-resolution branches above
// it need a live `AppHandle`, same constraint as `research_answer_tests`
// below). Pins PR #963 round 11: a daily-budget failure must report
// `AdmitOutcome::DailyBudgetExhausted`, not silently collapse into the
// `AdmitOutcome::RateLimited` the transient per-call cap uses.

#[test]
fn a_daily_budget_within_limits_admits() {
    let limiter = Limiter::new();
    assert!(charge_daily_or_reject(&limiter, "openai", 4_000, "test").is_none());
}

#[test]
fn an_exhausted_daily_budget_is_reported_distinctly_from_a_transient_rate_limit() {
    let limiter = Limiter::new();
    // Consume the only slot of a max=1/day bucket for this provider/day.
    limiter
        .charge_provider_daily("openai", 1)
        .expect("the first charge of the day succeeds");

    let outcome = charge_daily_or_reject(&limiter, "openai", 1, "test");

    assert!(
        matches!(outcome, Some(AdmitOutcome::DailyBudgetExhausted)),
        "a daily-budget failure must not collapse into AdmitOutcome::RateLimited"
    );
}

// ── research_answer_tests ─────────────────────────────────────────────
// Unit tests for `research_answer_core` — the `AppHandle`-free heart of
// `ai_research_answer`. A fake `AnswerSearcher` + a real (but
// `AppHandle`-free) `Limiter` exercise the branching/call order that
// matters: capability check strictly BEFORE the daily charge, and the
// charge happening exactly once on the successful path. The rate-limit /
// provider-resolution branches in the `#[tauri::command]` wrapper itself
// are NOT covered here — they need a live `AppHandle`, which this crate
// has no mock harness for (see `AnswerSearcher`'s doc comment); the
// `Limiter`/`ProviderId` logic they delegate to is already covered by
// `limits::tests` and `ai_provider::mod`'s own unit tests.

struct FakeAnswerSearcher {
    supports_web_search: bool,
    response: &'static str,
    calls: AtomicUsize,
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
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(format!("{}:{question}", self.response))
    }
}

#[tokio::test]
async fn a_non_searchable_provider_returns_empty_without_charging_the_daily_budget() {
    let limiter = Limiter::new();
    let searcher = FakeAnswerSearcher {
        supports_web_search: false,
        response: "notes",
        calls: AtomicUsize::new(0),
    };

    let result =
        research_answer_core(&searcher, &limiter, "openai", "question?", "role", "co").await;

    assert_eq!(result, "");
    assert_eq!(
        searcher.calls.load(Ordering::SeqCst),
        0,
        "the search itself must never run for a non-searchable provider"
    );
    // The daily budget must be untouched: a fresh max=1 charge still succeeds.
    assert!(
        limiter.charge_provider_daily("openai", 1).is_ok(),
        "skipping a non-searchable provider must not consume the daily budget"
    );
}

#[tokio::test]
async fn a_searchable_provider_charges_the_daily_budget_then_returns_the_search_result() {
    let limiter = Limiter::new();
    let searcher = FakeAnswerSearcher {
        supports_web_search: true,
        response: "notes",
        calls: AtomicUsize::new(0),
    };

    let result =
        research_answer_core(&searcher, &limiter, "openai", "question?", "role", "co").await;

    assert_eq!(result, "notes:question?");
    assert_eq!(searcher.calls.load(Ordering::SeqCst), 1);
    // The daily budget WAS charged: a max=1 charge for the same provider
    // now trips (this call already consumed the one slot).
    assert!(
        limiter.charge_provider_daily("openai", 1).is_err(),
        "a successful search must charge the daily budget exactly once"
    );
}

#[tokio::test]
async fn a_search_failure_degrades_to_empty_after_already_charging() {
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
            Err(crate::error::AppError::Provider(
                "search failed".to_string(),
            ))
        }
    }

    let limiter = Limiter::new();
    let result =
        research_answer_core(&ErrSearcher, &limiter, "openai", "question?", "role", "co").await;

    assert_eq!(result, "");
}

// ── truncate_question ────────────────────────────────────────────────────

#[test]
fn truncate_question_caps_at_the_question_specific_max() {
    let long = "a".repeat(ANSWER_QUESTION_MAX_CHARS + 500);
    assert_eq!(
        truncate_question(&long).chars().count(),
        ANSWER_QUESTION_MAX_CHARS
    );
}

#[test]
fn truncate_question_is_a_no_op_under_the_cap() {
    assert_eq!(
        truncate_question("Why do you want this role?"),
        "Why do you want this role?"
    );
}

#[test]
fn truncate_question_preserves_a_full_question_past_the_smaller_role_company_cap() {
    // The whole point of this fix: a real custom question longer than
    // `salary_research::MAX_INPUT_CHARS` (200) — but still under this
    // question-specific cap — must survive intact, unlike the old shared
    // 200-char cap which would have cut it mid-sentence.
    let question: String = "word ".repeat(60); // 300 chars, > 200 and < 700.
    assert_eq!(truncate_question(&question), question);
    assert!(question.chars().count() > crate::salary_research::MAX_INPUT_CHARS);
}

#[test]
fn truncate_question_never_splits_a_multi_byte_character() {
    let long: String = "日".repeat(ANSWER_QUESTION_MAX_CHARS + 200);
    let truncated = truncate_question(&long);
    assert_eq!(truncated.chars().count(), ANSWER_QUESTION_MAX_CHARS);
    assert!(truncated.chars().all(|c| c == '日'));
}
