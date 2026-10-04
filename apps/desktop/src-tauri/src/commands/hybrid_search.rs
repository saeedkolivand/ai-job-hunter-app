//! `scrape:hybridSearch` — the Tauri command wiring the L1 `retrieval`
//! module's lexical/dense/fusion/rerank primitives over the LIVE
//! `postings::PostingsCache`.
//!
//! **No persisted posting text** (ADR-scoped decision — see
//! `postings::PostingsCache`'s module doc): the FTS5 index is built
//! in-memory, fresh, from whatever slice of the live cache this search runs
//! over, and dropped with it. Dense embeddings ARE cached, but on
//! `PostingsCache` itself (`get_embedding`/`set_embedding`), not in a new
//! store — reviving a cache that already existed for exactly this purpose
//! rather than adding a parallel path.
//!
//! **Degrade, never silently claim more than ran.** `semantic_scoring`
//! defaults to FALSE, so a default install runs lexical-only — BOTH the
//! dense arm and the rerank step read the SAME `semantic_on` preference
//! (`should_rerank` gates the latter), because rerank reaches a provider
//! just as much as the dense arm does and a search box must never spend
//! against a paid provider with no opt-in. An embedding or rerank failure
//! degrades the SAME way. Either way the reply's `arms` says exactly which
//! of lexical/dense/rerank ran, so the UI can say "keyword results; semantic
//! ranking unavailable" instead of presenting a lexical list as hybrid.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::ipc_contracts::scrape::PostingsHybridSearchRequest;
use crate::jobs::cancel::CancelRegistry;
use crate::postings::PostingsCache;
use crate::retrieval::fusion;
use crate::retrieval::lexical::LexicalDoc;

mod candidates;
mod rerank_arm;

use candidates::{
    corpus_generation, eligible_subset, run_dense_arm, run_lexical_arm, to_lexical_doc, PostingRow,
};
use rerank_arm::maybe_rerank;

pub(crate) use candidates::dense_pair;

/// Required prefix on a renderer-minted `queryId`.
///
/// Every OTHER id sharing the `jobs::cancel::CancelRegistry` id space is
/// minted by RUST, all via `db::new_job_id` (`job-{uuid}`) — including
/// `resume_pipeline_run`, which ALSO mints a separate `run-{uuid}` for the
/// `pipeline_runs` row identity, but registers only its `job_id`, never that
/// one (see `jobs::cancel::CancelRegistry::register`'s own doc). This is the
/// one id the CALLER mints (it must exist before the search's promise
/// resolves, so it can be handed to a later `jobs.cancel` call) —
/// `CancelRegistry::register`'s "last writer wins needs no generation/handle"
/// safety argument rests on every id being freshly minted per invocation
/// with no way for two live registrations to share a key. A caller-chosen
/// `queryId` with no distinguishing prefix could instead NAME a live run's
/// own `job-<uuid>` id — replacing that run's cancellation token, and later
/// deleting ITS slot when this search's own cleanup runs. The prefix makes
/// the two id spaces disjoint by construction.
const QUERY_ID_PREFIX: &str = "search-";
/// Matches `PostingsHybridSearchRequestSchema.queryId`'s cap.
const QUERY_ID_MAX_CHARS: usize = 64;
/// Re-validated here even though `PostingsHybridSearchRequestSchema` already
/// caps it — a Tauri command is an IPC boundary a non-UI caller (the agent
/// CLI, a crafted extension message) can reach directly, bypassing the Zod
/// schema entirely.
const QUERY_MAX_CHARS: usize = 200;
/// Mirrors `PostingsHybridSearchRequestSchema.eligibleIds`'s cap — see that
/// schema's doc for why 2000 (well above any realistic multi-board live
/// cache).
const ELIGIBLE_IDS_MAX: usize = 2000;
/// `limit` when the request omits it.
const DEFAULT_LIMIT: usize = 20;
/// Hard ceiling on `limit`, regardless of what the caller asks for.
const MAX_LIMIT: usize = 50;

/// Per-candidate character budget when fencing a posting into the rerank
/// prompt (`prompt_fence::fenced`'s `cap`).
///
/// Deliberately much smaller than `prompt_fence::JOB_CAP` (8,000 — sized for
/// ONE full job description in a single-posting prompt): this prompt carries
/// up to [`RERANK_TOP_K`] candidates at once, so a per-item budget of
/// `JOB_CAP` would blow the whole prompt out to ~160,000 chars for no
/// accuracy gain — the model only has to judge RELATIVE relevance across the
/// batch, not deeply analyze any one posting. 600 chars covers a title,
/// company and a meaningful opening slice of the description (most job ads
/// front-load the role summary) for every candidate, keeping the aggregate
/// prompt at roughly `RERANK_TOP_K * 600` ≈ 12,000 chars.
pub(crate) const RERANK_ITEM_CHAR_BUDGET: usize = 600;

#[tauri::command]
pub async fn scrape_hybrid_search(
    app: AppHandle,
    req: PostingsHybridSearchRequest,
) -> AppResult<HybridSearchResult> {
    let query = req.query.trim().to_string();
    if query.is_empty() {
        return Err(AppError::Validation("query must not be empty".to_string()));
    }
    if query.chars().count() > QUERY_MAX_CHARS {
        return Err(AppError::Validation(format!(
            "query too long (max {QUERY_MAX_CHARS} chars)"
        )));
    }
    if req.query_id.chars().count() > QUERY_ID_MAX_CHARS
        || !req.query_id.starts_with(QUERY_ID_PREFIX)
    {
        return Err(AppError::Validation(format!(
            "queryId must be at most {QUERY_ID_MAX_CHARS} chars and start with \"{QUERY_ID_PREFIX}\""
        )));
    }
    if let Some(ids) = &req.eligible_ids {
        if ids.len() > ELIGIBLE_IDS_MAX {
            return Err(AppError::Validation(format!(
                "eligibleIds too long (max {ELIGIBLE_IDS_MAX})"
            )));
        }
    }
    let limit = (req.limit.unwrap_or(DEFAULT_LIMIT as u32) as usize).clamp(1, MAX_LIMIT);

    // F2 — register the cancellation token BEFORE any of the search's async
    // work, so a `jobs_cancel(queryId)` that arrives between this call and the
    // work starting is never a no-op. Same shared registry every job kind
    // dispatches through (`commands::scrape::scrape_boards`'s identical
    // pattern) — there is no separate cancel command for this one.
    let cancels = app.state::<Arc<CancelRegistry>>().inner().clone();
    let token = CancellationToken::new();
    cancels.register(&req.query_id, token.clone()).await;
    // RAII, not a plain `.await` after the search: a panic inside `run_search`
    // (unwind) or the containing future being DROPPED (the async runtime
    // tearing down mid-search) would otherwise skip `unregister` entirely and
    // leak the slot in `CancelRegistry` for the life of the process — Drop
    // always runs on both of those paths, a bare statement after an `.await`
    // does not.
    let _cancel_guard = CancelGuard {
        registry: cancels,
        id: req.query_id.clone(),
    };
    run_search(&app, &req, &query, limit, &token).await
}

/// See `_cancel_guard`'s call-site comment. `CancelRegistry::unregister` is
/// async (it takes a `tokio::sync::Mutex`), so the sync `Drop` below can't
/// call it directly — it spawns a short detached task instead. That means
/// the slot's actual removal lands slightly AFTER this guard drops rather
/// than before the command's promise resolves (unlike the old synchronous
/// `.await`); harmless, since the only consequence of the slot still being
/// visible for that brief window is that a `jobs_cancel` racing the tail end
/// of an already-finished search cancels a token nobody is listening to
/// anymore.
///
/// Shared with `commands::help`, which registers the same way, rather than
/// copied there — two RAII guards over one registry is a drift waiting to
/// happen, and the reasoning above would then live in only one of them. NOT
/// hoisted into `jobs::cancel` (its natural home by name) because that module
/// is deliberately L1 and Tauri-free and this `Drop` needs
/// `tauri::async_runtime::spawn`; `commands::help` already reuses this
/// module's [`ArmStatus`] and `dense_pair` for the same "one definition"
/// reason.
pub(crate) struct CancelGuard {
    pub(crate) registry: Arc<CancelRegistry>,
    pub(crate) id: String,
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        let registry = self.registry.clone();
        let id = std::mem::take(&mut self.id);
        tauri::async_runtime::spawn(async move {
            registry.unregister(&id).await;
        });
    }
}

// ── Wire response ────────────────────────────────────────────────────────────

/// Whether one arm of the search actually ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArmStatus {
    Ran,
    /// Not attempted — gated off by a preference, or nothing left to do.
    Skipped,
    /// Attempted and failed (no embedding provider reachable, a rate limit,
    /// cancellation mid-arm).
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchArms {
    pub lexical: ArmStatus,
    pub dense: ArmStatus,
    pub rerank: ArmStatus,
}

/// Why the search stopped short of returning ranked results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchOutcome {
    Ok,
    /// Superseded by a later search sharing the same query id
    /// (`jobs_cancel`), or superseded before any work started.
    Cancelled,
    /// The live postings cache was cleared (a replace-scrape's first
    /// streamed item) while this search was still running — see
    /// `PostingsCache::generation`. `hits` is empty rather than describing
    /// postings that may no longer exist.
    StaleCorpus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HybridSearchResult {
    pub outcome: SearchOutcome,
    /// Ranked posting ids, best first, already limited to the request's
    /// `limit`. Always empty unless `outcome == "ok"`.
    pub hits: Vec<String>,
    pub arms: SearchArms,
    /// How many postings this search actually ranked over (the eligible
    /// subset, or the whole live cache when no allowlist was supplied).
    pub corpus_size: usize,
}

/// The one place a search result is logged — content-free by construction
/// (counts and enum tags only, never the query or any posting text; a search
/// query is user-authored free text that may carry a company, location or
/// person's name). Every return path in [`run_search`] funnels its result
/// through here.
fn log_result(result: &HybridSearchResult) {
    log::info!(
        "[hybrid_search] outcome={:?} corpus_size={} hits={} arms=(lexical={:?} dense={:?} rerank={:?})",
        result.outcome,
        result.corpus_size,
        result.hits.len(),
        result.arms.lexical,
        result.arms.dense,
        result.arms.rerank
    );
}

fn degraded(
    outcome: SearchOutcome,
    arms: SearchArms,
    corpus_size: usize,
) -> AppResult<HybridSearchResult> {
    let result = HybridSearchResult {
        outcome,
        hits: Vec::new(),
        arms,
        corpus_size,
    };
    log_result(&result);
    Ok(result)
}

// ── Orchestration ────────────────────────────────────────────────────────────

async fn run_search(
    app: &AppHandle,
    req: &PostingsHybridSearchRequest,
    query: &str,
    limit: usize,
    token: &CancellationToken,
) -> AppResult<HybridSearchResult> {
    let (items, generation0) = {
        let guard = app.state::<Mutex<PostingsCache>>();
        let guard = guard.lock();
        (guard.get_all().to_vec(), guard.generation())
    };
    let eligible = eligible_subset(&items, req.eligible_ids.as_deref());
    let corpus_size = eligible.len();

    let none_ran = SearchArms {
        lexical: ArmStatus::Skipped,
        dense: ArmStatus::Skipped,
        rerank: ArmStatus::Skipped,
    };
    if corpus_size == 0 {
        let outcome = if token.is_cancelled() {
            SearchOutcome::Cancelled
        } else {
            SearchOutcome::Ok
        };
        return degraded(outcome, none_ran, 0);
    }
    if token.is_cancelled() {
        return degraded(SearchOutcome::Cancelled, none_ran, corpus_size);
    }

    // ── Lexical ──────────────────────────────────────────────────────────────
    let lexical_docs: Vec<LexicalDoc<'_>> = eligible.iter().map(to_lexical_doc).collect();
    let (lexical_ranks, lexical_status) = run_lexical_arm(&lexical_docs, query, corpus_size);

    if token.is_cancelled() {
        return degraded(
            SearchOutcome::Cancelled,
            SearchArms {
                lexical: lexical_status,
                dense: ArmStatus::Skipped,
                rerank: ArmStatus::Skipped,
            },
            corpus_size,
        );
    }

    // ── Dense (gated on the persisted preference) ───────────────────────────
    let eligible_by_id: HashMap<&str, &PostingRow> =
        eligible.iter().map(|row| (row.id.as_str(), row)).collect();
    let semantic_on = app
        .try_state::<crate::job_preferences::JobPreferencesStore>()
        .map(|s| s.semantic_scoring())
        .unwrap_or(false);
    let (dense_ranks, dense_status) = if semantic_on {
        run_dense_arm(
            app,
            query,
            &eligible,
            &eligible_by_id,
            &lexical_ranks,
            token,
            generation0,
        )
        .await
    } else {
        (Vec::new(), ArmStatus::Skipped)
    };

    if token.is_cancelled() {
        return degraded(
            SearchOutcome::Cancelled,
            SearchArms {
                lexical: lexical_status,
                dense: dense_status,
                rerank: ArmStatus::Skipped,
            },
            corpus_size,
        );
    }

    // ── Fuse ─────────────────────────────────────────────────────────────────
    let mut rank_lists = vec![lexical_ranks];
    if !dense_ranks.is_empty() {
        rank_lists.push(dense_ranks);
    }
    let fused: Vec<String> = fusion::reciprocal_rank_fusion(&rank_lists)
        .into_iter()
        .map(|(id, _)| id)
        .collect();

    // ── Rerank (top RERANK_TOP_K of the fused order, gated on the SAME
    // preference as the dense arm — see `should_rerank`) ────────────────────
    let (final_order, rerank_status) =
        maybe_rerank(app, query, &fused, &eligible_by_id, token, semantic_on).await;

    let arms = SearchArms {
        lexical: lexical_status,
        dense: dense_status,
        rerank: rerank_status,
    };
    if token.is_cancelled() {
        return degraded(SearchOutcome::Cancelled, arms, corpus_size);
    }
    if corpus_generation(app) != generation0 {
        return degraded(SearchOutcome::StaleCorpus, arms, corpus_size);
    }

    let result = HybridSearchResult {
        outcome: SearchOutcome::Ok,
        hits: final_order.into_iter().take(limit).collect(),
        arms,
        corpus_size,
    };
    log_result(&result);
    Ok(result)
}

#[cfg(test)]
mod tests;
