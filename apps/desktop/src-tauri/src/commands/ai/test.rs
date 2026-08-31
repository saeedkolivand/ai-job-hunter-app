use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tempfile::TempDir;

use super::*;
use crate::error::AppError;
use crate::error::AppResult;
use crate::limits::Limiter;

// ── admit_embed_tests ───────────────────────────────────────────────────
// `admit_embed` is `ai_embed`'s `AppHandle`-free admission core (same
// constraint as `admit_research`/`research_answer_core` above — this crate
// has no `tauri::test` mock-app harness). The rate/concurrency primitives
// themselves are already covered generically by `limits::test`; what's
// pinned here is `ai_embed`'s OWN addition — that a call is actually charged
// against the resolved EMBEDDING provider's daily budget, not left
// unguarded. Mutation check: deleting the `charge_provider_daily` call
// inside `admit_embed` makes the second call below wrongly succeed and
// fails this test.

#[test]
fn admit_embed_refuses_once_the_embedding_providers_daily_budget_is_exhausted() {
    let limiter = Arc::new(Limiter::new());
    // Room to spare on rate/concurrency (10/10) — only the daily cap (1) is
    // tight, so a second call must be refused by the CHARGE, not the window.
    assert!(
        admit_embed(&limiter, "ollama", 10, 10, 1).is_ok(),
        "the first call must be admitted"
    );
    // `ConcurrencyGuard` (the `Ok` payload) is deliberately not `Debug`, so
    // `matches!` rather than `expect_err` avoids requiring that just to assert.
    assert!(
        matches!(
            admit_embed(&limiter, "ollama", 10, 10, 1),
            Err(AppError::RateLimited(_))
        ),
        "a second call past the exhausted daily cap must be refused"
    );
}

#[test]
fn admit_embed_does_not_charge_a_different_providers_budget() {
    let limiter = Arc::new(Limiter::new());
    // Exhaust "ollama"'s single daily slot...
    assert!(admit_embed(&limiter, "ollama", 10, 10, 1).is_ok());
    // ...a call against a DIFFERENT embedding provider must be unaffected —
    // pins that the charge keys on the resolved embedding provider, not a
    // shared/global counter.
    assert!(
        admit_embed(&limiter, "openai", 10, 10, 1).is_ok(),
        "a different provider's budget must be untouched"
    );
}

#[test]
fn admit_embed_admits_a_normal_call_under_the_production_caps() {
    let limiter = Arc::new(Limiter::new());
    assert!(admit_embed(
        &limiter,
        "ollama",
        crate::limits::AI_EMBED_RATE_MAX,
        crate::limits::AI_EMBED_CONCURRENCY_MAX,
        crate::limits::PROVIDER_DAILY_MAX,
    )
    .is_ok());
}

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
// `limits::test` and `ai_provider::mod`'s own unit tests.

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

// ── reembed_tests ─────────────────────────────────────────────────────
// `reembed_run_failed` is the pure decision `ai_reembed_all` uses to pick
// `job_fail` vs `job_complete`. The command itself needs a live
// `AppHandle` this crate has no test harness for (see the same note on
// `AnswerSearcher`), so the fix (a total-failure run must emit
// `job.failed`, never `job.completed`) is pinned here instead.

#[test]
fn every_document_failing_reports_failure() {
    assert!(reembed_run_failed(0, 5));
}

#[test]
fn any_success_is_not_a_failed_run() {
    assert!(!reembed_run_failed(1, 4));
    assert!(!reembed_run_failed(5, 0));
}

#[test]
fn zero_documents_is_not_a_failed_run() {
    // Nothing to embed — a `job.completed` with a 0/0 payload is correct,
    // not a failure.
    assert!(!reembed_run_failed(0, 0));
}

// ── embedding_base_url_tests ──────────────────────────────────────────
// `ai_set_embedding_config` needs a live `AppHandle` this crate has no
// test harness for (see the same note on `AnswerSearcher` above), so its
// validation logic is pinned via the extracted pure
// `scrub_and_validate_embedding_base_url` — mirrors
// `ai_provider::tests`'s `resolve_by_name_*` base_url tests, which cover
// the same rule on the sibling probe path. "Persisted" is covered at the
// store level (`DocumentStore::set_embedding_config`), same idiom as
// `documents::test`'s command-layer notes.

#[test]
fn rejects_the_cloud_metadata_ip_on_openai_compatible() {
    let err = scrub_and_validate_embedding_base_url(
        ProviderId::OpenAiCompatible,
        Some("http://169.254.169.254/latest/meta-data".to_string()),
    )
    .expect_err("the cloud-metadata IP literal must be rejected");
    assert!(matches!(err, AppError::Validation(_)));
}

#[test]
fn accepts_a_local_lm_studio_style_base_url() {
    // Sanity: the guard must not break the ordinary local-endpoint case
    // it exists to protect around (LM Studio / vLLM / Ollama).
    let url = scrub_and_validate_embedding_base_url(
        ProviderId::OpenAiCompatible,
        Some("http://localhost:1234/v1".to_string()),
    )
    .expect("a local endpoint must still be accepted");
    assert_eq!(url.as_deref(), Some("http://localhost:1234/v1"));
}

#[test]
fn accepted_local_base_url_round_trips_through_the_store() {
    // "Persisted", not just "accepted": the scrubbed/validated value must
    // survive a real `DocumentStore` write + read unchanged.
    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    let base_url = scrub_and_validate_embedding_base_url(
        ProviderId::OpenAiCompatible,
        Some("http://localhost:1234/v1".to_string()),
    )
    .unwrap();
    let cfg = EmbeddingConfig {
        provider: ProviderId::OpenAiCompatible.as_str().to_string(),
        model: "local-embed".to_string(),
        base_url,
    };
    store.set_embedding_config(&cfg).unwrap();
    assert_eq!(
        store.embedding_config().base_url.as_deref(),
        Some("http://localhost:1234/v1")
    );
}

#[test]
fn drops_base_url_for_a_non_openai_compatible_provider_instead_of_erroring() {
    // Mirrors `AiConfigStore::validate_settings`'s scrub: `base_url` is
    // inert for egress on every provider except `OpenAiCompatible`, so a
    // bogus value (here, one that WOULD fail validation if checked) must
    // be silently dropped, not surfaced as an error.
    let url = scrub_and_validate_embedding_base_url(
        ProviderId::Gemini,
        Some("http://169.254.169.254/latest/meta-data".to_string()),
    )
    .expect("a non-OpenAiCompatible provider must never error on base_url");
    assert_eq!(url, None);
}

// ── embed_job_guard_tests ─────────────────────────────────────────────
// Only ONE embedding job may run at a time: auto-indexing and the manual
// "Re-index now" button are independent triggers that write the same
// vectors for the same documents, so a concurrent pair bills a cloud
// provider twice for identical work. `running_embed_job` itself needs an
// `AppHandle` this crate has no harness for; the decision it makes per job
// record does not.

#[test]
fn both_embedding_job_kinds_count_while_they_are_still_going() {
    for kind in ["ai.reembed", "ai.indexStale"] {
        for status in [JobStatus::Running, JobStatus::Queued, JobStatus::Streaming] {
            assert!(
                is_active_embed_job(kind, &status),
                "{kind} in {status:?} must block a second embedding job"
            );
        }
    }
}

#[test]
fn a_finished_embedding_job_never_blocks_the_next_one() {
    // The differential. Without it the guard would latch after the first
    // run and auto-indexing would never fire again.
    for status in [
        JobStatus::Completed,
        JobStatus::Failed,
        JobStatus::Cancelled,
    ] {
        assert!(
            !is_active_embed_job("ai.reembed", &status),
            "a {status:?} job must not block the next index"
        );
    }
}

#[test]
fn unrelated_jobs_do_not_block_indexing() {
    // A generation or a scrape running is no reason to refuse to index.
    for kind in ["ai.generate", "scrape", "ai.reembed.not-really"] {
        assert!(!is_active_embed_job(kind, &JobStatus::Running), "{kind}");
    }
}
