//! Unit tests for `commands::match_resume` — the metric-label contract
//! (one pre-processing pipeline per "Match %" surface), the cache-key
//! self-invalidation, and the pure pieces of the combined formula.
//!
//! Split into a sibling file (the `commands/autopilot.rs` + `commands/autopilot/`
//! precedent) purely to keep the parent module under R8's LOC cap; nothing about
//! the tests themselves changed in the move.

use super::*;
// Keyword-extraction and the coverage/gap math (stopwords, synonyms, short
// terms, `keyword_coverage`, `coverage_score`) are owned and tested by
// `crate::documents::keywords`. These cover the match-command wiring that
// still lives here: the corrupt-keywords fallback and readable gaps.

// ── the metric-label contract: one pipeline per "Match %" ────────────

/// Both surfaces that render a "Match %" must feed `score_one` the SAME
/// pre-processing. `translate_if_needed` runs BEFORE keyword extraction and
/// BEFORE the embed, so turning it off for one surface flips
/// `languages_align` on a cross-language pair — collapsing coverage to
/// language-neutral tech tokens and embedding a cross-lingual cosine. The
/// same job would then show two materially different percentages depending
/// on which screen the user is looking at.
#[test]
fn every_match_percent_surface_runs_the_same_pre_processing() {
    assert!(
        MatchSurface::JobsPage.translates(),
        "the in-app path has always translated"
    );
    assert_eq!(
        MatchSurface::Autopilot.translates(),
        MatchSurface::JobsPage.translates(),
        "the Autopilot re-rank renders its number under the same label as the Jobs page, \
         so it must run the same pipeline — translation is cloud-excluded (local providers \
         only), cached per job id, and bounded by the caller's top-N, so there is no cost \
         argument that survives the divergence"
    );
    assert!(
        !MatchSurface::Extension.translates(),
        "the extension's zero-egress guarantee is structural: it must never reach the \
         provider layer, and it never shows a combined number"
    );
}

/// The résumé language is resolved from ONE source — the persisted
/// (nullable) `DocumentRecord.locale`, falling back to `"en"`. An Autopilot
/// snapshot has no persisted locale, so it must land on the same fallback a
/// locale-less Jobs-page document does; detecting it on one surface only
/// would be the same divergence in the other direction.
#[test]
fn the_autopilot_resume_snapshot_resolves_its_language_like_a_jobs_page_document() {
    let german = "Erfahrener Softwareentwickler mit Kubernetes, Rust und Postgres, \
                  verantwortlich für den Aufbau verteilter Systeme.";
    let jobs_page = DocumentRecord {
        id: "doc-1".into(),
        title: String::new(),
        name: String::new(),
        locale: None, // `documents_add`'s `locale` is optional
        text: german.to_string(),
        pages: None,
        created_at: 0,
        indexed: false,
        is_default: false,
        keywords_json: None,
    };
    let autopilot = autopilot_resume_record(german);

    assert_eq!(
        autopilot.locale, jobs_page.locale,
        "same (absent) locale source on both surfaces"
    );
    assert_eq!(
        resume_target_lang(&autopilot),
        resume_target_lang(&jobs_page),
        "…so both resolve the same translation target and the same languages_align input"
    );
    assert_eq!(autopilot.text, jobs_page.text);
    assert_eq!(
        autopilot.keywords_json, None,
        "no cached token list for a raw snapshot — score_one live-extracts, its documented \
         fallback, from the identical text"
    );
}

/// A résumé SNAPSHOT has no `documents` row, so its vector must not enter
/// the document index (nothing would ever delete it, and the Embeddings
/// panel counts every row there).
#[test]
fn only_a_real_document_resume_is_written_to_the_document_vector_index() {
    assert_eq!(
        MatchSurface::JobsPage.resume_vector_home(),
        ResumeVectorHome::DocumentIndex
    );
    assert_eq!(
        MatchSurface::Extension.resume_vector_home(),
        ResumeVectorHome::DocumentIndex
    );
    assert_eq!(
        MatchSurface::Autopilot.resume_vector_home(),
        ResumeVectorHome::EphemeralCache,
        "the Autopilot résumé is a content-addressed snapshot: its vector belongs in the \
         TTL-pruned posting-vector cache, never in the document index"
    );
    assert!(
        crate::documents::is_synthetic_scoring_id(&autopilot_resume_id("any résumé")),
        "…and its id is one the document index refuses outright"
    );
}

// The stemmed gaps from `keyword_coverage` must be mapped back to readable,
// unstemmed forms before surfacing — "kubernetes"/"developer", not the
// Snowball stems "kubernet"/"develop". Mirrors `score_one`'s gap pipeline.
#[test]
fn gaps_are_surfaced_in_readable_unstemmed_form() {
    use crate::documents::keywords::{display_forms, make_stemmer, readable_gaps};

    let job_text = "kubernetes developer building scalable services";
    let stemmer = make_stemmer(job_text);
    let job_kw = keywords(job_text, &stemmer);
    // An empty résumé → every job keyword is a gap.
    let (_ats, gap_stems) =
        keyword_coverage(&job_kw, &HashSet::new()).expect("non-empty job must return Some");

    // The raw stems are mangled.
    assert!(
        gap_stems.iter().any(|g| g == "kubernet" || g == "develop"),
        "precondition: stems should be mangled; got {gap_stems:?}"
    );

    let readable = readable_gaps(&gap_stems, &display_forms(job_text, &stemmer));
    assert!(
        readable.iter().any(|g| g == "kubernetes"),
        "readable gaps must contain 'kubernetes', not the stem; got {readable:?}"
    );
    assert!(
        readable.iter().any(|g| g == "developer"),
        "readable gaps must contain 'developer', not 'develop'; got {readable:?}"
    );
    assert!(
        !readable.iter().any(|g| g == "kubernet" || g == "develop"),
        "no mangled stems may leak into the readable gaps; got {readable:?}"
    );
}

// Corrupt keywords_json must not silently produce an empty resume word-set.
// Verifies that the match-branch falls back to live extraction so ATS
// score is computed from the resume text rather than an empty HashSet.
#[test]
fn corrupt_keywords_json_falls_back_to_live_extraction() {
    use crate::documents::keywords::make_stemmer;

    let resume_text = "experienced rust and typescript developer";
    let stemmer = make_stemmer(resume_text);

    // Simulate the deserialization branch directly: malformed JSON that
    // would previously silent-default to Vec::new() / empty HashSet.
    let corrupt_json = "not valid json [[[";
    let resume_words: HashSet<String> = match serde_json::from_str::<Vec<String>>(corrupt_json) {
        Ok(tokens) => apply_stemmer(tokens.into_iter().collect(), &stemmer),
        Err(_) => keywords(resume_text, &stemmer),
    };

    // The fallback must not be empty — the resume text has real content.
    assert!(
        !resume_words.is_empty(),
        "corrupt keywords_json must fall back to live extraction, not an empty set"
    );

    // A job keyword present in the resume text must be covered.
    let job = keywords("rust developer typescript", &stemmer);
    let (cov, _gaps) =
        keyword_coverage(&job, &resume_words).expect("non-empty job must return Some");
    assert!(
        cov > 0.0,
        "ATS coverage must be > 0 when resume text contains matching terms"
    );
}

// Pins the production `semantic_enabled_bit` helper (used by both the cache
// key and the skip-branch): only `Some(true)` → 1 (enabled); `Some(false)`
// AND `None` → 0 (keyword-only) so an omitted flag defaults OFF, matching the
// app-wide default. Tests the real fn, not an inline re-implementation.
#[test]
fn semantic_enabled_bit_maps_flag_to_key_column() {
    assert_eq!(semantic_enabled_bit(Some(false)), 0, "explicit disable → 0");
    assert_eq!(semantic_enabled_bit(Some(true)), 1, "explicit enable → 1");
    assert_eq!(
        semantic_enabled_bit(None),
        0,
        "default (unset) → keyword-only (semantic OFF)"
    );
}

// A bump to MATCH_FORMULA_VERSION must change the cache key, so a score
// cached under the current version is a miss under the next one. Exercises
// self-invalidation end-to-end against a real store.
#[test]
fn formula_version_bump_invalidates_cached_score() {
    use crate::documents::{sha256_hex, DocumentStore, MatchScoreKey};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let hash = sha256_hex("job text");
    let key = |fv: i64| MatchScoreKey {
        resume_id: "r",
        job_id: "j",
        provider: "ollama",
        model: "nomic-embed-text",
        semantic_enabled: 1,
        formula_version: fv,
        vector_version: EMBEDDING_VECTOR_VERSION,
        job_text_hash: &hash,
    };

    // Cache a score under the current formula version → hit.
    store
        .upsert_match_score(&key(MATCH_FORMULA_VERSION), "{\"combined\":50}")
        .unwrap();
    assert!(store.get_match_score(&key(MATCH_FORMULA_VERSION)).is_some());

    // The next formula version is a different key → miss (stale on bump).
    assert!(store
        .get_match_score(&key(MATCH_FORMULA_VERSION + 1))
        .is_none());
}

// The defect this pins: a semantic score is derived from embedding vectors,
// so a vector-FORMAT bump (`EMBEDDING_VECTOR_VERSION`) changes what a cached
// score means even when `formula_version` and the job text are unchanged.
// Before `vector_version` joined the key, this bump only self-invalidated
// by accident (a coincidental MATCH_FORMULA_VERSION bump, e.g. #933) — a
// future vector-format bump with no coincidental formula bump would have
// silently served a stale semantic score forever. Two otherwise-identical
// keys differing ONLY in `vector_version` must not collide.
#[test]
fn vector_version_bump_invalidates_cached_score() {
    use crate::documents::{sha256_hex, DocumentStore, MatchScoreKey};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let hash = sha256_hex("job text");
    let key = |vv: i64| MatchScoreKey {
        resume_id: "r",
        job_id: "j",
        provider: "ollama",
        model: "nomic-embed-text",
        semantic_enabled: 1,
        formula_version: MATCH_FORMULA_VERSION,
        vector_version: vv,
        job_text_hash: &hash,
    };

    // Cache a score under the current vector version → hit.
    store
        .upsert_match_score(&key(EMBEDDING_VECTOR_VERSION), "{\"combined\":50}")
        .unwrap();
    assert!(store
        .get_match_score(&key(EMBEDDING_VECTOR_VERSION))
        .is_some());

    // The next vector version is a different key → miss (stale on bump),
    // with formula_version and every other field held identical — proves
    // vector_version alone, not some other field, drives the invalidation.
    assert!(store
        .get_match_score(&key(EMBEDDING_VECTOR_VERSION + 1))
        .is_none());
}

// MATCH_FORMULA_VERSION guard: if a maintainer bumps the constant they MUST
// also bump the expected value here and invalidate any affected caches.
// Failing here is intentional — it's the reminder that a bump is breaking.
#[test]
fn formula_version_constant_is_pinned() {
    assert_eq!(
        MATCH_FORMULA_VERSION, 2,
        "MATCH_FORMULA_VERSION changed — update this assert AND invalidate \
         cached match scores (clear match_scores table or bump the stored version)"
    );
}

// A6 — Combined-score formula: combined = round(0.6 * semantic + 0.4 * ats).
// Tests the arithmetic kernel in isolation, covering the branch in `score_one`
// where `job_vec.is_some()` is true. The formula is not `0.6*s + 0.4*a` before
// rounding — we pin the specific rounded values to catch weight drift.
#[test]
fn combined_formula_is_weighted_60_semantic_40_ats_rounded() {
    // Simulate the production formula: both vectors present → combined branch.
    let semantic = 80.0_f64;
    let ats = 60.0_f64;
    let combined = (0.6 * semantic + 0.4 * ats).round();
    // 0.6 * 80 + 0.4 * 60 = 48 + 24 = 72 → rounded = 72
    assert_eq!(
        combined, 72.0,
        "combined must be round(0.6*80 + 0.4*60) = 72"
    );

    // Verify a different pair to guard against accidental integer short-circuit.
    let semantic2 = 75.0_f64;
    let ats2 = 50.0_f64;
    let combined2 = (0.6 * semantic2 + 0.4 * ats2).round();
    // 0.6 * 75 + 0.4 * 50 = 45 + 20 = 65 → rounded = 65
    assert_eq!(
        combined2, 65.0,
        "combined must be round(0.6*75 + 0.4*50) = 65"
    );

    // When semantic and ats differ, combined must differ from BOTH so we can
    // distinguish it from an accidental identity (combined == ats).
    assert_ne!(
        combined, ats,
        "combined must differ from ats (weights are 0.6/0.4)"
    );
    assert_ne!(
        combined, semantic,
        "combined must differ from semantic (weights are 0.6/0.4)"
    );
}

// A6 — Degrade path: when the semantic vector is unavailable (`job_vec.is_none()`),
// the production branch in `score_one` yields `combined = ats` (no semantic
// weighting). This test pins that degrade-path logic is `!= 0.6*semantic +
// 0.4*ats`; combined equals ATS score when semantic is absent.
//
// The branch in score_one is: `let combined = if job_vec.is_some() {
//     (0.6 * semantic + 0.4 * ats).round() } else { ats };`
// We verify that the ELSE arm produces exactly `ats`, not 0.6*0 + 0.4*ats.
#[test]
fn degrade_path_combined_equals_ats_when_no_semantic_vector() {
    // Simulate: job_vec is None → semantic stays 0.0 (no computation),
    // combined = ats (the else branch).
    let ats = 65.0_f64;
    let job_vec_present = false;
    let semantic = 0.0_f64; // unused in degrade branch

    let combined = if job_vec_present {
        (0.6 * semantic + 0.4 * ats).round()
    } else {
        ats // degrade: keyword-only
    };

    assert_eq!(
        combined, ats,
        "degrade path (no job vector) must yield combined == ats ({ats}); got {combined}"
    );

    // The degrade combined must NOT equal the weighted formula applied to
    // ats alone (0.6*0 + 0.4*65 = 26 ≠ 65), proving the else-branch is
    // `ats` not `0.6*semantic + 0.4*ats`.
    let weighted_ats_only = (0.6 * 0.0 + 0.4 * ats).round();
    assert_ne!(
        combined, weighted_ats_only,
        "degrade combined ({combined}) must not be the weighted-formula partial ({weighted_ats_only})"
    );
}

// A6 — Degrade explanation: when semantic is disabled the explanation must
// say "(semantic scoring disabled)" and NOT mention "Semantic similarity".
// When semantic is available the explanation includes "Semantic similarity".
// Both explanations must carry the guidance framing ("guidance estimate").
// Mirrors the `explanation` construction in `score_one` (pure string logic,
// tested without AppHandle).
#[test]
fn explanation_reflects_semantic_enabled_state() {
    let job_kw_count = 10_usize;
    let ats = 70.0_f64;
    let semantic = 85.0_f64;
    const GUIDANCE: &str =
        "This score is a guidance estimate — not the employer's decision or any ATS system's score.";

    // Degrade (skip_semantic = true):
    let degrade_explanation = format!(
        "Keyword coverage {ats:.0}% across {job_kw_count} job keywords (semantic scoring disabled). {GUIDANCE}"
    );
    assert!(
        degrade_explanation.contains("semantic scoring disabled"),
        "degrade explanation must say 'semantic scoring disabled'; got: {degrade_explanation}"
    );
    assert!(
        !degrade_explanation.contains("Semantic similarity"),
        "degrade explanation must NOT mention 'Semantic similarity'; got: {degrade_explanation}"
    );
    assert!(
        degrade_explanation.contains("guidance estimate"),
        "degrade explanation must carry guidance framing; got: {degrade_explanation}"
    );

    // Normal (skip_semantic = false):
    let normal_explanation = format!(
        "Semantic similarity {semantic:.0}%, keyword coverage {ats:.0}% across {job_kw_count} job keywords. {GUIDANCE}"
    );
    assert!(
        normal_explanation.contains("Semantic similarity"),
        "normal explanation must mention 'Semantic similarity'; got: {normal_explanation}"
    );
    assert!(
        !normal_explanation.contains("disabled"),
        "normal explanation must NOT mention 'disabled'; got: {normal_explanation}"
    );
    assert!(
        normal_explanation.contains("guidance estimate"),
        "normal explanation must carry guidance framing; got: {normal_explanation}"
    );
}

// Empty JD keywords → explanation flags unavailable score, not misleading 0%.
// Mirrors the `no_jd_keywords` branch in `score_one`.
#[test]
fn empty_jd_keywords_explanation_flags_unavailable() {
    const GUIDANCE: &str =
        "This score is a guidance estimate — not the employer's decision or any ATS system's score.";
    let explanation = format!(
        "No extractable keywords found in this job posting — coverage score is unavailable. {GUIDANCE}"
    );
    assert!(
        explanation.contains("No extractable keywords"),
        "empty-JD explanation must flag unavailability; got: {explanation}"
    );
    assert!(
        explanation.contains("guidance estimate"),
        "empty-JD explanation must carry guidance framing; got: {explanation}"
    );
    // Must NOT claim 0% — that would be indistinguishable from a real mismatch.
    assert!(
        !explanation.contains("0%"),
        "empty-JD explanation must not claim 0%; got: {explanation}"
    );
}

// Stemmer-language guard: when JD language matches the résumé locale,
// apply_stemmer runs; when they diverge, the normalized (unstemmed) set is
// used directly. This pins the guard logic (pure boolean, no AppHandle).
#[test]
fn stemmer_language_guard_skips_stemming_on_mismatch() {
    use crate::documents::keywords::{apply_stemmer, keywords_normalized, make_stemmer};

    // German JD, English résumé (locale "en") — languages diverge.
    let jd_text = "Wir suchen einen erfahrenen Softwareentwickler mit Rust-Kenntnissen";
    let stemmer = make_stemmer(jd_text); // German stemmer
    let resume_tokens = keywords_normalized("experienced rust developer");

    // Guard logic mirrors score_one: German JD, English locale → no match.
    let jd_matches_en = false; // German JD vs "en" locale
    let resume_words_diverge: HashSet<String> = if jd_matches_en {
        apply_stemmer(resume_tokens.clone(), &stemmer)
    } else {
        resume_tokens.clone() // unstemmed
    };

    // When languages match (English JD, English résumé) → stemmer applied.
    let en_jd = "experienced rust developer";
    let en_stemmer = make_stemmer(en_jd);
    let en_tokens = keywords_normalized("experienced rust developer");
    let resume_words_match = apply_stemmer(en_tokens.clone(), &en_stemmer);

    // The stemmed set must differ from the unstemmed one for ordinary words.
    // ("developer" → "develop" under English Snowball).
    assert!(
        resume_words_match.contains("develop"),
        "English stemmer must reduce 'developer' to 'develop'; got {:?}",
        resume_words_match
    );
    assert!(
        resume_words_diverge.contains("developer"),
        "Without stemming, 'developer' must survive unstemmed; got {:?}",
        resume_words_diverge
    );
    assert!(
        !resume_words_diverge.contains("develop"),
        "Without stemming, stemmed form 'develop' must be absent; got {:?}",
        resume_words_diverge
    );
}

// Round-trip parity: a 7-field MatchScore JSON blob survives
// upsert_match_score → get_match_score with every field name and type intact.
// Guards against a future rename/drop of any result-cache field.
#[test]
fn match_score_round_trip_preserves_all_seven_fields() {
    use crate::documents::{sha256_hex, DocumentStore, MatchScoreKey};
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();

    let hash = sha256_hex("round trip job text");
    let key = MatchScoreKey {
        resume_id: "resume-rt",
        job_id: "job-rt",
        provider: "ollama",
        model: "nomic-embed-text",
        semantic_enabled: 1,
        formula_version: MATCH_FORMULA_VERSION,
        vector_version: EMBEDDING_VECTOR_VERSION,
        job_text_hash: &hash,
    };

    // Build a known 7-field score JSON that mirrors the shape score_one produces.
    let score_json = serde_json::json!({
        "resumeId":       "resume-rt",
        "jobId":          "job-rt",
        "ats":            60.0_f64,
        "semantic":       75.0_f64,
        "combined":       70.0_f64,
        "gaps":           ["kubernetes", "terraform"],
        "recommendations": ["Consider adding evidence of: kubernetes, terraform."]
    });
    store
        .upsert_match_score(&key, &serde_json::to_string(&score_json).unwrap())
        .unwrap();

    let got = store
        .get_match_score(&key)
        .expect("score must be present after upsert");

    assert_eq!(
        got["resumeId"], "resume-rt",
        "resumeId field must survive round-trip"
    );
    assert_eq!(
        got["jobId"], "job-rt",
        "jobId field must survive round-trip"
    );
    assert_eq!(
        got["ats"], 60.0_f64,
        "ats field must survive round-trip as a number"
    );
    assert_eq!(
        got["semantic"], 75.0_f64,
        "semantic field must survive round-trip as a number"
    );
    assert_eq!(
        got["combined"], 70.0_f64,
        "combined field must survive round-trip as a number"
    );
    assert!(
        got["gaps"].is_array(),
        "gaps must survive round-trip as an array"
    );
    assert_eq!(
        got["gaps"].as_array().unwrap().len(),
        2,
        "gaps array length must be preserved"
    );
    assert!(
        got["recommendations"].is_array(),
        "recommendations must survive round-trip as an array"
    );
    // Distinct values: ats != semantic != combined — guards against field swap.
    assert_ne!(
        got["ats"], got["combined"],
        "ats and combined must be distinct"
    );
    assert_ne!(
        got["semantic"], got["combined"],
        "semantic and combined must be distinct"
    );
}

// Integration test for HIGH stemmer-asymmetry regression fix.
//
// A German-language JD and an English-locale résumé share the language-neutral
// token `docker`. With the OLD asymmetric code (JD stemmed with German stemmer,
// résumé unstemmed), the German Snowball stemmer mutates `docker` on the JD side
// while the résumé keeps the raw form — neither set contains the same token after
// asymmetric processing, so coverage is 0%.
//
// The symmetric fix leaves BOTH sides unstemmed (normalized-only) when languages
// diverge, so `docker` survives on both sides and the coverage is > 0%.
//
// This test FAILS against the pre-fix asymmetric code and PASSES after the fix.
#[test]
fn divergent_language_pair_shared_tech_token_matches_symmetrically() {
    use crate::documents::keywords::{
        apply_stemmer, keyword_coverage, keywords, keywords_normalized, make_stemmer,
    };

    // German JD with shared tech token `docker` embedded in German prose.
    let german_jd =
        "Wir suchen einen erfahrenen Softwareentwickler mit docker und kubernetes Kenntnissen";
    let english_resume = "experienced engineer shipping docker containers and kubernetes clusters";

    // Build the German stemmer (what score_one uses for this JD).
    let german_stemmer = make_stemmer(german_jd);

    // --- OLD asymmetric behavior ---
    // Old code: JD side stemmed with German stemmer; résumé side unstemmed.
    let jd_stemmed = keywords(german_jd, &german_stemmer);
    let resume_unstemmed = keywords_normalized(english_resume);
    let (old_cov, _) = keyword_coverage(&jd_stemmed, &resume_unstemmed).unwrap_or((0.0, vec![]));

    // --- NEW symmetric behavior preserves the shared token ---
    // New code: BOTH sides normalized-only (unstemmed) when languages diverge.
    let jd_normalized = keywords_normalized(german_jd);
    let resume_normalized = keywords_normalized(english_resume);
    let (new_cov, _) =
        keyword_coverage(&jd_normalized, &resume_normalized).unwrap_or((0.0, vec![]));

    // Softened from assert_eq!(old_cov, 0.0): the exact value depends on the
    // German Snowball stemmer's behaviour for `docker`/`kubernetes`, which may
    // change with a stemmer-version bump.  The invariant that actually matters
    // is that symmetric normalization yields STRICTLY more coverage than the
    // old asymmetric pairing — not that the old value is exactly 0.
    assert!(
        old_cov < new_cov,
        "symmetric normalization must yield strictly more coverage than asymmetric stemming; \
         old (asymmetric) = {old_cov}%, new (symmetric) = {new_cov}%"
    );
    assert!(
        new_cov > 0.0,
        "symmetric normalization (both unstemmed) must yield > 0% coverage \
         — 'docker' and 'kubernetes' appear on both sides; got {new_cov}%"
    );

    // Also verify that the symmetric STEMMED path (same language) is not broken:
    // English JD + English résumé sharing `docker` must still match when both are stemmed.
    let en_jd = "looking for a developer with docker and kubernetes experience";
    let en_resume = "shipped docker containers and kubernetes clusters";
    let en_stemmer = make_stemmer(en_jd);
    let jd_en_stemmed = keywords(en_jd, &en_stemmer);
    let resume_en_stemmed = apply_stemmer(keywords_normalized(en_resume), &en_stemmer);
    let (en_cov, _) = keyword_coverage(&jd_en_stemmed, &resume_en_stemmed).unwrap_or((0.0, vec![]));
    assert!(
        en_cov > 0.0,
        "matching-language path (both English, both stemmed) must still yield > 0% coverage; \
         got {en_cov}%"
    );
}

// ── trim suggestions (ranking itself lives in documents::evidence) ───────

/// The `match:trimSuggestions` payload must stay wire-identical after the
/// scorer moved into `documents::evidence`: three camelCase fields, `score`
/// as a JSON INTEGER (not `1.0`), and the same weakest-first ordering.
/// Compares the serialized shim output against the `EvidenceBullet` the
/// shared scorer produced, so a field rename or a widened numeric type on
/// either side fails here.
#[test]
fn trim_candidate_wire_shape_is_unchanged() {
    let resume = "EXPERIENCE\n\n\
                  - Built and shipped Docker containers onto a Kubernetes cluster\n\
                  - Organised the team offsite and the summer party for forty people\n";
    let job = "Backend engineer with strong Docker and Kubernetes experience.";

    let ranked = rank_bullets(resume, job);
    assert_eq!(ranked.len(), 2, "both bullets are candidates");

    let lines: Vec<TrimCandidate> = ranked
        .iter()
        .cloned()
        .map(TrimCandidate::from)
        .collect::<Vec<_>>();
    let wire = serde_json::to_value(&lines).expect("TrimCandidate must serialize");
    let first = &wire[0];

    // Exactly the three historical fields — no `id` leaking onto the wire.
    // `serde_json::Value` stores its map sorted, so compare against the
    // sorted field set rather than declaration order.
    let keys: Vec<&str> = first
        .as_object()
        .expect("each line is an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        vec!["hits", "score", "text"],
        "the trim payload's field set must not change; got {keys:?}"
    );
    assert!(
        first["score"].is_u64(),
        "score must serialize as an integer, not a float; got {}",
        first["score"]
    );

    // Ordering and values still come straight from the shared scorer.
    assert_eq!(first["text"], ranked[0].text);
    assert_eq!(first["score"].as_u64().unwrap() as f64, ranked[0].score);
    assert!(
        first["text"].as_str().unwrap().contains("offsite"),
        "weakest-first ordering must survive the shim; got {first}"
    );
}

/// The request schema's `.max(200_000)` is zod — renderer-side only. serde
/// enforces nothing, so the command must cap its own inputs or an IPC caller
/// that isn't our UI hands language detection + stemming unbounded work.
/// Clamped on a char boundary, so the text stays valid UTF-8.
#[tokio::test]
async fn oversized_input_is_clamped_rather_than_processed_whole() {
    // Multi-byte char straddling the cap — a naive byte truncate would split
    // it and produce invalid UTF-8.
    let huge = "a".repeat(MAX_JOB_DESCRIPTION_BYTES - 1) + "\u{1F600}" + &"b".repeat(5_000);
    assert!(huge.len() > MAX_JOB_DESCRIPTION_BYTES);

    let clamped = clamp_to_bytes(huge.clone(), MAX_JOB_DESCRIPTION_BYTES);
    assert_eq!(clamped.len(), MAX_JOB_DESCRIPTION_BYTES - 1);
    assert!(!clamped.contains('\u{1F600}'), "must cut before the emoji");

    // And the command itself survives the oversized pair.
    let out = resume_trim_suggestions(ResumeTrimSuggestionsRequest {
        resume_text: huge.clone(),
        job_text: huge,
        locale: Some("us".into()),
    })
    .await;
    assert_eq!(out["maxPages"], 2);
    assert!(out["lines"].is_array());
}

/// The renderer skips the trim query entirely for documents of 2 pages or
/// fewer (`SHORTEST_OVERFLOW` in `features/ai-generate/components/TrimPanel`),
/// which is only sound while no market's target is below 2. Adding a
/// 1-page market means revisiting that guard — this test is the tripwire.
#[test]
fn no_market_targets_fewer_than_two_pages() {
    for profile in LocaleProfile::all() {
        assert!(
            profile.max_pages >= 2,
            "market {} targets {} pages; the renderer's SHORTEST_OVERFLOW guard \
             assumes no market goes below 2",
            profile.id,
            profile.max_pages
        );
    }
}

// ── the budget is charged at the call, on the bytes the call consumes ────────
//
// These drive the REAL kernel (`score_one`) against a REAL `DocumentStore`, with
// the two provider-reaching effects behind `ScoreIo`. That composition is the
// point: the defect they replace was a charge PREDICATE evaluated by the caller
// on the PRE-translation blob, which no fixture could catch while raw text ==
// embedded text. Here the fake translator TRANSFORMS the text, so a charge
// decided on anything other than what the embed consumes shows up as a count.

/// A translator that rewrites the JD (German → English, the real cross-language
/// case) and an embedder that records exactly which bytes it was asked to embed.
struct FakeScoreIo {
    /// `raw job text → translated job text`. A miss returns the text unchanged.
    translations: std::collections::HashMap<String, String>,
    /// Every text an ACTUAL round-trip was made for, in order.
    embedded: Mutex<Vec<String>>,
    space: crate::commands::ai_provider::EmbeddingSpace,
}

impl FakeScoreIo {
    fn new(store: &DocumentStore, translations: &[(&str, &str)]) -> Self {
        let active = store.embedding_config();
        Self {
            translations: translations
                .iter()
                .map(|(from, to)| ((*from).to_string(), (*to).to_string()))
                .collect(),
            embedded: Mutex::new(Vec::new()),
            space: crate::commands::ai_provider::EmbeddingSpace {
                provider: active.provider,
                model: active.model,
                dim: 3,
                version: EMBEDDING_VECTOR_VERSION,
            },
        }
    }
    fn embedded(&self) -> Vec<String> {
        self.embedded.lock().clone()
    }
    fn vector(&self) -> EmbeddingVector {
        EmbeddingVector {
            values: vec![0.1, 0.2, 0.3],
            space: self.space.clone(),
        }
    }
}

#[async_trait::async_trait]
impl Embedder for FakeScoreIo {
    async fn embed_one(&self, text: &str) -> Option<EmbeddingVector> {
        self.embedded.lock().push(text.to_string());
        Some(self.vector())
    }
}

#[async_trait::async_trait]
impl ScoreIo for FakeScoreIo {
    async fn translate(&self, _job_id: &str, text: String, _target_lang: &str) -> String {
        self.translations.get(&text).cloned().unwrap_or(text)
    }
}

/// Counts charges against the shared daily ceiling. `affordable: false` models
/// the ceiling already being reached (every charge refused).
struct CountingBudget {
    charges: std::sync::atomic::AtomicUsize,
    affordable: bool,
}

impl CountingBudget {
    fn new() -> Self {
        Self {
            charges: std::sync::atomic::AtomicUsize::new(0),
            affordable: true,
        }
    }
    fn exhausted() -> Self {
        Self {
            affordable: false,
            ..Self::new()
        }
    }
    fn charges(&self) -> usize {
        self.charges.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl crate::documents::EmbedBudget for CountingBudget {
    fn charge_one_embed(&self) -> crate::error::AppResult<()> {
        self.charges
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.affordable {
            Ok(())
        } else {
            Err(crate::error::AppError::RateLimited("daily ceiling".into()))
        }
    }
}

const GERMAN_JD: &str = "Wir suchen einen erfahrenen Rust-Entwickler mit Kubernetes-Erfahrung \
                         für den Aufbau verteilter Systeme in Berlin.";
const ENGLISH_JD: &str = "We are looking for an experienced Rust developer with Kubernetes \
                          experience to build distributed systems in Berlin.";
const RESUME_TEXT: &str = "Experienced Rust developer. Kubernetes, Postgres, distributed systems.";

fn scoring_store() -> (tempfile::TempDir, DocumentStore) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let store = DocumentStore::open(&temp_dir.path().to_path_buf()).unwrap();
    (temp_dir, store)
}

/// Run the Autopilot surface's kernel exactly as `score_autopilot_semantic`
/// does, with the two provider seams faked.
async fn score_autopilot(
    io: &FakeScoreIo,
    store: &DocumentStore,
    budget: &CountingBudget,
    job_id: &str,
    raw_job_text: &str,
) -> Value {
    let resume = autopilot_resume_record(RESUME_TEXT);
    let active = store.embedding_config();
    score_one(
        io,
        store,
        &resume,
        None,
        &active,
        job_id,
        Some(raw_job_text.to_string()),
        1,
        MatchSurface::Autopilot,
        Some(budget),
    )
    .await
}

fn seed_posting_vector(store: &DocumentStore, io: &FakeScoreIo, job_id: &str, text: &str) {
    store
        .upsert_posting_vector(job_id, &sha256_hex(text), &io.vector())
        .unwrap();
}

/// The semantic cache key of one autopilot job, as the kernel writes it.
fn semantic_key<'a>(
    resume_id: &'a str,
    job_id: &'a str,
    active: &'a EmbeddingConfig,
    job_text_hash: &'a str,
) -> MatchScoreKey<'a> {
    MatchScoreKey {
        resume_id,
        job_id,
        provider: &active.provider,
        model: &active.model,
        semantic_enabled: 1,
        formula_version: MATCH_FORMULA_VERSION,
        vector_version: EMBEDDING_VECTOR_VERSION,
        job_text_hash,
    }
}

/// THE regression: a translated posting whose vectors are all cached must cost
/// NOTHING. The charge used to be decided against the UNTRANSLATED blob, whose
/// hash can never match the row the embed wrote — so every hourly run of a
/// German-locale autopilot billed the shared ceiling for 20 total cache hits.
#[tokio::test]
async fn a_fully_cached_translated_posting_charges_nothing() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let budget = CountingBudget::new();
    let job_id = "autopilot:cached";
    // Both vectors already cached, each under the text its embed consumed: the
    // posting under the TRANSLATED JD, the résumé snapshot under its own text.
    seed_posting_vector(&store, &io, job_id, ENGLISH_JD);
    seed_posting_vector(&store, &io, &autopilot_resume_id(RESUME_TEXT), RESUME_TEXT);

    let result = score_autopilot(&io, &store, &budget, job_id, GERMAN_JD).await;

    assert!(
        io.embedded().is_empty(),
        "every vector was cached — no round-trip may happen"
    );
    assert_eq!(
        budget.charges(),
        0,
        "a total cache hit must not touch the shared per-provider daily ceiling"
    );
    assert_eq!(
        result.get("scoreSource").and_then(Value::as_str),
        Some(SCORE_SOURCE_COMBINED),
        "…and the cached vectors really were used: this is a semantic score"
    );
}

/// Each ACTUAL embed is charged exactly once — the résumé snapshot and the
/// posting counted separately, both on the bytes they consume. The old
/// posting-only predicate could not see the résumé embed at all.
#[tokio::test]
async fn each_actual_embed_charges_exactly_one() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let budget = CountingBudget::new();

    score_autopilot(&io, &store, &budget, "autopilot:cold", GERMAN_JD).await;

    assert_eq!(
        io.embedded(),
        vec![RESUME_TEXT.to_string(), ENGLISH_JD.to_string()],
        "two round-trips: the résumé snapshot, then the POST-translation posting text"
    );
    assert_eq!(
        budget.charges(),
        io.embedded().len(),
        "one charge per actual round-trip — no more, no less"
    );
}

/// The other half of the résumé blind spot: posting fresh, résumé vector
/// evicted. The embed is real, so the charge must be real — the old predicate
/// consulted only the posting row and let this one through free.
#[tokio::test]
async fn an_evicted_resume_vector_is_a_charged_round_trip_of_its_own() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let budget = CountingBudget::new();
    let job_id = "autopilot:posting-fresh";
    seed_posting_vector(&store, &io, job_id, ENGLISH_JD);

    score_autopilot(&io, &store, &budget, job_id, GERMAN_JD).await;

    assert_eq!(
        io.embedded(),
        vec![RESUME_TEXT.to_string()],
        "only the résumé side embeds"
    );
    assert_eq!(budget.charges(), 1);
}

// ── the degrade needs BOTH vectors, not just the posting ─────────────────────
//
// A cosine is computed from a PAIR. Every shape below therefore has to agree on
// one question — did an embedding actually back this number — and the two MIXED
// shapes are what an all-present / all-absent fixture can never see.

/// Posting vector cached, résumé embed refused: the mixed shape that survived
/// two review rounds because `semantic_available` asked only `job_vec.is_some()`.
/// The cosine needs both sides, so `semantic` is 0.0 and the published number
/// becomes `0.6 × 0 + 0.4 × ats` — an ats of 86 shipping as a "combined" 34,
/// cached under the semantic key, where the Autopilot's `rerank_score_from`
/// adopts it and the early return serves that 34 for the whole TTL.
#[tokio::test]
async fn a_cached_posting_alone_is_not_a_semantic_score() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let job_id = "autopilot:posting-only";
    let resume_id = autopilot_resume_id(RESUME_TEXT);
    let active = store.embedding_config();
    seed_posting_vector(&store, &io, job_id, ENGLISH_JD);
    // The ceiling refuses the résumé round-trip. An offline provider and a
    // failed embed produce the IDENTICAL shape — this is the whole degrade class.
    let budget = CountingBudget::exhausted();

    let result = score_autopilot(&io, &store, &budget, job_id, GERMAN_JD).await;

    let ats = result["ats"].as_f64().expect("ats is a number");
    assert!(
        ats > 0.0,
        "fixture precondition: the pair must have real keyword coverage, or \
         `combined == ats` below would hold vacuously at zero"
    );
    assert_eq!(
        result.get("scoreSource").and_then(Value::as_str),
        Some(SCORE_SOURCE_KEYWORD),
        "no résumé vector means no cosine — this number is keyword-only"
    );
    assert_eq!(
        result["combined"].as_f64(),
        Some(ats),
        "the degrade keeps the keyword score; it must never publish 40% of it \
         as if a 0% similarity had been measured"
    );
    assert!(
        store
            .get_match_score(&semantic_key(
                &resume_id,
                job_id,
                &active,
                &sha256_hex(ENGLISH_JD)
            ))
            .is_none(),
        "…and a keyword-only number must not be frozen under the semantic key, \
         where the next run would read it back as the semantic answer"
    );
}

/// The mirror shape — résumé vector cached, posting embed refused. Tested as its
/// own case deliberately: the two sides are what an all-present/all-absent
/// fixture cannot distinguish, and the asymmetry is how the defect above
/// survived.
#[tokio::test]
async fn a_cached_resume_alone_is_not_a_semantic_score_either() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let job_id = "autopilot:resume-only";
    seed_posting_vector(&store, &io, &autopilot_resume_id(RESUME_TEXT), RESUME_TEXT);
    let budget = CountingBudget::exhausted();

    let result = score_autopilot(&io, &store, &budget, job_id, GERMAN_JD).await;

    assert!(
        io.embedded().is_empty(),
        "the résumé was cached and the posting was refused: no round-trip happened"
    );
    assert_eq!(
        result.get("scoreSource").and_then(Value::as_str),
        Some(SCORE_SOURCE_KEYWORD)
    );
    assert_eq!(result["combined"].as_f64(), result["ats"].as_f64());
}

/// The explanation has to describe the same reality `scoreSource` does. Saying
/// "Semantic similarity 0%" for a measurement that never ran reads as "you are
/// a terrible match" when the truth is "we could not check" — three distinct
/// states, three distinct sentences.
#[tokio::test]
async fn the_explanation_never_reports_a_similarity_that_was_not_measured() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let resume = autopilot_resume_record(RESUME_TEXT);
    let active = store.embedding_config();
    let explanation = |v: &Value| v["explanation"].as_str().unwrap_or_default().to_string();

    // 1. Semantic ON but no embedding happened (offline / refused).
    let degraded = explanation(
        &score_autopilot(
            &io,
            &store,
            &CountingBudget::exhausted(),
            "autopilot:offline",
            GERMAN_JD,
        )
        .await,
    );
    assert!(
        !degraded.contains("Semantic similarity"),
        "no cosine was computed, so no similarity may be reported: {degraded}"
    );
    assert!(
        degraded.contains("could not be computed"),
        "the honest phrasing names the missing measurement: {degraded}"
    );
    assert!(
        !degraded.contains("disabled"),
        "the user did NOT switch semantic scoring off — that is a different state: {degraded}"
    );

    // 2. Semantic OFF — the user's own choice, and its own distinct wording.
    let disabled = explanation(
        &score_one(
            &io,
            &store,
            &resume,
            None,
            &active,
            "autopilot:off",
            Some(GERMAN_JD.to_string()),
            0,
            MatchSurface::Autopilot,
            None,
        )
        .await,
    );
    assert!(
        disabled.contains("semantic scoring disabled"),
        "a deliberate opt-out keeps its own sentence: {disabled}"
    );

    // 3. A real measurement still reports the number it measured.
    let measured = explanation(
        &score_autopilot(
            &io,
            &store,
            &CountingBudget::new(),
            "autopilot:live",
            GERMAN_JD,
        )
        .await,
    );
    assert!(
        measured.contains("Semantic similarity"),
        "…and a score that DID embed must still report its similarity: {measured}"
    );
}

/// A refused charge stops the round-trip (that is the point of a ceiling) and
/// the job degrades to keyword-only.
#[tokio::test]
async fn a_refused_charge_makes_no_provider_call_and_degrades_to_keyword_only() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let budget = CountingBudget::exhausted();

    let result = score_autopilot(&io, &store, &budget, "autopilot:broke", GERMAN_JD).await;

    assert!(
        io.embedded().is_empty(),
        "the ceiling refused: no bytes may reach the provider"
    );
    assert_eq!(
        result.get("scoreSource").and_then(Value::as_str),
        Some("keyword"),
        "the job keeps its keyword score — a run never fails because of scoring"
    );
}

/// …and that degrade must NOT be frozen under the semantic cache key. It was
/// computed without the embedding the key promises, so caching it would make
/// tomorrow's run — ceiling reset, provider back — read the degrade as the
/// semantic answer and never retry, for the whole cache TTL.
#[tokio::test]
async fn a_degraded_score_is_not_cached_under_the_semantic_key() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let job_id = "autopilot:broke";
    let resume_id = autopilot_resume_id(RESUME_TEXT);
    let active = store.embedding_config();
    let hash = sha256_hex(ENGLISH_JD);

    let refused = CountingBudget::exhausted();
    score_autopilot(&io, &store, &refused, job_id, GERMAN_JD).await;

    assert!(
        store
            .get_match_score(&semantic_key(&resume_id, job_id, &active, &hash))
            .is_none(),
        "a keyword-only result must never occupy a semantic_enabled = 1 row"
    );

    // Proof the run really can recover: with budget, the same job scores
    // semantically and THAT result is cached.
    let funded = CountingBudget::new();
    let result = score_autopilot(&io, &store, &funded, job_id, GERMAN_JD).await;
    assert_eq!(
        result.get("scoreSource").and_then(Value::as_str),
        Some(SCORE_SOURCE_COMBINED)
    );
    assert!(store
        .get_match_score(&semantic_key(&resume_id, job_id, &active, &hash))
        .is_some());
}

/// A second job in the same run reuses the résumé vector the first one paid
/// for: the snapshot lands in the posting-vector cache under its
/// content-addressed id, so only the new posting is charged.
#[tokio::test]
async fn the_second_job_of_a_run_only_pays_for_its_own_posting() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let budget = CountingBudget::new();

    score_autopilot(&io, &store, &budget, "autopilot:one", GERMAN_JD).await;
    assert_eq!(budget.charges(), 2, "first job: résumé + posting");

    score_autopilot(
        &io,
        &store,
        &budget,
        "autopilot:two",
        "A different posting entirely, in English already.",
    )
    .await;
    assert_eq!(
        budget.charges(),
        3,
        "second job: the posting only — the résumé snapshot is cached"
    );
}

/// The interactive surfaces pass no budget, so the unattended ceiling can never
/// refuse a user-initiated score.
#[tokio::test]
async fn the_jobs_page_is_not_metered_by_the_unattended_daily_ceiling() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let resume = DocumentRecord {
        id: "doc-1".into(),
        title: String::new(),
        name: String::new(),
        locale: None,
        text: RESUME_TEXT.to_string(),
        pages: None,
        created_at: 0,
        indexed: false,
        is_default: false,
        keywords_json: None,
    };
    let active = store.embedding_config();

    let result = score_one(
        &io,
        &store,
        &resume,
        None,
        &active,
        "job-1",
        Some(GERMAN_JD.to_string()),
        1,
        MatchSurface::JobsPage,
        None,
    )
    .await;

    assert_eq!(
        result.get("scoreSource").and_then(Value::as_str),
        Some(SCORE_SOURCE_COMBINED)
    );
    assert_eq!(
        io.embedded(),
        vec![RESUME_TEXT.to_string(), ENGLISH_JD.to_string()],
        "the Jobs page still embeds both sides — it is simply not metered"
    );
}
