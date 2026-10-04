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
//! command is reachable IN PRINCIPLE from the agent CLI or a crafted
//! extension message, neither of which ever sees the schema. `help_search`
//! itself is currently `NotExposed` on the agent-CLI policy table (issue
//! #1169), so these caps are defence in depth against a future
//! reclassification, not today's only guard.
//!
//! **Degrade, never silently claim more than ran.** The dense arm is gated on
//! the SAME `semantic_scoring` preference that gates hybrid postings search,
//! and that preference defaults to FALSE — a search surface must never spend
//! against a paid provider with no opt-in. Off → keyword-only,
//! `dense: "skipped"`. Any embedding failure → keyword-only,
//! `dense: "unavailable"`, never an error to the user. `mode` is `"hybrid"`
//! only when the dense arm actually RAN.
//!
//! **The question's own function words are dropped, per LANGUAGE.** The
//! lexical arm ORs the question's tokens (`QueryMode::Any`), so every "how",
//! "the", "ich", "und" is an extra branch matching most of the corpus. The
//! request carries the locale its ENTRIES are written in and [`stopwords`]
//! turns it into a drop list — a per-language table, not a `detect()` on the
//! question (the repo already records whatlang reading a short line as the
//! wrong language with no signal that anything went wrong). An unknown
//! locale drops nothing rather than falling back to English. Measured, both
//! languages, in `tests/help_retrieval.rs`.
//!
//! **Cancellable, through the same registry every job kind cancels through.**
//! A question is a single deliberate action with no supersede-on-keystroke
//! shape behind it, so v1 shipped without this — but the leg it left
//! uncancellable is the expensive one: a first question on a cloud embedder
//! against a cold cache runs to completion (up to [`HELP_EMBED_MISSES_MAX`]
//! entry embeds) even after the user presses Stop or navigates away. So the
//! request now carries an OPTIONAL caller-minted `queryId`
//! ([`QUERY_ID_PREFIX`]) that `help_search` registers against the app-wide
//! `jobs::cancel::CancelRegistry` BEFORE any async work, exactly as
//! `commands::hybrid_search` does; `jobs.cancel(queryId)` is the supersede
//! channel and there is no cancel command of its own. Omitting the id is one
//! code path, not two: the command then uses an unregistered token nobody can
//! fire.
//!
//! Which of the two a caller gets is a property of its REQUEST, not of who
//! it is — this stays true even though `help_search` is currently
//! `NotExposed` on the agent-CLI policy table (issue #1169). If it is ever
//! reclassified, an agent-CLI or extension-bridge caller would get exactly
//! what its own body asked for — send a `help-` id and `jobs_cancel` reaches
//! this search like any other, omit it and nothing can. There is no
//! renderer-only path in this code.
//!
//! A cancel makes the dense arm stop SOONER, not instantly: the token is
//! raced against each individual embed (so a cancel does not wait out the
//! provider's per-attempt timeout) and checked between entries, and the arm
//! then reports [`ArmStatus::Unavailable`] with the keyword results still
//! returned. It is deliberately NOT a distinct wire outcome the way
//! `hybrid_search`'s `SearchOutcome::Cancelled` is: the only cancelling
//! caller discards the reply by id, and `HelpSearchResult` has no `outcome`
//! field at all, so adding one would be a wire-shape change bought for
//! nobody. If a non-renderer caller ever needs to tell "cancelled" apart from
//! "the embedding provider was unreachable", that field is the upgrade path.
//!
//! The two bounds that stopped a runaway arm before cancellation existed are
//! unchanged and still the only ones an ID-less caller gets: the SAME
//! wall-clock budget postings search uses (`timeouts::DENSE_ARM_TIMEOUT`) and
//! [`HELP_EMBED_MISSES_MAX`] embeds per request. Hitting either means the arm
//! reports `unavailable` rather than a partly-ranked `hybrid`.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::commands::hybrid_search::{ArmStatus, CancelGuard};
use crate::error::{AppError, AppResult};
use crate::ipc_contracts::help::{HelpSearchRequest, HelpSearchRequestEntry};
use crate::jobs::cancel::CancelRegistry;
use crate::retrieval::fusion;
use crate::retrieval::lexical::{LexicalDoc, LexicalIndex};

mod dense_arm;
mod stopwords;

use dense_arm::run_dense;

/// Re-validated here even though `HelpSearchRequestSchema` already caps it —
/// see the module doc for why a Zod cap is not a boundary check.
const QUERY_MAX_CHARS: usize = 500;
/// Mirrors `HelpSearchRequestSchema.entries`'s cap — how many entries one
/// request may CARRY. It is not what bounds the request's spend or the
/// vector cache's growth: [`HELP_EMBED_MISSES_MAX`] is.
const ENTRIES_MAX: usize = 200;
/// How many cache-MISS embeds one request may make. Past it the remaining
/// entries stay lexical-only and the dense arm reports
/// [`ArmStatus::Unavailable`] (see [`dense_arm::run_dense_arm`]).
///
/// The request, not the shipped corpus, decides how many entries arrive
/// (`help_search` is reachable IN PRINCIPLE from the agent CLI and the
/// extension bridge with a hand-written body — currently `NotExposed` on
/// the agent-CLI policy table, issue #1169), so without this an
/// entry-cap-sized call could
/// charge 200 embeds AND write 200 permanent `help_vectors` rows — per call,
/// repeatable. 64 is comfortably above the ~51 entries the app ships, so no
/// real question is ever degraded by it, and comfortably below the entry cap.
///
/// "Comfortably above the entries the app ships" is the half of that claim
/// nothing in this crate could check — the corpus lives in the translation
/// bundles (module doc), and a corpus grown past this cap would degrade the
/// dense arm to `unavailable` on every cold-cache question with no test going
/// red. `pub` so `tests/help_retrieval.rs`, which already reads the REAL en
/// bundle, asserts the shipped corpus still fits under it.
pub const HELP_EMBED_MISSES_MAX: usize = 64;
// A cap at or above [`ENTRIES_MAX`] is not a cap at all — the loop could never
// reach it — so the ordering that makes it real is compile-time, not a comment.
const _: () = assert!(HELP_EMBED_MISSES_MAX < ENTRIES_MAX);
const ENTRY_ID_MAX_CHARS: usize = 64;
/// Required prefix on a caller-minted `queryId`.
///
/// Distinct from `commands::hybrid_search::QUERY_ID_PREFIX` (`"search-"`) for
/// the same reason that one is distinct from the Rust-minted `job-{uuid}`
/// ids: `CancelRegistry::register` is last-writer-wins and trusts every id it
/// is handed, so two features minting into one id space must not be able to
/// name each other's live searches. See that registry's `register` doc.
const QUERY_ID_PREFIX: &str = "help-";
/// Matches `HelpSearchRequestSchema.queryId`'s cap.
const QUERY_ID_MAX_CHARS: usize = 64;
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
    validate_query_id(req.query_id.as_deref())?;
    let limit = (req.limit as usize).clamp(1, MAX_LIMIT);

    // Registered BEFORE any async work, so a `jobs_cancel(queryId)` arriving
    // between here and the dense arm starting is never a no-op — the same
    // ordering (and the same shared registry) `scrape_hybrid_search` uses.
    // ONE code path for both callers: without a `queryId` this is simply a
    // token nobody holds a handle to.
    let token = CancellationToken::new();
    // The guard's `Drop` is what removes the slot on EVERY exit path — a
    // panic or a dropped future would skip a trailing `unregister().await`
    // and leak the slot for the life of the process (see `CancelGuard`).
    let _cancel_guard = match req.query_id.as_deref() {
        Some(id) => {
            let registry = app.state::<Arc<CancelRegistry>>().inner().clone();
            registry.register(id, token.clone()).await;
            Some(CancelGuard {
                registry,
                id: id.to_string(),
            })
        }
        None => None,
    };

    let lexical = run_lexical_arm(
        &req.entries,
        &query,
        req.entries.len(),
        req.locale.as_deref(),
    );
    let dense = if semantic_on(&app) {
        run_dense(&app, &query, &req.entries, &token).await
    } else {
        // Not "no embedding provider" — deliberately not attempted. The
        // reply says `skipped`, and the UI says so too.
        (Vec::new(), ArmStatus::Skipped)
    };

    let result = assemble(lexical, dense, limit);
    log_result(&result, req.entries.len());
    Ok(result)
}

/// The `queryId` boundary check, kept as its own pure function because it is
/// the one field whose validity is a SAFETY property rather than a size cap:
/// the id becomes a key in the app-wide `jobs::cancel::CancelRegistry`, which
/// is last-writer-wins and trusts whatever it is handed, so an id outside
/// [`QUERY_ID_PREFIX`] could name a live scrape's `job-{uuid}` slot (or a
/// postings search's `search-` one) and replace — then, on cleanup, delete —
/// that run's own token.
///
/// Re-checked here even though `HelpSearchRequestSchema` already constrains
/// it: an agent CLI or a crafted extension message could reach this command
/// directly and never see the Zod schema (`help_search` is currently
/// `NotExposed` on the agent-CLI policy table, issue #1169, so this is
/// defence in depth against a future reclassification). `None` is valid —
/// the id is optional, and omitting it means "not cancellable".
fn validate_query_id(query_id: Option<&str>) -> AppResult<()> {
    let Some(id) = query_id else {
        return Ok(());
    };
    if id.chars().count() > QUERY_ID_MAX_CHARS || !id.starts_with(QUERY_ID_PREFIX) {
        return Err(AppError::Validation(format!(
            "queryId must be at most {QUERY_ID_MAX_CHARS} chars and start with \"{QUERY_ID_PREFIX}\""
        )));
    }
    Ok(())
}

/// Boundary re-validation of the whole request, in one pure function so every
/// cap is a unit test rather than a claim. Mirrors `scrape_hybrid_search`'s
/// own refusals (`AppError::Validation`, message naming the cap).
///
/// `locale` is deliberately absent: a locale can never be a refusal (an
/// unknown one means "drop no function words", not an error), so its cap and
/// normalisation live with the table it selects —
/// [`stopwords::stopwords_for_locale`], which bounds the caller's string
/// before it allocates anything from it.
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
/// `hybrid_search::candidates::run_lexical_arm` makes, for the same reason: FTS5 can fail
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
/// `locale` is the request's own (the locale the ENTRIES are written in, not
/// a guess from the question — see [`stopwords`] for why a per-query
/// `detect()` is unsound at this length), and selects the function words to
/// drop from the question before the OR-join. An unknown locale drops
/// nothing.
///
/// `Option`, mirroring the wire: `HelpSearchRequestSchema.locale` is
/// optional, and `None` means the caller did not say — which is NOT English.
/// Taking the `Option` here rather than an already-defaulted `&str` puts that
/// decision in the one function that owns the drop list, so no call site can
/// quietly default it to `"en"` (the `unwrap_or("en")` this signature exists
/// to make impossible would have degraded a French corpus silently).
///
/// Pure (no app, no network), and `pub` for the same reason as
/// [`to_lexical_doc`]: `tests/help_retrieval.rs` measures THIS function,
/// locale routing included, over the real shipped bundles.
pub fn run_lexical_arm(
    entries: &[HelpSearchRequestEntry],
    query: &str,
    limit: usize,
    locale: Option<&str>,
) -> (Vec<String>, ArmStatus) {
    let docs: Vec<LexicalDoc<'_>> = entries.iter().map(to_lexical_doc).collect();
    // `unwrap_or_default()` — the empty string is a tag `stopwords_for_locale`
    // has no list for, i.e. exactly the "drop nothing" branch an unknown
    // locale takes. One branch, not two.
    let stopwords = stopwords::stopwords_for_locale(locale.unwrap_or_default());
    match LexicalIndex::build(&docs).and_then(|index| index.search_any(query, limit, stopwords)) {
        Ok(ranks) => (ranks, ArmStatus::Ran),
        Err(_) => (Vec::new(), ArmStatus::Unavailable),
    }
}

// ── Dense arm ────────────────────────────────────────────────────────────────

/// THE production gate for the dense arm — one named function with exactly
/// one call site, mirroring `hybrid_search::rerank_arm::should_rerank`'s reasoning, so
/// "semantic OFF makes zero embed calls" is a property of one `if`.
///
/// Missing state reads as OFF: the failure direction that spends nothing.
fn semantic_on(app: &AppHandle) -> bool {
    app.try_state::<crate::job_preferences::JobPreferencesStore>()
        .map(|s| s.semantic_scoring())
        .unwrap_or(false)
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
mod tests;
