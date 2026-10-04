//! The match-score kernel: `score_one` and the types it is driven through — the
//! surface that is asking, where the résumé vector is cached, and the `ScoreIo`
//! seam for translation + embedding. Split out of `commands/match_resume.rs` for
//! R8 (issue #1280); every weight, threshold, message and ordering moved verbatim.
//! `match_resume.rs` re-exports the crate-visible items, so each keeps its
//! `commands::match_resume::<name>` path.

use std::collections::HashSet;

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::ai_provider::{EmbeddingVector, EMBEDDING_VECTOR_VERSION};
use crate::documents::keywords::{
    apply_stemmer, display_forms, keyword_coverage, keywords, keywords_normalized, languages_align,
    make_stemmer, readable_gaps,
};
use crate::documents::{
    embed_charged, posting_vector_or_embed, sha256_hex, AppEmbedder, DocumentRecord, DocumentStore,
    EmbedBudget, Embedder, EmbeddingConfig, MatchScoreKey,
};

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
///
/// Bumped 2 -> 3: `documents::keywords::keywords_normalized_list` gained
/// language-aware stopwords (German/French/Spanish/Italian/Portuguese/Dutch,
/// selected the same way `make_stemmer` picks its algorithm) plus a
/// pure-numeric-token filter. Both change which tokens survive into a job's
/// or résumé's keyword set, so every previously-cached score is stale. A
/// stale row is never read again once this constant differs (`formula_version`
/// is part of the `match_scores` table's PRIMARY KEY, so a bumped-version
/// lookup is a structural miss, not a stale hit) — old rows are simply
/// orphaned until the existing TTL/max-rows prune sweep (`prune_caches`) or a
/// resume-scoped delete reclaims them. No migration needed.
pub(super) const MATCH_FORMULA_VERSION: i64 = 3;

/// Which user-facing surface is asking for a score.
///
/// Every surface that renders its number under the app's "Match %" label (the
/// Jobs page, the headless Autopilot re-rank, and the Score tab) shares the
/// SAME translation pre-processing: [`crate::commands::translation::
/// translate_if_needed`] rewrites the JD into the résumé language BEFORE both
/// keyword extraction and the embed, so skipping it on one of them flips
/// [`languages_align`] for a cross-language pair — collapsing coverage to
/// language-neutral tech tokens and embedding a cross-lingual cosine. The same
/// job would then show two materially different percentages on two screens.
///
/// [`MatchSurface::Extension`] is the ONE deliberate exception to translation:
/// it never shows a combined number, and its zero-egress guarantee has to be
/// structural (no flag to flip) — see [`score_adhoc_keyword_only`].
///
/// [`MatchSurface::JobAdText`] DOES translate, and it runs the same
/// markdown-stripping blob builder
/// ([`crate::documents::keywords::posting_text_blob`], via
/// [`job_ad_text_blob`]) the Jobs page runs on the description (via
/// [`posting_to_text`]) — so identical job text scores identically on both
/// surfaces **at `semantic_enabled = 0`**. Two axes still legitimately
/// diverge, deliberately, and neither should be forced to zero:
///
/// 1. **Composition.** The Jobs page's blob is title + description +
///    requirements ([`posting_to_text`]); `JobAdView` only ever holds the
///    description (`jobDesc: string`), so [`score_resume_against_text`] builds
///    its blob with an empty title and no requirements. A posting whose title
///    alone carries a keyword the description never repeats scores lower here
///    than on the Jobs page — an accepted cost of this surface having
///    strictly less structured input, not a bug.
/// 2. **`semantic_enabled`.** Both surfaces now read the SAME renderer
///    preference (`useSemanticScoring()`), threaded through as an explicit,
///    optional request flag (`MatchResumeRequest.semanticScoringEnabled` /
///    `MatchTextRequest.semanticScoringEnabled` — see [`semantic_enabled_bit`]
///    for the shared default-to-keyword-only rule). With the preference ON,
///    BOTH surfaces run the SAME 0.6/0.4 weighted blend of embedding
///    similarity and keyword coverage over their own composed blob — this
///    axis no longer diverges between them. Only axis 1 (composition) still
///    legitimately does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MatchSurface {
    /// The in-app [`match_resume`] command (the Jobs page and everything routed
    /// through it).
    JobsPage,
    /// The headless Autopilot phase-2 semantic re-rank.
    Autopilot,
    /// The browser extension's ad-hoc, keyword-only "Check fit".
    Extension,
    /// The in-app Score tab's ad-hoc scoring of arbitrary job-ad TEXT (no
    /// `PostingsCache` id in hand) — see [`score_resume_against_text`].
    JobAdText,
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
            // All three score a REAL stored `documents` row, never a snapshot.
            Self::JobsPage | Self::Extension | Self::JobAdText => ResumeVectorHome::DocumentIndex,
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
pub(super) async fn score_one(
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
    // ONE comparison, and both the number and its availability are derived from
    // it. `compare` refuses a cross-space pair (`Err`) — presence of two vectors
    // is not comparability, and the two answers must not be sourced separately:
    // asking `is_some()` about the vectors while the score came from a refused
    // `compare` is exactly how a placeholder becomes a published measurement.
    //
    // `None` here means "no cosine exists": embeddings disabled, one/both sides
    // unavailable (offline provider, failed embed, ceiling refusal), or a pair
    // whose spaces do not match.
    let comparison = match (&resume_vec, &job_vec) {
        (Some(a), Some(b)) => crate::commands::ai_provider::compare(a, b).ok(),
        _ => None,
    };
    let semantic = comparison.map_or(0.0, |s| (s.clamp(0.0, 1.0) * 100.0).round());

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
    // It is the COMPARISON that is available or not — never the vectors. Two
    // mistakes live on this line historically, and both published
    // `0.6 × 0 + 0.4 × ats` as a "combined" score, cached it under the semantic
    // key, and served that ~40%-of-keyword number for the whole cache TTL:
    //
    // - `job_vec.is_some()` — a MIXED pair (cached posting, résumé embed refused
    //   or failed) called the placeholder a measurement;
    // - `resume_vec.is_some() && job_vec.is_some()` — presence of BOTH is still
    //   not comparability. `compare` returns `Err` for a cross-space pair and the
    //   `.ok()` above flattens it to the same `0.0`, so an incomparable pair
    //   passed a presence check while no cosine had been computed at all.
    let semantic_available = comparison.is_some();
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

/// The `scoreSource` value [`score_one`] emits when a real embedding pair backed
/// the `combined` number. Anything else — a keyword-only run, a failed embed, an
/// error object, a field-less legacy cache row — is a degrade. One constant so
/// the producer and the Autopilot consumer can't drift.
pub(crate) const SCORE_SOURCE_COMBINED: &str = "combined";

/// The `scoreSource` value for a `combined` number that is really the keyword
/// score — semantic scoring off, or an embedding that did not happen. The
/// degrade half of [`SCORE_SOURCE_COMBINED`], named for the same reason.
pub(crate) const SCORE_SOURCE_KEYWORD: &str = "keyword";

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
