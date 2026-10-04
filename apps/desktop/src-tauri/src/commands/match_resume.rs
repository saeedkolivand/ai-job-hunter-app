//! The match-score IPC commands. The scoring kernel, the per-surface entry points
//! and the cached-posting lookups live in submodules (R8, issue #1280) and are glob
//! re-exported here, so every item keeps its `commands::match_resume::<name>` path.

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::applications::{clamp_to_bytes, MAX_JOB_DESCRIPTION_BYTES};
use crate::documents::evidence::{rank_bullets, EvidenceBullet};
use crate::documents::{DocumentRecord, DocumentStore};
use crate::ipc_contracts::matching::{
    MatchResumeRequest, MatchTextRequest, ResumeTrimSuggestionsRequest,
};
use crate::ipc_contracts::resume::ResumeExtractTextRequest;
use crate::locale::LocaleProfile;

/// The hard-constraint pass — the non-negotiables (can I take this job at all?)
/// that a relevance score structurally cannot answer. A sibling module rather
/// than more of this file: it shares nothing with the scoring kernel by design,
/// and must not be able to reach it. See its module doc for the three rules it
/// holds to and for which constraints were refused for lack of candidate-side
/// data.
mod constraints;
mod entry_points;
mod posting;
mod score;

pub(crate) use entry_points::*;
pub(crate) use posting::*;
pub(crate) use score::*;

/// Map the `semantic_scoring_enabled` request flag to the `semantic_enabled`
/// cache-key column: only an explicit `Some(true)` enables semantic scoring
/// (`1`); `Some(false)` AND an omitted flag (`None`) default to keyword-only
/// (`0`), matching the app-wide default (`semanticScoring: false`) and the
/// renderer — so a caller that omits the flag (e.g. the agent match tool) never
/// silently runs embeddings. Single source of this bit so the cache key and the
/// skip-branch can't drift; unit-tested directly.
fn semantic_enabled_bit(flag: Option<bool>) -> i64 {
    if flag == Some(true) {
        1
    } else {
        0
    }
}

#[tauri::command]
pub async fn match_resume(app: AppHandle, req: MatchResumeRequest) -> Value {
    let store = app.state::<DocumentStore>();
    // INVARIANT (errors-never-cached): every error early-return MUST precede the
    // first `get_match_score`/`upsert_match_score` call. The resume-not-found
    // guard below returns before any cache access; `score_one`'s job-not-found
    // early-return likewise precedes its first cache call. So an error path can
    // never read or pollute the result cache. See
    // `errors_never_populate_match_scores_cache` in documents/test.rs, which
    // pins the store-level non-pollution half.
    let Some(resume) = store.get(&req.resume_id) else {
        return json!({ "error": format!("resume not found: {}", req.resume_id) });
    };

    // Parse the résumé's cached keywords ONCE (absent/corrupt → None → live
    // extraction fallback inside `score_one`).
    let resume_raw_keywords = parse_resume_keywords(&resume);
    let active = store.embedding_config();
    let semantic_enabled = semantic_enabled_bit(req.semantic_scoring_enabled);
    let (job_text, posting_facts) = job_text_and_posting_facts(&app, &req.job_id);

    let scored = score_one(
        &AppScoreIo(&app),
        &store,
        &resume,
        resume_raw_keywords.as_deref(),
        &active,
        &req.job_id,
        job_text,
        semantic_enabled,
        MatchSurface::JobsPage,
        None, // user-initiated: not charged against the unattended daily ceiling
    )
    .await;

    // Hard constraints, reported SEPARATELY and computed AFTER the kernel — the
    // verdict is a sibling field of the score, never an input to it. Deliberately
    // out here rather than inside `score_one`: `score_one`'s result is cached in
    // `match_scores` under a key composed of SCORING inputs only, so a verdict
    // frozen into that row would be served stale for the whole TTL the moment the
    // user edits their job preferences. Out here it is recomputed every call, and
    // a cache HIT gets a fresh verdict on a cached score. It also keeps the
    // Autopilot and extension entry points — which share the kernel but not this
    // command — byte-identical.
    constraints::attach(&app, &posting_facts, scored)
}

/// [`match_resume_text`]'s two structural preconditions, in the SAME order the
/// errors-never-cached invariant requires: resolve the résumé (a fixed error
/// object, mirroring [`match_resume`]'s own resume-not-found shape) BEFORE any
/// cache access, then clamp the job text to [`MAX_JOB_DESCRIPTION_BYTES`] — the
/// SAME cap [`resume_trim_suggestions`] already enforces on the identical kind
/// of text. The renderer's zod `.max()` is client-side only (serde enforces
/// nothing), and this is a new IPC surface a non-UI caller can reach directly
/// with unbounded scraper/user input, so the backend must cap it itself. Pure —
/// no `AppHandle` — so it is directly unit-testable against a real
/// `DocumentStore` without a Tauri runtime.
fn resolve_resume_and_text(
    store: &DocumentStore,
    resume_id: &str,
    job_text: String,
) -> Result<(DocumentRecord, String), Value> {
    let Some(resume) = store.get(resume_id) else {
        return Err(json!({ "error": format!("resume not found: {}", resume_id) }));
    };
    Ok((resume, clamp_to_bytes(job_text, MAX_JOB_DESCRIPTION_BYTES)))
}

/// Score a stored résumé against arbitrary job-ad TEXT — the Score tab's IPC
/// entry point. See [`score_resume_against_text`] for why this exists (no
/// `PostingsCache` id reaches `JobAdView`). Semantic scoring is gated on the
/// SAME request flag [`match_resume`] uses (`semanticScoringEnabled`,
/// defaulting to keyword-only when omitted — see [`semantic_enabled_bit`]),
/// not hardcoded off.
#[tauri::command]
pub async fn match_resume_text(app: AppHandle, req: MatchTextRequest) -> Value {
    let store = app.state::<DocumentStore>();
    let semantic_enabled = semantic_enabled_bit(req.semantic_scoring_enabled);
    let (resume, job_text) = match resolve_resume_and_text(&store, &req.resume_id, req.job_text) {
        Ok(pair) => pair,
        Err(err) => return err,
    };
    let resume_raw_keywords = parse_resume_keywords(&resume);
    let active = store.embedding_config();
    score_resume_against_text(
        &app,
        &store,
        &resume,
        resume_raw_keywords.as_deref(),
        &active,
        job_text,
        semantic_enabled,
    )
    .await
}

#[tauri::command]
pub async fn resume_extract_text(req: ResumeExtractTextRequest) -> Value {
    match crate::extraction::route(&req.name, &req.bytes) {
        Ok(r) => json!({ "text": r.text, "confidence": format!("{:?}", r.confidence) }),
        Err(crate::extraction::types::ExtractionError::ScannedPdfWithoutOcr) => {
            json!({ "error": "scanned_pdf", "message": "PDF appears to be scanned. Please upload a text-based PDF or DOCX." })
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ── Trim suggestions (advisory) ───────────────────────────────────────────────

/// One résumé bullet, scored by how much of THIS posting's vocabulary it carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrimCandidate {
    /// The bullet's markdown-stripped text, as the reader sees it.
    pub text: String,
    /// Readable (unstemmed) job keywords this line carries — may be empty.
    pub hits: Vec<String>,
    pub score: usize,
}

/// Wire shim: the ranking itself lives in `documents::evidence::rank_bullets`
/// (the same scorer now also feeds evidence extraction), and this narrows an
/// [`EvidenceBullet`] back to the three fields the `match:trimSuggestions`
/// payload has always carried.
///
/// `id` is dropped and `score` narrows from `f64` to `usize` deliberately: the
/// score is a hit COUNT (always a non-negative whole number), and `usize` is
/// what the existing TS `TrimCandidate` expects — a widened `1.0` would be a
/// silent wire change. Pinned by `trim_candidate_wire_shape_is_unchanged`.
impl From<EvidenceBullet> for TrimCandidate {
    fn from(bullet: EvidenceBullet) -> Self {
        Self {
            text: bullet.text,
            hits: bullet.hits,
            score: bullet.score as usize,
        }
    }
}

/// Advisory trim panel: which bullets are carrying the least weight for this
/// posting, and how long this market expects the document to be.
///
/// Read-only — it never edits the document. The renderer shows the ranking when
/// the rendered preview exceeds `maxPages`; the user does the cutting.
#[tauri::command]
pub async fn resume_trim_suggestions(req: ResumeTrimSuggestionsRequest) -> Value {
    // Bound the work before doing any of it. The request schema's `.max(200_000)`
    // is zod, i.e. renderer-side — serde enforces nothing, so an IPC caller that
    // isn't our own UI could otherwise hand language detection, stemming and the
    // résumé parser an unbounded string. Clamped rather than rejected, matching
    // `clamp_job_description`'s convention: an advisory panel ranking the first
    // 200 kB beats an error dialog.
    let resume_text = clamp_to_bytes(req.resume_text, MAX_JOB_DESCRIPTION_BYTES);
    let job_text = clamp_to_bytes(req.job_text, MAX_JOB_DESCRIPTION_BYTES);
    let profile = LocaleProfile::get(req.locale.as_deref().unwrap_or("en"));
    let lines: Vec<TrimCandidate> = rank_bullets(&resume_text, &job_text)
        .into_iter()
        .map(TrimCandidate::from)
        .collect();
    json!({
        "maxPages": profile.max_pages,
        "lines": lines,
    })
}

#[cfg(test)]
mod tests;
