use std::collections::HashSet;

use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::applications::{clamp_to_bytes, MAX_JOB_DESCRIPTION_BYTES};
use crate::commands::ai_provider::{EmbeddingVector, EMBEDDING_VECTOR_VERSION};
use crate::documents::evidence::{rank_bullets, EvidenceBullet};
use crate::documents::keywords::{
    apply_stemmer, display_forms, keyword_coverage, keywords, keywords_normalized, languages_align,
    make_stemmer, readable_gaps,
};
use crate::documents::{
    embed_charged, posting_vector_or_embed, sha256_hex, AppEmbedder, DocumentRecord, DocumentStore,
    EmbedBudget, Embedder, EmbeddingConfig, MatchScoreKey,
};
use crate::ipc_contracts::matching::{MatchResumeRequest, ResumeTrimSuggestionsRequest};
use crate::ipc_contracts::resume::ResumeExtractTextRequest;
use crate::locale::LocaleProfile;
use crate::postings::PostingsCache;

/// Score a resume against a job posting.
///
/// Returns a `MatchScore` (see packages/shared types): a semantic score from
/// embedding cosine similarity, an ATS score from job-keyword coverage, a
/// weighted `combined` score, the missing keywords (`gaps`), and short
/// recommendations. Degrades gracefully to keyword-only when Ollama is offline.
/// Cache-busting version for the match_scores result cache. Bump whenever the
/// 0.6/0.4 weighting, the combined-score formula, or the keyword/stemmer logic
/// changes — any of which would make a previously-cached score stale. Was
/// bumped to 2 alongside the v1->v2 `EMBEDDING_VECTOR_VERSION` bump (naive
/// single truncation → chunk-and-mean-pool). A vector-FORMAT change no longer
/// needs a coincidental bump here to invalidate: [`MatchScoreKey::vector_version`]
/// carries `EMBEDDING_VECTOR_VERSION` directly, so that axis self-invalidates
/// on its own (a cached score computed against an OLD-format vector is a miss
/// once the vector itself can be a new-format one under the identical space
/// tag) — this constant is now purely about the scoring FORMULA.
const MATCH_FORMULA_VERSION: i64 = 2;

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

/// Which user-facing surface is asking for a score.
///
/// The variants differ ONLY in **cache identity / where the résumé vector
/// lives** — never in pre-processing. Every surface that renders its number
/// under the app's "Match %" label (the Jobs page AND the headless Autopilot
/// re-rank) runs the SAME pipeline, because the pre-processing comes with the
/// label: [`crate::commands::translation::translate_if_needed`] rewrites the JD
/// into the résumé language BEFORE both keyword extraction and the embed, so
/// skipping it on one of them flips [`languages_align`] for a cross-language
/// pair — collapsing coverage to language-neutral tech tokens and embedding a
/// cross-lingual cosine. The same job would then show two materially different
/// percentages on two screens.
///
/// [`MatchSurface::Extension`] is the ONE deliberate exception: it never shows
/// a combined number, and its zero-egress guarantee has to be structural (no
/// flag to flip) — see [`score_adhoc_keyword_only`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MatchSurface {
    /// The in-app [`match_resume`] command (the Jobs page and everything routed
    /// through it).
    JobsPage,
    /// The headless Autopilot phase-2 semantic re-rank.
    Autopilot,
    /// The browser extension's ad-hoc, keyword-only "Check fit".
    Extension,
}

/// Where [`score_one`] reads/writes the RÉSUMÉ-side embedding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResumeVectorHome {
    /// The `vectors` table — the DOCUMENT index. Only a résumé that has a real
    /// `documents` row belongs here: that index is what the Embeddings panel
    /// counts (`count_vectors_in_space`) and what document delete / re-embed
    /// maintain, and both iterate real documents.
    DocumentIndex,
    /// The TTL-pruned `posting_vectors` cache. For a résumé SNAPSHOT (Autopilot
    /// stores résumé text, not a document reference): it still caches across a
    /// run and across repeat runs, but it is bounded by the same TTL/row-cap
    /// discipline as every other derived cache and can never be mistaken for an
    /// indexed document.
    EphemeralCache,
}

impl MatchSurface {
    /// Whether [`score_one`] runs the optional local-only translation step.
    ///
    /// TRUE for every "Match %" surface — see the type doc. Flipping this off
    /// for one of them is the metric-label divergence
    /// `every_match_percent_surface_runs_the_same_pre_processing` pins.
    pub(crate) fn translates(self) -> bool {
        !matches!(self, Self::Extension)
    }

    /// Where this surface's résumé embedding is cached — see
    /// [`ResumeVectorHome`].
    pub(crate) fn resume_vector_home(self) -> ResumeVectorHome {
        match self {
            Self::Autopilot => ResumeVectorHome::EphemeralCache,
            Self::JobsPage | Self::Extension => ResumeVectorHome::DocumentIndex,
        }
    }
}

/// The scoring kernel's outside world: the local-only JD translation and the
/// embedding round-trip, behind ONE seam.
///
/// [`score_one`] needs nothing else from the `AppHandle`, so this makes the
/// whole kernel — translation, cache identity, the charge, the degrade — a
/// plain unit test over a real [`DocumentStore`]. That matters here
/// specifically: the two effects are ordered (translate, THEN hash + embed the
/// TRANSLATED bytes), and an untestable kernel is how a budget predicate came
/// to hash the pre-translation blob.
#[async_trait::async_trait]
pub(crate) trait ScoreIo: Embedder {
    /// Rewrite the JD into `target_lang` when a local provider can (cloud
    /// providers are excluded upstream); returns `text` unchanged otherwise.
    async fn translate(&self, job_id: &str, text: String, target_lang: &str) -> String;
}

/// Production [`ScoreIo`]: the real translation command + the real embedder.
pub(crate) struct AppScoreIo<'a>(pub &'a AppHandle);

#[async_trait::async_trait]
impl Embedder for AppScoreIo<'_> {
    async fn embed_one(&self, text: &str) -> Option<EmbeddingVector> {
        AppEmbedder(self.0).embed_one(text).await
    }
}

#[async_trait::async_trait]
impl ScoreIo for AppScoreIo<'_> {
    async fn translate(&self, job_id: &str, text: String, target_lang: &str) -> String {
        crate::commands::translation::translate_if_needed(self.0, job_id, &text, target_lang).await
    }
}

/// The résumé language `score_one` matches in: the PERSISTED
/// `DocumentRecord.locale` (nullable — `documents_add`'s `locale` is optional),
/// falling back to `"en"`.
///
/// One function, so the translation target and the [`languages_align`] check
/// can never resolve a résumé to two different languages, and so every entry
/// point resolves it from the same source (an Autopilot résumé snapshot carries
/// no persisted locale, so it lands on the same `"en"` fallback the Jobs page
/// uses for a locale-less document).
pub(crate) fn resume_target_lang(resume: &DocumentRecord) -> &str {
    resume.locale.as_deref().unwrap_or("en")
}

/// Score a single resume against one job posting, returning a `MatchScore`
/// JSON value (or a `{ "error": … }` object when the job isn't cached).
///
/// The per-job kernel behind [`match_resume`]. `resume_raw_keywords` is the
/// parsed `keywords_json` (parsed ONCE by the caller); `None` — absent or corrupt
/// JSON — falls back to live extraction from `resume.text`, preserving the legacy
/// behaviour. `active` is the embedding config and `semantic_enabled` the
/// already-derived cache-key bit, both hoisted by the caller.
///
/// `job_text` is the posting blob resolved by the caller via [`job_text_for`]
/// (`None` → the posting wasn't in the live cache → job-not-found error).
///
/// Errors-never-cached invariant: the only error return (job-not-found) happens
/// before any `get_match_score`/`upsert_match_score`, so an error path can never
/// read or pollute the result cache.
///
/// `surface` carries the two per-entry-point decisions: whether the optional
/// local-only translation step runs ([`MatchSurface::translates`] — on for every
/// "Match %" surface; off for the extension, whose zero-egress guarantee means
/// the call is skipped ENTIRELY, not just short-circuited), and where the
/// résumé vector is cached ([`MatchSurface::resume_vector_home`]).
///
/// `budget`, when present, is charged **once per actual embedding round-trip**
/// this call makes — résumé and posting counted separately, nothing charged for
/// a cache hit. It is threaded down to the call rather than evaluated by the
/// caller because only here are the exact bytes known: the posting embed
/// consumes the POST-translation text, and whether the résumé side embeds at
/// all depends on a second cache this function owns. `None` for the interactive
/// surfaces, which are user-initiated and not budgeted.
#[allow(clippy::too_many_arguments)] // house convention (see clippy.toml threshold=8) — this fn legitimately threads every cache-key input plus the surface
async fn score_one(
    io: &dyn ScoreIo,
    store: &DocumentStore,
    resume: &DocumentRecord,
    resume_raw_keywords: Option<&[String]>,
    active: &EmbeddingConfig,
    job_id: &str,
    job_text: Option<String>,
    semantic_enabled: i64,
    surface: MatchSurface,
    budget: Option<&dyn EmbedBudget>,
) -> Value {
    let Some(job_text) = job_text else {
        return json!({ "error": format!("job not found in cache: {}", job_id) });
    };

    // Optional, local-only translation: when the JD language differs from the
    // resume locale and a local provider is configured, translate before keyword
    // extraction (and embedding) so matching happens in the resume language.
    // Always falls back to the original text on any failure. Cloud providers are
    // excluded, so this never incurs an unexpected API cost. Skipped entirely
    // (no call at all, not just a no-op) when the surface does not translate.
    let job_text = if surface.translates() {
        io.translate(job_id, job_text, resume_target_lang(resume))
            .await
    } else {
        job_text
    };

    // `semantic_enabled` is the cache-key bit; `skip_semantic` is its inverse.
    let skip_semantic = semantic_enabled == 0;

    // Self-invalidating result cache: the key captures every input that can
    // change the score (ids, embedding space, semantic on/off, formula version,
    // embedding vector version, and a hash of the final job text). A hit skips
    // embedding + cosine + keyword work entirely. The job-not-found error above
    // is returned before this point and is never cached.
    let job_text_hash = sha256_hex(&job_text);
    let cache_key = MatchScoreKey {
        resume_id: &resume.id,
        job_id,
        provider: &active.provider,
        model: &active.model,
        semantic_enabled,
        formula_version: MATCH_FORMULA_VERSION,
        vector_version: EMBEDDING_VECTOR_VERSION,
        job_text_hash: &job_text_hash,
    };
    if let Some(cached) = store.get_match_score_async(cache_key.to_owned_key()).await {
        return cached;
    }
    let (resume_vec, job_vec) = if skip_semantic {
        (None, None)
    } else {
        let rv = match surface.resume_vector_home() {
            ResumeVectorHome::DocumentIndex => match store.get_vector_async(&resume.id).await {
                Some(v) if active.matches(&v.space) => Some(v),
                _ => {
                    // A real round-trip, so it goes through the same charged
                    // choke point as every other embed here. The embedder logs
                    // its own failure; this caller keeps its existing "degrade
                    // to keyword-only" contract for match scoring.
                    let v = embed_charged(io, budget, &resume.text).await;
                    if let Some(ref ev) = v {
                        let _ = store.upsert_vector_async(&resume.id, ev).await;
                    }
                    v
                }
            },
            // A résumé SNAPSHOT has no `documents` row, so its vector must not
            // enter the document index — it would be counted as an indexed
            // document forever (nothing deletes it: document delete/re-embed
            // iterate real documents, `prune_caches` only touches
            // posting_vectors/match_scores). The posting-vector cache is the
            // right home: same space + text-hash guard, plus a TTL and a row
            // cap. Reuse is unchanged — the first job of a run embeds the
            // résumé, every later job (and every repeat run inside the TTL)
            // hits this row.
            ResumeVectorHome::EphemeralCache => {
                posting_vector_or_embed(store, active, io, budget, &resume.id, &resume.text).await
            }
        };
        // The posting embed consumes the POST-translation text — which is what
        // its cache row is keyed on, and therefore what the charge above is
        // decided on.
        let jv = posting_vector_or_embed(store, active, io, budget, job_id, &job_text).await;
        (rv, jv)
    };
    let semantic = match (&resume_vec, &job_vec) {
        (Some(a), Some(b)) => crate::commands::ai_provider::compare(a, b)
            .map(|s| (s.clamp(0.0, 1.0) * 100.0).round())
            .unwrap_or(0.0),
        _ => 0.0, // embeddings unavailable or disabled.
    };

    // ATS: how many job keywords appear in the resume text. The JD language
    // defines the stemmer; both sides are stemmed with the SAME stemmer when the
    // languages match (or translation ran). When they diverge, BOTH sides stay
    // unstemmed (normalized only) so intersection is symmetric — stemming only
    // one side would mangle tech tokens that survive in their raw form (e.g.
    // `docker`, `kubernetes`) and produce WORSE matches than no stemming at all.
    let stemmer = make_stemmer(&job_text);

    // Re-detect the JD language after translate_if_needed (translation may have
    // changed the text language). The decision itself lives in the keyword
    // kernel — `rank_trim_candidates` below routes through the same function, so
    // the trim panel and this score can't disagree on a cross-language pair.
    let jd_matches_resume_locale = languages_align(&job_text, resume_target_lang(resume));

    // Symmetric treatment: stem BOTH sides with the JD stemmer when languages
    // match; leave BOTH sides normalized-only (unstemmed) when they diverge.
    // Mixing stemmed-JD vs unstemmed-résumé would cause language-neutral tokens
    // like `docker` / `kubernetes` to be mutated on one side only and match
    // neither set — strictly worse than the unstemmed symmetric baseline.
    let job_keywords: HashSet<String> = if jd_matches_resume_locale {
        keywords(&job_text, &stemmer)
    } else {
        keywords_normalized(&job_text)
    };
    let resume_words: HashSet<String> = match resume_raw_keywords {
        Some(tokens) => {
            let token_set: HashSet<String> = tokens.iter().cloned().collect();
            if jd_matches_resume_locale {
                apply_stemmer(token_set, &stemmer)
            } else {
                token_set // normalized-only: symmetric with the JD side above
            }
        }
        None => {
            if jd_matches_resume_locale {
                keywords(&resume.text, &stemmer)
            } else {
                // Live extraction without stemming — symmetric with JD side.
                keywords_normalized(&resume.text)
            }
        }
    };

    // keyword_coverage returns None when the JD has no extractable keywords
    // (sparse posting) — distinguish from a genuine 0% match.
    let (ats, gap_stems, no_jd_keywords) = match keyword_coverage(&job_keywords, &resume_words) {
        Some((a, g)) => (a, g, false),
        None => (0.0, Vec::new(), true),
    };
    // The coverage kernel works on stemmed tokens; map them back to readable,
    // unstemmed forms before surfacing them so the UI shows "kubernetes" /
    // "developer", not the Snowball stems "kubernet" / "develop".
    let gaps = readable_gaps(&gap_stems, &display_forms(&job_text, &stemmer));

    // ONE decision, three consumers: the combined formula below, the
    // `scoreSource` label, and the explanation. All hang off this single
    // boolean, so a caller can never be told "combined" for a number that is
    // really keyword-only — the degrade case (semantic disabled, or an embed
    // that failed / a provider that is offline / the ceiling refusing the
    // round-trip). `semantic == 0.0` is NOT a usable proxy for it: a real cosine
    // can legitimately clamp to zero.
    //
    // BOTH vectors are required, and the MIXED shape is the reason this is
    // spelled out: a cosine is computed from a pair, so the `semantic` match
    // above already yields 0.0 unless both sides are present. Asking only about
    // the posting (a cached posting vector, a résumé embed that was refused or
    // failed) declared that 0.0 a measurement — publishing `0.6 × 0 + 0.4 × ats`
    // as a "combined" score, caching it under the semantic key, and serving that
    // ~40%-of-keyword number for the whole cache TTL.
    let semantic_available = resume_vec.is_some() && job_vec.is_some();
    let combined = if semantic_available {
        (0.6 * semantic + 0.4 * ats).round()
    } else {
        ats // no semantic signal available
    };

    let recommendations = recommendations(&gaps);
    // Guidance framing: the score is our estimate, not the employer's verdict.
    const GUIDANCE: &str =
        "This score is a guidance estimate — not the employer's decision or any ATS system's score.";
    let explanation = if no_jd_keywords {
        format!(
            "No extractable keywords found in this job posting — coverage score is unavailable. {GUIDANCE}"
        )
    } else if skip_semantic {
        format!(
            "Keyword coverage {ats:.0}% across {} job keywords (semantic scoring disabled). {GUIDANCE}",
            job_keywords.len()
        )
    } else if semantic_available {
        format!(
            "Semantic similarity {semantic:.0}%, keyword coverage {ats:.0}% across {} job keywords. {GUIDANCE}",
            job_keywords.len()
        )
    } else {
        // Semantic scoring is ON but no embedding pair exists (provider offline,
        // an embed that failed, or the daily ceiling refusing the round-trip).
        // Reporting the formula's placeholder as "Semantic similarity 0%" states
        // a measurement that never happened — and reads as "you are a terrible
        // match" — while `scoreSource` next to it says keyword. Distinct from
        // the disabled branch above: the user did not opt out here.
        format!(
            "Keyword coverage {ats:.0}% across {} job keywords (semantic similarity could not be computed — no embedding was available for this pair). {GUIDANCE}",
            job_keywords.len()
        )
    };

    let result = json!({
        "resumeId": resume.id,
        "jobId": job_id,
        "ats": ats,
        "semantic": semantic,
        "combined": combined,
        "gaps": gaps,
        "recommendations": recommendations,
        "explanation": explanation,
        "guidance": GUIDANCE,
        // Which kernel actually produced `combined`. Purely additive — no
        // MATCH_FORMULA_VERSION bump, because no SCORE changes: a row cached
        // before this field existed still holds the right numbers, and the one
        // consumer that branches on it (the Autopilot re-rank) writes its own
        // fresh rows under its own `resume_id`/`job_id` namespace, so it never
        // reads a field-less legacy row.
        "scoreSource": if semantic_available { SCORE_SOURCE_COMBINED } else { SCORE_SOURCE_KEYWORD },
    });
    // Cache only a result the key can honestly describe. A `semantic_enabled = 1`
    // key promises a semantic answer; when the embed did not happen (provider
    // offline, or the daily ceiling refused the round-trip) the number is
    // keyword-only, and freezing it under that key would make the NEXT run —
    // provider back, ceiling reset — read the degrade as the semantic answer and
    // never retry, for the whole cache TTL. A keyword-only key (`semantic_enabled
    // = 0`) is always honest and always cached: that is the whole result.
    let cacheable = skip_semantic || semantic_available;
    if cacheable {
        if let Ok(s) = serde_json::to_string(&result) {
            store
                .upsert_match_score_async(cache_key.to_owned_key(), s)
                .await
                .ok();
        }
    }
    result
}

/// Ad-hoc, KEYWORD-ONLY scoring entry point for `extension_bridge::match_live`
/// (the browser extension's "Check fit" button + its `import.result.matchScore`
/// fill) — a thin forwarding wrapper around [`score_one`], NOT a new scoring
/// path: every existing `match_resume` caller is untouched, and this adds no
/// new branch to `score_one` itself. `job_id` here is a synthetic per-URL cache
/// key (not a real `PostingsCache` id) — the caller is responsible for
/// deriving it (e.g. a hash of the normalized job url) so repeat calls for the
/// same page hit the SAME self-invalidating `match_scores` row `score_one`
/// already maintains (formula version / semantic bit / job-text hash — see
/// its cache-key doc). `job_text` is required (not `Option`) because the
/// caller always has JD text in hand by construction (a browser DOM parse,
/// never a `PostingsCache` miss).
///
/// Deliberately NO `semantic_enabled` parameter (unlike the removed
/// `score_adhoc`): semantic scoring is hardcoded OFF below, not
/// caller-configurable, and this NEVER translates ([`MatchSurface::Extension`]
/// to [`score_one`]) — the extension bridge has no channel to the app's
/// semantic-scoring setting (see `extension_bridge::match_live`'s module doc)
/// and a CLI-agent provider configured as "local" still performs cloud egress
/// despite `ProviderId::is_local()`, so the zero-egress guarantee for this
/// entry point must be structural (no flag to flip), not a default. Trade-off:
/// a foreign-language job posting is scored keyword-only against its RAW
/// (untranslated) text — an accepted accuracy cost for that guarantee.
pub(crate) async fn score_adhoc_keyword_only(
    app: &AppHandle,
    store: &DocumentStore,
    resume: &DocumentRecord,
    resume_raw_keywords: Option<&[String]>,
    active: &EmbeddingConfig,
    job_id: &str,
    job_text: String,
) -> Value {
    score_one(
        &AppScoreIo(app),
        store,
        resume,
        resume_raw_keywords,
        active,
        job_id,
        Some(job_text),
        0, // semantic_enabled hardcoded OFF — never caller-configurable
        // Never translates: this entry point must not reach the provider layer.
        MatchSurface::Extension,
        None, // keyword-only: there is no round-trip to budget
    )
    .await
}

/// The `scoreSource` value [`score_one`] emits when a real embedding pair backed
/// the `combined` number. Anything else — a keyword-only run, a failed embed, an
/// error object, a field-less legacy cache row — is a degrade. One constant so
/// the producer and the Autopilot consumer can't drift.
pub(crate) const SCORE_SOURCE_COMBINED: &str = "combined";

/// The `scoreSource` value for a `combined` number that is really the keyword
/// score — semantic scoring off, or an embedding that did not happen. The
/// degrade half of [`SCORE_SOURCE_COMBINED`], named for the same reason.
pub(crate) const SCORE_SOURCE_KEYWORD: &str = "keyword";

/// Content-addressed cache identity for an Autopilot's résumé snapshot.
///
/// The Autopilot record persists `resume_text` (a raw string copied at setup
/// time), not a `DocumentRecord` id, so the semantic path needs a stable id for
/// its `posting_vectors` / `match_scores` rows. Hashing the text makes it
/// **self-invalidating**: editing the autopilot's résumé yields a different id,
/// so a stale résumé vector can never be scored against — the same discipline
/// `posting_vectors.text_hash` uses.
///
/// The `autopilot-resume:` namespace prefix does two jobs. It marks the id as
/// synthetic, so `DocumentStore::upsert_vector` REFUSES it (see
/// `documents::is_synthetic_scoring_id`) and the document index can never
/// acquire a row nothing ever deletes. And it separates this key space from the
/// posting keys (`autopilot:<hash of canonical_job_key>`), which now share the
/// `posting_vectors` table with it.
///
/// `pub(crate)` so `commands::autopilot`'s cache-reuse test can assert against
/// the REAL identity instead of a hand-retyped mirror of this format string.
pub(crate) fn autopilot_resume_id(resume_text: &str) -> String {
    format!("autopilot-resume:{}", sha256_hex(resume_text))
}

/// The synthetic [`DocumentRecord`] the Autopilot re-rank scores with — an
/// Autopilot stores résumé TEXT, not a document reference.
///
/// Only the four fields `score_one` reads are meaningful:
/// - `id` — [`autopilot_resume_id`], the content-addressed cache identity;
/// - `text` — the résumé itself;
/// - `locale` — **`None`, deliberately**: the Jobs page reads the persisted
///   (nullable) `DocumentRecord.locale` and falls back to `"en"`
///   ([`resume_target_lang`]), and a snapshot has no persisted locale, so
///   `None` is the SAME source resolving the SAME way. Detecting the language
///   here instead would make the two surfaces disagree about a non-English
///   résumé — a parity break in the opposite direction from a missing
///   translate step. (Detect-and-backfill onto the document row would be
///   better behaviour for both surfaces; that is a separate change, not
///   something to smuggle into one of them.)
/// - `keywords_json: None` — no cached token list, so `score_one` live-extracts
///   (its documented fallback).
pub(crate) fn autopilot_resume_record(resume_text: &str) -> DocumentRecord {
    DocumentRecord {
        id: autopilot_resume_id(resume_text),
        title: String::new(),
        name: String::new(),
        locale: None,
        text: resume_text.to_string(),
        pages: None,
        created_at: 0,
        indexed: false,
        is_default: false,
        keywords_json: None,
    }
}

/// SEMANTIC (combined) scoring entry point for the headless Autopilot re-rank —
/// a thin forwarding wrapper around [`score_one`], NOT a second scoring path:
/// no new branch is added to `score_one`, so Autopilot and the Jobs page share
/// one kernel, `languages_align` included. That inclusion is the point: the
/// Autopilot's previous `coverage_score` path stemmed BOTH sides with the JD
/// stemmer unconditionally, which mangles language-neutral tokens on a
/// cross-language résumé↔posting pair; routing through `score_one` closes that
/// known divergence (ADR-020 addendum).
///
/// `job_id` is a synthetic per-job cache key the caller derives (see
/// `commands::autopilot::autopilot_job_id`) — Autopilot postings never enter
/// `PostingsCache`, so there is no real posting id to use.
///
/// The résumé is wrapped by [`autopilot_resume_record`] (see its doc for why
/// every field is what it is).
///
/// [`MatchSurface::Autopilot`] means the FULL pre-processing pipeline runs here,
/// exactly as on the Jobs page — translation included. That is not a cost
/// decision to re-litigate per surface: the number is rendered under the same
/// "Match %" label, and translation is cloud-excluded (local providers only, so
/// it cannot incur an API cost), cached per job id for the session, and bounded
/// by the caller's top-N ceiling.
///
/// `budget` is the headless run's share of the shared per-provider daily
/// ceiling. It is charged inside the kernel, once per embed that actually
/// happens (see [`score_one`]) — a fully-cached job costs nothing, and a job
/// that has to embed BOTH the résumé snapshot and the posting costs two.
pub(crate) async fn score_autopilot_semantic(
    app: &AppHandle,
    store: &DocumentStore,
    resume_text: &str,
    active: &EmbeddingConfig,
    job_id: &str,
    job_text: String,
    budget: &dyn EmbedBudget,
) -> Value {
    let resume = autopilot_resume_record(resume_text);
    score_one(
        &AppScoreIo(app),
        store,
        &resume,
        None, // no cached keyword list for a raw résumé snapshot — live-extract
        active,
        job_id,
        Some(job_text),
        1, // semantic_enabled: this entry point exists only for the semantic re-rank
        MatchSurface::Autopilot,
        Some(budget),
    )
    .await
}

/// Parse the résumé's cached normalized keywords (`keywords_json`) into a token
/// list. Absent OR corrupt JSON → `None`, which makes [`score_one`] fall back to
/// live extraction from `resume.text` (the legacy behaviour). `pub(crate)` so
/// `extension_bridge::match_live` reuses the SAME fallback rule instead of
/// re-deriving it.
pub(crate) fn parse_resume_keywords(resume: &DocumentRecord) -> Option<Vec<String>> {
    resume
        .keywords_json
        .as_deref()
        .and_then(|j| serde_json::from_str::<Vec<String>>(j).ok())
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
    let job_text = job_text_for(&app, &req.job_id);

    score_one(
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
    .await
}

/// Build a searchable text blob for a single cached posting JSON value (title +
/// description + requirements). Pure — no lock — so it can be reused for both the
/// single-job and batch lookups. Returns None if the posting has no usable text.
fn posting_to_text(posting: &Value) -> Option<String> {
    let title = posting.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let description = posting.get("description").and_then(|v| v.as_str());
    // `requirements` is an array of strings; collect to a Vec the shared helper
    // can borrow as a slice.
    let requirements: Option<Vec<String>> = posting
        .get("requirements")
        .and_then(|v| v.as_array())
        .map(|reqs| {
            reqs.iter()
                .filter_map(|r| r.as_str().map(|s| s.to_string()))
                .collect()
        });
    crate::documents::keywords::posting_text_blob(title, description, requirements.as_deref())
}

/// Build a searchable text blob for a cached job posting (title + description +
/// requirements). Returns None if the posting isn't in the live cache.
///
/// `pub(crate)` so the agent tools reuse the same posting → text resolution instead
/// of re-deriving it.
pub(crate) fn job_text_for(app: &AppHandle, job_id: &str) -> Option<String> {
    let cache = app.state::<Mutex<PostingsCache>>();
    let guard = cache.lock();
    let posting = guard
        .get_all()
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(job_id))?;
    posting_to_text(posting)
}

/// Identity fields of a cached posting: the company/title/url/board an
/// application aggregate needs when a document is saved for it. All loaded
/// server-side by id (mirrors [`job_text_for`]'s single-lock lookup) — the model
/// never supplies these, so a prompt-injected posting can't spoof the target of a
/// save. Returns `None` when the posting isn't in the live cache.
#[derive(Debug, Clone, Default)]
pub(crate) struct JobPostingMeta {
    pub company: String,
    pub title: String,
    pub url: String,
    pub board: String,
    pub location: String,
}

/// `pub(crate)` so the agent's `save_cover_letter` Write tool resolves the same
/// posting identity the rest of the app uses, instead of re-deriving it.
pub(crate) fn job_meta_for(app: &AppHandle, job_id: &str) -> Option<JobPostingMeta> {
    let cache = app.state::<Mutex<PostingsCache>>();
    let guard = cache.lock();
    let posting = guard
        .get_all()
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(job_id))?;
    let field = |k: &str| {
        posting
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    Some(JobPostingMeta {
        company: field("company"),
        title: field("title"),
        url: field("url"),
        // `JobPosting` serializes the originating board under `source`.
        board: field("source"),
        location: field("location"),
    })
}

fn recommendations(gaps: &[String]) -> Vec<String> {
    if gaps.is_empty() {
        return vec!["Strong keyword coverage — no obvious gaps.".to_string()];
    }
    let preview: Vec<&str> = gaps.iter().take(8).map(String::as_str).collect();
    vec![format!(
        "Consider adding evidence of: {}.",
        preview.join(", ")
    )]
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
mod test;
