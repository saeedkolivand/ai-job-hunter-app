//! Embedding index status and (re)indexing — the status strip's read model, the
//! full rebuild, the stale-only auto-index and the single-flight guard they share.
//! Split out of `commands/ai/mod.rs` for R8 (issue #1280); `mod.rs` re-exports the
//! commands, so each keeps its `commands::ai::<name>` path. (`ai_set_embedding_config`
//! stays in `mod.rs`, next to the validation it calls.)

use parking_lot::Mutex;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::db::new_job_id;
use crate::documents::DocumentStore;
use crate::events::{emit_event, JobEvent, JOBS_EVENT};
use crate::jobs::{JobStatus, JobTracker};
use crate::observability::sanitize_reason;
use crate::postings::PostingsCache;

// ── Embeddings configuration & re-indexing ──────────────────────────────────────

/// The active embedding space, the vector counts per space, and how many
/// documents are indexed in the active space (vs. stale / unindexed).
#[tauri::command]
pub async fn ai_embedding_status(app: AppHandle) -> Value {
    let store = app.state::<DocumentStore>();
    let cfg = store.embedding_config();
    let total_docs = store.list().len();
    // SQL COUNT in the active space — never deserializes the vector blobs (the old
    // path loaded every vector via a full vector scan just to count the matching ones).
    let indexed_in_active = store.count_vectors_in_space(&cfg.provider, &cfg.model);
    let spaces: Vec<Value> = store
        .vector_space_counts()
        .into_iter()
        .map(|(s, n)| {
            json!({
                "provider": s.provider,
                "model": s.model,
                "dim": s.dim,
                "count": n,
                "active": cfg.provider == s.provider && cfg.model == s.model,
            })
        })
        .collect();
    json!({
        "active": { "provider": cfg.provider, "model": cfg.model, "baseUrl": cfg.base_url },
        "spaces": spaces,
        "documents": {
            "total": total_docs,
            "indexedInActiveSpace": indexed_in_active,
            "stale": total_docs.saturating_sub(indexed_in_active),
        },
        // Whether an embedding job is running right now (auto or manual). The
        // settings strip needs the real thing: inferring "indexing now" from the
        // auto-index PREFERENCE alone claims work is happening even when the run
        // already failed, or was never started because nothing changed.
        "indexing": running_embed_job(&app).is_some(),
    })
}

/// Whether a re-embed run should report failure (`job.failed`) rather than a
/// `job.completed` with a 0/N payload — true only when EVERY document failed
/// and there was at least one to embed. Pure + unit-tested so the bug this
/// fixes (a total failure used to still emit `job.completed`, leaving the
/// settings strip showing a stale success toast over an unchanged index)
/// can't silently regress. A run with zero documents (`failed == 0`) is not a
/// failure — there was nothing to fail at.
fn reembed_run_failed(done: u32, failed: u32) -> bool {
    done == 0 && failed > 0
}

/// Job kinds that embed documents. Both write the same vectors for the same
/// documents, so only ONE may run at a time.
const EMBED_JOB_KINDS: [&str; 2] = ["ai.reembed", "ai.indexStale"];

/// Claim the right to run an embedding job, or report the one already running.
///
/// Auto-indexing and the manual "Re-index now" button are independent triggers
/// with no knowledge of each other, so without this a background auto-index and
/// a user's button press embed the same documents concurrently — billing a cloud
/// provider twice for identical work. Enforced in the BACKEND rather than by
/// disabling the button, because that is the only place every trigger has to
/// pass through; a UI-only guard narrows the race instead of closing it.
///
/// The scan and the registration happen under ONE lock
/// ([`JobTracker::start_exclusive`]): checking first and starting after is
/// check-then-act, and two commands can both see "nothing running" before either
/// registers.
///
/// `None` means this caller owns the run; `Some(existing)` is the job to watch
/// instead — a normal outcome, not a failure, which is why this is not a
/// `Result` (see R6).
fn claim_embed_job(app: &AppHandle, job_id: &str, kind: &str) -> Option<String> {
    crate::commands::jobs::job_start_exclusive(app, job_id, kind, &EMBED_JOB_KINDS)
}

/// Whether an embedding job is running right now (for the status surface).
fn running_embed_job(app: &AppHandle) -> Option<String> {
    app.state::<Mutex<JobTracker>>()
        .lock()
        .list()
        .iter()
        .find(|j| is_active_embed_job(&j.kind, &j.status))
        .map(|j| j.id.clone())
}

/// Whether a job record is an embedding job that has NOT finished.
///
/// Split out purely so it is testable: the callers need an `AppHandle` this
/// crate has no harness for, this needs nothing. The terminal-status half is the
/// load-bearing part — counting a COMPLETED job as active would block every
/// future index permanently after the first run. Mirrors the predicate inside
/// [`JobTracker::start_exclusive`]; a test pins the two agreeing.
fn is_active_embed_job(kind: &str, status: &JobStatus) -> bool {
    EMBED_JOB_KINDS.contains(&kind)
        && matches!(
            status,
            JobStatus::Running | JobStatus::Queued | JobStatus::Streaming
        )
}

/// Documents with no usable vector in the ACTIVE embedding space — i.e. never
/// indexed, or indexed under a different provider/model/format.
///
/// The same `EmbeddingConfig::matches` predicate `match_resume` uses to decide
/// whether it can reuse a stored vector, so "stale" means exactly the same thing
/// to the indexer and to the consumer.
fn stale_documents(app: &AppHandle) -> Vec<crate::documents::DocumentRecord> {
    let store = app.state::<DocumentStore>();
    let cfg = store.embedding_config();
    store
        .list()
        .into_iter()
        .filter(|d| {
            !store
                .get_vector(&d.id)
                .is_some_and(|v| cfg.matches(&v.space))
        })
        .collect()
}

/// Embed `docs` with the active config and write the vectors, emitting
/// `jobs:event` progress. The shared body of [`ai_reembed_all`] (every document,
/// a full rebuild) and [`ai_index_stale_documents`] (only what is missing) so the
/// two can never drift in error handling, cancellation or progress reporting.
async fn run_embed_job(
    app: AppHandle,
    job_id: String,
    docs: Vec<crate::documents::DocumentRecord>,
) {
    let app_clone = app;
    let job_id_clone = job_id;
    {
        let total = docs.len();
        let mut done = 0u32;
        let mut failed = 0u32;
        // The FIRST embedding/write error, carried through to `job_fail` so a
        // total failure surfaces its real cause instead of dying in the log
        // (e.g. an Ollama context-length overflow or a retired Gemini model).
        let mut first_error: Option<String> = None;

        // Re-embed with bounded concurrency: each document is normally one HTTP
        // round-trip, though a document longer than the provider's per-chunk cap
        // now costs several (see `ai_provider::embed::embed_adaptive` — chunk-and-mean-
        // pool, bounded to at most `MAX_CHUNKS_PER_DOCUMENT` chunks). A small
        // fan-out here keeps the provider busy without overwhelming it (or
        // hammering a rate limit). Cancellation is honored between chunks; store
        // writes (sync) stay serialized to avoid lock contention.
        const REEMBED_CONCURRENCY: usize = 4;
        let mut was_cancelled = false;
        for chunk in docs.chunks(REEMBED_CONCURRENCY) {
            let cancelled = app_clone
                .state::<Mutex<JobTracker>>()
                .lock()
                .get(&job_id_clone)
                .map(|j| j.status == JobStatus::Cancelled)
                .unwrap_or(false);
            if cancelled {
                was_cancelled = true;
                break;
            }

            // Embed this chunk's documents concurrently, preserving order so each
            // result pairs with its document id.
            let embeds = futures::future::join_all(
                chunk
                    .iter()
                    .map(|doc| crate::documents::embed(&app_clone, &doc.text)),
            )
            .await;

            for (doc, ev) in chunk.iter().zip(embeds) {
                match ev {
                    Ok(ev) => {
                        let store = app_clone.state::<DocumentStore>();
                        match store
                            .upsert_vector(&doc.id, &ev)
                            .and_then(|_| store.set_indexed(&doc.id))
                        {
                            Ok(()) => done += 1,
                            Err(e) => {
                                log::warn!(
                                    "reembed write failed for {}: {}",
                                    doc.id,
                                    sanitize_reason(&e.to_string())
                                );
                                first_error.get_or_insert_with(|| e.to_string());
                                failed += 1;
                            }
                        }
                    }
                    Err(e) => {
                        first_error.get_or_insert_with(|| e.to_string());
                        failed += 1;
                    }
                }
            }

            emit_event(
                &app_clone,
                JOBS_EVENT,
                JobEvent {
                    r#type: "job.stream".to_string(),
                    job_id: job_id_clone.clone(),
                    data: Some(json!({ "done": done, "failed": failed, "total": total })),
                    ts: crate::db::now_ms() as i64,
                },
            );
        }

        // A user-cancelled job is already in Cancelled status; calling
        // job_complete would overwrite it with Completed. Bail with partial counts.
        if was_cancelled {
            return;
        }

        // Every document failed — this is a failure, not a "completed" run with
        // a 0/N count (the bug this branch fixes: the embed provider erroring
        // for every document used to still emit `job.completed`, so the
        // settings strip showed a stale "success" toast over an unchanged
        // 0/N index). Partial success still completes with the existing
        // `{reembedded, failed, total}` payload.
        if reembed_run_failed(done, failed) {
            crate::commands::jobs::job_fail(
                &app_clone,
                &job_id_clone,
                first_error.unwrap_or_else(|| "embedding failed for every document".to_string()),
            );
            return;
        }

        crate::commands::jobs::job_complete(
            &app_clone,
            &job_id_clone,
            json!({ "reembedded": done, "failed": failed, "total": total }),
        );
    }
}

/// Re-embed every document with the active embedding config, rebuilding the
/// vector index in the active space. Emits `jobs:event` progress and returns a
/// job id. Clears the live posting embedding cache so stale-space entries go too.
#[tauri::command]
pub async fn ai_reembed_all(app: AppHandle) -> Value {
    let job_id = new_job_id();
    // Already embedding (an auto-index run, or a double-click): hand back the
    // running job so the caller watches THAT instead of starting a paid duplicate.
    if let Some(existing) = claim_embed_job(&app, &job_id, "ai.reembed") {
        return json!({ "jobId": existing });
    }

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        // Drop stale live-posting embeddings so search re-embeds them.
        app_clone
            .state::<Mutex<PostingsCache>>()
            .lock()
            .clear_embeddings();

        // Snapshot documents up front so no store guard is held across awaits.
        let docs = app_clone.state::<DocumentStore>().list();
        run_embed_job(app_clone, job_id_clone, docs).await;
    });

    json!({ "jobId": job_id })
}

/// Index only the documents that have no usable vector in the active space.
///
/// The auto-index path (renderer preference `autoIndexOnUpload`): a newly
/// imported résumé, or every document after the embedding provider/model
/// changed. Deliberately NOT [`ai_reembed_all`] — that re-embeds every document
/// unconditionally, so using it here would re-bill a cloud embedding provider
/// for documents that are already correctly indexed every time one new file is
/// added.
///
/// Returns `{ "jobId": null }` when nothing is stale, so the caller can stay
/// silent instead of showing progress for a no-op run.
#[tauri::command]
pub async fn ai_index_stale_documents(app: AppHandle) -> Value {
    let docs = stale_documents(&app);
    if docs.is_empty() {
        return json!({ "jobId": Value::Null });
    }
    let job_id = new_job_id();
    // Same guard as `ai_reembed_all` — a manual re-index already covers every
    // stale document, so joining it is strictly better than racing it.
    if let Some(existing) = claim_embed_job(&app, &job_id, "ai.indexStale") {
        return json!({ "jobId": existing });
    }

    let job_id_clone = job_id.clone();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        run_embed_job(app_clone, job_id_clone, docs).await;
    });

    json!({ "jobId": job_id })
}

#[cfg(test)]
mod tests;
