//! `help:search` — rank the in-app help corpus against a user question,
//! using the SAME L1 `retrieval` primitives as `commands::hybrid_search`
//! (ADR-039): FTS5 lexical, cosine dense, RRF fusion. No new keyword or
//! fusion code exists anywhere for help search.
//!
//! **The corpus stays in the translation bundles.** The help entries live at
//! `support.faq.<section>Questions.<id>.{q,a}` in
//! `packages/translations/src/locales/{en,de}/translation.json` and are
//! rendered by the support page; Rust never reads those files (no
//! `include_str!` of a 182 KB bundle, no build step). The renderer sends the
//! entries of the ACTIVE locale with each question and this module does the
//! retrieval math, the embedding, the spend charge and the vector cache. The
//! entry text is app copy, but the REQUEST is still renderer-supplied input
//! crossing an IPC boundary, so every Zod cap is re-checked here — a Tauri
//! command is reachable directly by the agent CLI or a crafted extension
//! message, which never see the schema.
//!
//! **Degrade, never silently claim more than ran.** The dense arm is gated on
//! the SAME `semantic_scoring` preference that gates hybrid postings search,
//! and that preference defaults to FALSE — a search surface must never spend
//! against a paid provider with no opt-in. Off → keyword-only,
//! `dense: "skipped"`. Any embedding failure → keyword-only,
//! `dense: "unavailable"`, never an error to the user. `mode` is `"hybrid"`
//! only when the dense arm actually RAN.
//!
//! **Tradeoff recorded, not hidden: no cancellation in v1.** This mirrors
//! `hybrid_search::embed_or_cancel` MINUS its `CancellationToken` — a help
//! question is a single deliberate action with no supersede-on-keystroke
//! shape behind it, so there is no `queryId` and no `CancelRegistry`
//! registration. The cost is that a first question on a cloud embedder,
//! against a cold cache, runs to completion (≤ 51 entry embeds, once per
//! embedding space) even if the user navigates away. With no token to stop
//! it, the dense arm is bounded by two things instead: the SAME wall-clock
//! budget postings search uses (`timeouts::DENSE_ARM_TIMEOUT`) and
//! [`HELP_EMBED_MISSES_MAX`] embeds per request. Hitting either means the
//! arm reports `unavailable` rather than a partly-ranked `hybrid`.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::commands::ai_provider::EmbeddingVector;
use crate::commands::hybrid_search::{dense_pair, ArmStatus};
use crate::documents::{embed_with_config, sha256_hex, DocumentStore, Embedder, EmbeddingConfig};
use crate::error::{AppError, AppResult};
use crate::ipc_contracts::help::{HelpSearchRequest, HelpSearchRequestEntry};
use crate::observability::sanitize_reason;
use crate::retrieval::lexical::{LexicalDoc, LexicalIndex};
use crate::retrieval::{dense, fusion};

/// Re-validated here even though `HelpSearchRequestSchema` already caps it —
/// see the module doc for why a Zod cap is not a boundary check.
const QUERY_MAX_CHARS: usize = 500;
/// Mirrors `HelpSearchRequestSchema.entries`'s cap — how many entries one
/// request may CARRY. It is not what bounds the request's spend or the
/// vector cache's growth: [`HELP_EMBED_MISSES_MAX`] is.
const ENTRIES_MAX: usize = 200;
/// How many cache-MISS embeds one request may make. Past it the remaining
/// entries stay lexical-only and the dense arm reports
/// [`ArmStatus::Unavailable`] (see [`run_dense_arm`]).
///
/// The request, not the shipped corpus, decides how many entries arrive
/// (a `help_search` is reachable from the agent CLI and the extension bridge
/// with a hand-written body), so without this an entry-cap-sized call could
/// charge 200 embeds AND write 200 permanent `help_vectors` rows — per call,
/// repeatable. 64 is comfortably above the ~51 entries the app ships, so no
/// real question is ever degraded by it, and comfortably below the entry cap.
const HELP_EMBED_MISSES_MAX: usize = 64;
// A cap at or above [`ENTRIES_MAX`] is not a cap at all — the loop could never
// reach it — so the ordering that makes it real is compile-time, not a comment.
const _: () = assert!(HELP_EMBED_MISSES_MAX < ENTRIES_MAX);
const ENTRY_ID_MAX_CHARS: usize = 64;
const ENTRY_TITLE_MAX_CHARS: usize = 200;
const ENTRY_BODY_MAX_CHARS: usize = 2000;
/// Hard ceiling on `limit`, regardless of what the caller asks for. Clamped
/// rather than rejected, exactly like `scrape_hybrid_search`'s own `limit`.
const MAX_LIMIT: usize = 10;

// ── Wire response ────────────────────────────────────────────────────────────

/// Whether the reply was ranked by BOTH arms or by keywords alone.
///
/// Derived from the dense arm's [`ArmStatus`] in exactly one place
/// ([`mode_of`]) so the UI's "semantic ranking is off" notice can never
/// disagree with `arms.dense`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HelpSearchMode {
    Hybrid,
    Keyword,
}

/// Which arms ran. [`ArmStatus`] is REUSED from `commands::hybrid_search`
/// rather than redeclared: the two commands make the same three-way
/// ran/skipped/unavailable promise on the wire, and two enums that must
/// serialize identically forever is a drift waiting to happen. `lexical`
/// never carries `Skipped` — it always runs — which is why the TS side types
/// it as the narrower `'ran' | 'unavailable'`; see
/// `help_arm_statuses_serialize_as_the_wire_contract_tags` for the pin.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelpSearchArms {
    pub lexical: ArmStatus,
    pub dense: ArmStatus,
}

/// One ranked entry. Ids only — the renderer already holds the text it sent,
/// so no copy of the corpus crosses back.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelpSearchHit {
    pub id: String,
    /// The RRF fused score (`retrieval::fusion`), not a BM25 or cosine value
    /// — comparable only WITHIN one reply's own ordering.
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelpSearchResult {
    /// At most the request's `limit`, best first.
    pub results: Vec<HelpSearchHit>,
    pub mode: HelpSearchMode,
    pub arms: HelpSearchArms,
}

/// The one place a help search is logged — content-free by construction
/// (counts and enum tags only). A help question is user-authored free text
/// and the entry bodies are the whole corpus; neither belongs in a log file
/// that the diagnostics bundle ships verbatim.
fn log_result(result: &HelpSearchResult, entries_received: usize) {
    log::info!(
        "[help_search] entries={} results={} mode={:?} arms=(lexical={:?} dense={:?})",
        entries_received,
        result.results.len(),
        result.mode,
        result.arms.lexical,
        result.arms.dense
    );
}

// ── Command ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn help_search(app: AppHandle, req: HelpSearchRequest) -> AppResult<HelpSearchResult> {
    let query = req.query.trim().to_string();
    validate(&query, &req.entries)?;
    let limit = (req.limit as usize).clamp(1, MAX_LIMIT);

    let lexical = run_lexical_arm(&req.entries, &query, req.entries.len());
    let dense = if semantic_on(&app) {
        run_dense(&app, &query, &req.entries).await
    } else {
        // Not "no embedding provider" — deliberately not attempted. The
        // reply says `skipped`, and the UI says so too.
        (Vec::new(), ArmStatus::Skipped)
    };

    let result = assemble(lexical, dense, limit);
    log_result(&result, req.entries.len());
    Ok(result)
}

/// Boundary re-validation of the whole request, in one pure function so every
/// cap is a unit test rather than a claim. Mirrors `scrape_hybrid_search`'s
/// own refusals (`AppError::Validation`, message naming the cap).
fn validate(trimmed_query: &str, entries: &[HelpSearchRequestEntry]) -> AppResult<()> {
    if trimmed_query.is_empty() {
        return Err(AppError::Validation("query must not be empty".to_string()));
    }
    if trimmed_query.chars().count() > QUERY_MAX_CHARS {
        return Err(AppError::Validation(format!(
            "query too long (max {QUERY_MAX_CHARS} chars)"
        )));
    }
    if entries.is_empty() {
        return Err(AppError::Validation(
            "entries must not be empty".to_string(),
        ));
    }
    if entries.len() > ENTRIES_MAX {
        return Err(AppError::Validation(format!(
            "entries too long (max {ENTRIES_MAX})"
        )));
    }
    for entry in entries {
        let id_len = entry.id.chars().count();
        if id_len == 0 || id_len > ENTRY_ID_MAX_CHARS {
            return Err(AppError::Validation(format!(
                "entry id must be 1..={ENTRY_ID_MAX_CHARS} chars"
            )));
        }
        // The schema's own `^[A-Za-z0-9_.-]+$`. Ids are echoed straight back
        // to the caller, so an id that could not have come from a translation
        // leaf path is refused rather than round-tripped.
        if !entry
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        {
            return Err(AppError::Validation(
                "entry id must match [A-Za-z0-9_.-]+".to_string(),
            ));
        }
        let title_len = entry.title.chars().count();
        if title_len == 0 || title_len > ENTRY_TITLE_MAX_CHARS {
            return Err(AppError::Validation(format!(
                "entry title must be 1..={ENTRY_TITLE_MAX_CHARS} chars"
            )));
        }
        let body_len = entry.body.chars().count();
        if body_len == 0 || body_len > ENTRY_BODY_MAX_CHARS {
            return Err(AppError::Validation(format!(
                "entry body must be 1..={ENTRY_BODY_MAX_CHARS} chars"
            )));
        }
    }
    Ok(())
}

// ── Lexical arm ──────────────────────────────────────────────────────────────

/// How a help entry maps onto the four BM25 columns
/// (`retrieval::lexical::BM25_WEIGHTS`): the QUESTION is the `title` (weight
/// 3.0 — a help corpus's questions are written as the phrasings users search
/// for, so a question hit is the strongest topical signal available) and the
/// ANSWER is the `description` (weight 1.0). `company`/`location` are empty:
/// they are job-posting columns with no help-corpus counterpart, and FTS5
/// scores an empty column as a non-match rather than needing a schema of its
/// own.
///
/// `pub` so `tests/help_retrieval.rs` measures THIS function over the real
/// shipped bundle instead of a hand-mirrored copy of it (the mirror shape
/// PR #1091's review rejected in `tests/lexical_synonym_gaps.rs`).
pub fn to_lexical_doc(entry: &HelpSearchRequestEntry) -> LexicalDoc<'_> {
    LexicalDoc {
        id: &entry.id,
        title: &entry.title,
        company: "",
        location: "",
        description: &entry.body,
    }
}

/// Run the lexical arm end-to-end and collapse a build/search failure to
/// [`ArmStatus::Unavailable`] — the same one reporting decision
/// `hybrid_search::run_lexical_arm` makes, for the same reason: FTS5 can fail
/// for real (see `LexicalIndex::search`'s NUL-byte note) and "zero hits"
/// must not be reported for it.
///
/// **`search_any`, never `search`** — the one place this arm deliberately
/// differs from the postings one. `search`'s implicit AND requires EVERY
/// token of the query to appear in a document, which is right for a search
/// box (each word is a filter the user added) and wrong for a question: "How
/// do I export my resume as a PDF?" is a conjunction no help entry satisfies,
/// so the arm answered zero hits — and on a default install, where
/// `semantic_scoring` is off, this is the ONLY arm that runs. `search_any`
/// ORs the same quoted tokens instead, so `bm25()` ranks by how many of the
/// question's terms an entry matched (`retrieval::lexical::QueryMode`).
///
/// Pure (no app, no network), and `pub` for the same reason as
/// [`to_lexical_doc`].
pub fn run_lexical_arm(
    entries: &[HelpSearchRequestEntry],
    query: &str,
    limit: usize,
) -> (Vec<String>, ArmStatus) {
    let docs: Vec<LexicalDoc<'_>> = entries.iter().map(to_lexical_doc).collect();
    match LexicalIndex::build(&docs).and_then(|index| index.search_any(query, limit)) {
        Ok(ranks) => (ranks, ArmStatus::Ran),
        Err(_) => (Vec::new(), ArmStatus::Unavailable),
    }
}

// ── Dense arm ────────────────────────────────────────────────────────────────

/// THE production gate for the dense arm — one named function with exactly
/// one call site, mirroring `hybrid_search::should_rerank`'s reasoning, so
/// "semantic OFF makes zero embed calls" is a property of one `if`.
///
/// Missing state reads as OFF: the failure direction that spends nothing.
fn semantic_on(app: &AppHandle) -> bool {
    app.try_state::<crate::job_preferences::JobPreferencesStore>()
        .map(|s| s.semantic_scoring())
        .unwrap_or(false)
}

/// One embed round-trip on the caller's config snapshot, charged against the
/// active provider's daily ceiling.
///
/// Routed through `documents::embed_with_config` — never `embed`/`AppEmbedder`
/// (`documents::embed`), which independently RE-READS `embedding_config()` on
/// every call. `cfg` is read ONCE by [`run_dense`] and threaded through both
/// the charge closure (which provider's budget) and the dispatch (which
/// provider actually receives the request), so `ai_set_embedding_config`
/// landing mid-search can never charge one provider while dispatching to
/// another — the #1087 finding-2 shape.
///
/// Implements the crate's existing [`Embedder`] seam rather than being a bare
/// function, so [`run_dense_arm`] below takes no `AppHandle` and "how many
/// provider calls did this search make" is a plain unit test.
struct ChargedEmbedder<'a> {
    app: &'a AppHandle,
    limiter: Arc<crate::limits::Limiter>,
    cfg: &'a EmbeddingConfig,
}

#[async_trait]
impl Embedder for ChargedEmbedder<'_> {
    async fn embed_one(&self, text: &str) -> Option<EmbeddingVector> {
        let limiter = self.limiter.clone();
        let provider = self.cfg.provider.clone();
        let charge_fn =
            move || limiter.charge_provider_daily(&provider, crate::limits::PROVIDER_DAILY_MAX);
        let charge: &(dyn Fn() -> AppResult<()> + Send + Sync) = &charge_fn;
        // `embed_with_config` logs its own failure (sanitized); `.ok()` here
        // only discards the error into the degrade-to-keyword-only signal.
        embed_with_config(self.app, self.cfg, text, Some(charge))
            .await
            .ok()
    }
}

/// Resolve the app-side state the dense arm needs, then run it. The ONLY
/// `AppHandle`-touching part of the dense path — the cache/embed/rank logic
/// itself lives in [`run_dense_arm`], behind a store + an [`Embedder`].
///
/// Missing managed state is [`ArmStatus::Unavailable`], not a panic: this is
/// a degradable arm, and a `help_search` that 500s because the store was not
/// registered would be strictly worse than keyword-only results.
async fn run_dense(
    app: &AppHandle,
    query: &str,
    entries: &[HelpSearchRequestEntry],
) -> (Vec<String>, ArmStatus) {
    let (Some(store), Some(limiter)) = (
        app.try_state::<DocumentStore>(),
        app.try_state::<Arc<crate::limits::Limiter>>(),
    ) else {
        return (Vec::new(), ArmStatus::Unavailable);
    };
    // ONE read, shared by the charge closure and every dispatch below.
    let cfg: EmbeddingConfig = store.embedding_config();
    let embedder = ChargedEmbedder {
        app,
        limiter: limiter.inner().clone(),
        cfg: &cfg,
    };
    run_dense_arm(
        &store,
        &cfg,
        &embedder,
        query,
        entries,
        crate::commands::ai_provider::timeouts::DENSE_ARM_TIMEOUT,
    )
    .await
}

/// Embed the query, resolve every entry's vector (cache first), and rank by
/// cosine similarity.
///
/// The query is embedded on EVERY request — it is different text each time,
/// so there is nothing to cache. Entry bodies are cached by
/// `sha256_hex(body)` in `help_vectors`, so an unchanged answer costs at most
/// one embed per embedding space, ever.
///
/// **All-or-nothing, by design.** The arm reports [`ArmStatus::Ran`] only
/// when EVERY requested entry was paired with a comparable vector; anything
/// less — the wall-clock bound below firing, the miss budget running out, an
/// entry whose embed failed, a vector that came back in another embedding
/// space — returns [`ArmStatus::Unavailable`] with NO ranks. Reporting `Ran`
/// on a partial pool would put `mode: "hybrid"` on the wire for a reply the
/// dense arm only half-ranked, and keeping the partial ranks while reporting
/// `Unavailable` would be the same lie from the other side: the fused order
/// would still be part-semantic under a `keyword` label. The embeds are not
/// wasted — every one of them is cached, so the next question is warm.
///
/// Returns [`ArmStatus::Unavailable`] when the query embed fails too. In
/// every one of these cases the caller still returns the lexical results, so
/// an unreachable embedding provider degrades the search rather than failing
/// it.
///
/// Takes a store + an [`Embedder`] rather than an `AppHandle` so the cache
/// decisions (hit by hash, miss on changed text, miss on a changed embedding
/// space), the embed COUNT and both bounds are unit tests over a real
/// `DocumentStore`.
///
/// `budget` is production's `timeouts::DENSE_ARM_TIMEOUT`, passed in by
/// [`run_dense`] rather than read here so the bound itself is testable: a test
/// that had to wait out the real one would be a 100-second test nobody runs.
async fn run_dense_arm<E: Embedder + ?Sized>(
    store: &DocumentStore,
    active: &EmbeddingConfig,
    embedder: &E,
    query: &str,
    entries: &[HelpSearchRequestEntry],
    budget: std::time::Duration,
) -> (Vec<String>, ArmStatus) {
    // Started BEFORE the query embed, exactly like `hybrid_search`'s own dense
    // arm: a slow query embed eats into the SAME budget rather than getting a
    // separate one.
    let started = std::time::Instant::now();
    let Some(query_vector) = embedder.embed_one(query).await else {
        return (Vec::new(), ArmStatus::Unavailable);
    };
    let query_f32: Vec<f32> = query_vector.values.iter().map(|v| *v as f32).collect();

    let mut pairs: Vec<(String, Vec<f32>)> = Vec::with_capacity(entries.len());
    let mut misses = 0usize;
    for entry in entries {
        // The whole loop is bounded by ELAPSED time, the same constant and
        // the same shape `hybrid_search::run_dense_arm` uses (a
        // `tokio::time::timeout` around the loop would DROP the future and
        // throw away the vectors already cached inside it). There is no
        // cancellation token here — v1 has none (module doc) — so this is the
        // ONLY thing that stops a cold cache on a slow provider from running
        // `entries.len()` × the per-embed timeout.
        if started.elapsed() >= budget {
            break;
        }
        let hash = sha256_hex(&entry.body);
        let vector = match store.get_help_vector(&hash, active) {
            Some(cached) => Some(cached),
            None => {
                if misses >= HELP_EMBED_MISSES_MAX {
                    // Budget spent. Nothing further can restore `Ran` (this
                    // entry can no longer be paired), so there is nothing to
                    // gain by walking the rest of the list.
                    break;
                }
                misses += 1;
                let embedded = embedder.embed_one(&entry.body).await;
                if let Some(v) = &embedded {
                    // Best-effort cache write: the embed already succeeded,
                    // so a failed upsert must not fail the search — it only
                    // means the next question re-embeds (and re-charges) this
                    // entry. Logged rather than dropped, because a
                    // persistently failing write is otherwise invisible and
                    // reads downstream as "the cache never hits". Neither the
                    // hash nor the entry text is logged; the reason goes
                    // through the same sanitizer every other store-write
                    // warning uses (a rusqlite error can carry a path).
                    if let Err(e) = store.upsert_help_vector(&hash, v) {
                        log::warn!(
                            "[help_search] help vector not cached: {}",
                            sanitize_reason(&e.to_string())
                        );
                    }
                }
                embedded
            }
        };
        // `dense_pair` (shared with `hybrid_search`) is what keeps two
        // vectors from different embedding spaces from ever being scored
        // together — belt and braces with `get_help_vector`'s own space
        // check, and the only guard on a FRESH embed.
        if let Some(pair) = vector
            .as_ref()
            .and_then(|v| dense_pair(&entry.id, &query_vector.space, v))
        {
            pairs.push(pair);
        }
    }
    // The all-or-nothing rule (see this fn's doc). Also the ONE check that
    // covers every early `break` above: a loop that stopped short leaves
    // `pairs` short too, so neither bound needs its own reporting path.
    if pairs.len() < entries.len() {
        return (Vec::new(), ArmStatus::Unavailable);
    }
    (
        dense::rank_by_similarity(&query_f32, &pairs),
        ArmStatus::Ran,
    )
}

// ── Fusion + reply assembly ──────────────────────────────────────────────────

/// `"hybrid"` only when the dense arm actually RAN. Both `Skipped` (the
/// preference is off) and `Unavailable` (an embedding failure) are keyword
/// results, and saying otherwise would present a lexical list as hybrid.
fn mode_of(dense_status: ArmStatus) -> HelpSearchMode {
    match dense_status {
        ArmStatus::Ran => HelpSearchMode::Hybrid,
        ArmStatus::Skipped | ArmStatus::Unavailable => HelpSearchMode::Keyword,
    }
}

/// Fuse the two arms' rankings and build the wire reply. Pure, so "keyword
/// results still come back when the dense arm is unavailable", "`limit` is
/// honoured", and "`mode` follows the dense arm's real status" are unit tests
/// rather than claims.
///
/// An empty rank list is a no-op inside `reciprocal_rank_fusion`, so a
/// skipped/unavailable arm degrades the fusion to whichever arm DID run with
/// no special-casing here.
fn assemble(
    lexical: (Vec<String>, ArmStatus),
    dense_arm: (Vec<String>, ArmStatus),
    limit: usize,
) -> HelpSearchResult {
    let (lexical_ranks, lexical_status) = lexical;
    let (dense_ranks, dense_status) = dense_arm;
    let rank_lists = vec![lexical_ranks, dense_ranks];
    let results: Vec<HelpSearchHit> = fusion::reciprocal_rank_fusion(&rank_lists)
        .into_iter()
        .take(limit)
        .map(|(id, score)| HelpSearchHit { id, score })
        .collect();
    HelpSearchResult {
        results,
        mode: mode_of(dense_status),
        arms: HelpSearchArms {
            lexical: lexical_status,
            dense: dense_status,
        },
    }
}

#[cfg(test)]
mod test;
