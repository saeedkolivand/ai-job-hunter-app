use super::*;

// ── reembed_tests ─────────────────────────────────────────────────────
// `reembed_run_failed` is the pure decision `ai_reembed_all` uses to pick
// `job_fail` vs `job_complete`. The command itself needs a live
// `AppHandle` this crate has no test harness for (see the same note on
// `AnswerSearcher`), so the fix (a total-failure run must emit
// `job.failed`, never `job.completed`) is pinned here instead.

#[test]
fn only_a_run_where_every_document_failed_reports_failure() {
    assert!(reembed_run_failed(0, 5));
    // Any success is not a failed run.
    assert!(!reembed_run_failed(1, 4));
    assert!(!reembed_run_failed(5, 0));
    // Nothing to embed — a `job.completed` with a 0/0 payload is correct,
    // not a failure.
    assert!(!reembed_run_failed(0, 0));
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
