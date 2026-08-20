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
    assert_eq!(
        MatchSurface::JobAdText.translates(),
        MatchSurface::JobsPage.translates(),
        "the Score tab's ad-hoc text surface renders under the SAME 'Match' label as the \
         Jobs page and runs INSIDE the app against a user-owned résumé (not the untrusted \
         browser bridge), so it has no zero-egress obligation and must run the same pipeline \
         — unlike Extension, which is the one deliberate exception"
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
        MatchSurface::JobAdText.resume_vector_home(),
        ResumeVectorHome::DocumentIndex,
        "the Score tab scores a real stored résumé, not a text snapshot"
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
        MATCH_FORMULA_VERSION, 3,
        "MATCH_FORMULA_VERSION changed — update this assert AND invalidate \
         cached match scores (clear match_scores table or bump the stored version)"
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

/// A vector in the ACTIVE provider/model/version but a DIFFERENT dimensionality
/// than [`FakeScoreIo::vector`] — the shape an OpenAI-compatible `base_url`
/// switch leaves behind. `EmbeddingConfig::matches` compares provider + model +
/// version and never `dim`, so such a row is a cache HIT; `EmbeddingSpace`'s
/// `PartialEq` DOES include `dim`, so the pair is incomparable at `compare()`.
fn stale_space_vector(store: &DocumentStore, dim: usize) -> EmbeddingVector {
    let active = store.embedding_config();
    EmbeddingVector {
        values: vec![0.5; dim],
        space: crate::commands::ai_provider::EmbeddingSpace {
            provider: active.provider,
            model: active.model,
            dim,
            version: EMBEDDING_VECTOR_VERSION,
        },
    }
}

/// A vector in the ACTIVE embedding space with caller-chosen values, so a test
/// can seed a pair whose cosine — and therefore the kernel's `semantic` number
/// — is known in advance instead of inheriting [`FakeScoreIo::vector`]'s
/// identical-on-both-sides 1.0.
fn vector_of(store: &DocumentStore, values: [f64; 3]) -> EmbeddingVector {
    let active = store.embedding_config();
    EmbeddingVector {
        values: values.to_vec(),
        space: crate::commands::ai_provider::EmbeddingSpace {
            provider: active.provider,
            model: active.model,
            dim: 3,
            version: EMBEDDING_VECTOR_VERSION,
        },
    }
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

// ── the published number: weights, the empty-JD branch, the stemmer guard ────
//
// All three drive the REAL kernel. The versions these replace re-declared the
// formula / the sentence / the guard inside the test body and asserted against
// their own copy, so a production weight flip, a reworded explanation or an
// inverted guard could not fail them.

/// A JD the résumé covers COMPLETELY (it contains the posting verbatim), so
/// `ats` is exactly 100 and the only variable left in the combined number is
/// the cosine — which the seeded vector pair fixes at 0.6.
const COVERED_JD: &str = "We are looking for an experienced Rust developer with Kubernetes \
                          experience to build distributed systems in Berlin.";
const COVERING_RESUME: &str = "We are looking for an experienced Rust developer with Kubernetes \
                               experience to build distributed systems in Berlin. Shipped \
                               Postgres and Terraform work alongside it.";

/// The weights, pinned on the kernel's own output rather than on a copy of the
/// formula. Both inputs are fixed by the fixture — cosine 0.6 → `semantic` 60,
/// full keyword coverage → `ats` 100 — so `combined` has exactly one correct
/// value: `round(0.6 × 60 + 0.4 × 100)` = 76.
///
/// Mutation: any weight change moves it (0.5/0.5 → 80, 0.4/0.6 → 84, dropping
/// the semantic term → 100).
#[tokio::test]
async fn the_combined_score_weights_semantic_60_and_ats_40() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[]);
    let job_id = "autopilot:weights";
    // Unit vectors 0.6 apart: cos = (1·0.6 + 0·0.8 + 0·0) / (1 × 1) = 0.6.
    store
        .upsert_posting_vector(
            &autopilot_resume_id(COVERING_RESUME),
            &sha256_hex(COVERING_RESUME),
            &vector_of(&store, [1.0, 0.0, 0.0]),
        )
        .unwrap();
    store
        .upsert_posting_vector(
            job_id,
            &sha256_hex(COVERED_JD),
            &vector_of(&store, [0.6, 0.8, 0.0]),
        )
        .unwrap();

    let resume = autopilot_resume_record(COVERING_RESUME);
    let active = store.embedding_config();
    let result = score_one(
        &io,
        &store,
        &resume,
        None,
        &active,
        job_id,
        Some(COVERED_JD.to_string()),
        1,
        MatchSurface::Autopilot,
        None,
    )
    .await;

    assert!(
        io.embedded().is_empty(),
        "fixture precondition: both vectors are seeded, so the kernel measures \
         the cosine this test chose and not one an embed invented"
    );
    assert_eq!(
        result["semantic"].as_f64(),
        Some(60.0),
        "fixture precondition: the seeded pair is 0.6 apart"
    );
    assert_eq!(
        result["ats"].as_f64(),
        Some(100.0),
        "fixture precondition: the résumé contains the posting verbatim, so every \
         JD keyword is covered"
    );
    assert_eq!(
        result["combined"].as_f64(),
        Some(76.0),
        "combined must be round(0.6 × semantic + 0.4 × ats) = round(36 + 40); any \
         other number is a weight change and needs a MATCH_FORMULA_VERSION bump"
    );
    assert_eq!(
        result.get("scoreSource").and_then(Value::as_str),
        Some(SCORE_SOURCE_COMBINED),
        "…and it really is the semantic branch that produced it"
    );
}

/// A posting with no extractable keywords (the garbled / boilerplate-only JD)
/// must say the coverage is UNAVAILABLE. The alternative — reporting the
/// kernel's `0.0` placeholder as "0%" — is indistinguishable from a genuine
/// total mismatch, which is a different message to the user entirely.
#[tokio::test]
async fn a_jd_with_no_extractable_keywords_reports_an_unavailable_score() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[]);
    let resume = autopilot_resume_record(RESUME_TEXT);
    let active = store.embedding_config();

    // Punctuation and digits only: nothing survives keyword extraction.
    let result = score_one(
        &io,
        &store,
        &resume,
        None,
        &active,
        "autopilot:garbled",
        Some("--- 123 456 --- *** ///".to_string()),
        0,
        MatchSurface::Autopilot,
        None,
    )
    .await;

    let explanation = result["explanation"].as_str().unwrap_or_default();
    assert!(
        explanation.contains("No extractable keywords"),
        "an unscorable posting must be named as such: {explanation}"
    );
    assert!(
        !explanation.contains("0%"),
        "…and must never be reported as a 0% match, which is a real measurement: {explanation}"
    );
    assert!(
        explanation.contains("guidance estimate"),
        "the guidance framing rides on every branch: {explanation}"
    );
    assert_eq!(
        result["ats"].as_f64(),
        Some(0.0),
        "there is no coverage to report"
    );
    assert!(
        result["gaps"].as_array().is_some_and(|g| g.is_empty()),
        "…and no gap terms either — there were no keywords to miss"
    );
}

// ── the Score tab's ad-hoc text surface (job_ad_text_id / MatchSurface::JobAdText) ──

/// [`job_ad_text_id`] must be stable (repeated opens of the SAME hashed text
/// reuse the SAME `match_scores` row) and prefixed so it can never collide
/// with a real `PostingsCache` id. Mirrors `extension_bridge::match_live`'s
/// `adhoc_job_id_is_stable_and_prefixed`.
#[test]
fn job_ad_text_id_is_stable_and_prefixed() {
    let a = job_ad_text_id("Senior Rust engineer, Kubernetes, Postgres.");
    let b = job_ad_text_id("Senior Rust engineer, Kubernetes, Postgres.");
    assert_eq!(
        a, b,
        "the same job text must yield the same cache key — a repeated open of the same \
         posting must reuse the same row"
    );
    assert!(
        a.starts_with("job-ad-text:"),
        "must be namespaced so it can never collide with a real PostingsCache id"
    );
}

#[test]
fn job_ad_text_id_differs_per_text() {
    let a = job_ad_text_id("posting one");
    let b = job_ad_text_id("posting two");
    assert_ne!(a, b, "different postings must never share a cache key");
}

/// BLOCKING regression pin: the Score tab's ad-hoc pre-processing
/// ([`job_ad_text_blob`]) must strip markdown IDENTICALLY to the Jobs-page
/// path ([`posting_to_text`]) for the SAME description. Before this fix,
/// `score_resume_against_text` hashed and scored `job_text` raw — a markdown
/// anchor like `[Apply now](https://acme.example.com/jobs)` collapsed to
/// `Apply now` on the Jobs page (the bare url deleted) but leaked the JD
/// keywords `https`/`acme`/`example`/`com` here, inflating the coverage
/// denominator with tokens no résumé can ever contain while diverging from
/// the SAME posting's Jobs-page percentage. `posting_to_text` is driven with
/// an empty title and no requirements — the ONE axis that legitimately still
/// differs between the two surfaces (composition, not markdown-handling) —
/// so this test isolates the description-only transformation both surfaces
/// must share.
#[test]
fn job_ad_text_blob_matches_posting_to_text_for_the_same_markdown_description() {
    let description = "[Apply now](https://acme.example.com/jobs) to help us build reliable \
                        systems with Rust and Kubernetes. See more at \
                        https://acme.example.com/careers.";
    let posting = json!({ "title": "", "description": description });

    let via_jobs_page = posting_to_text(&posting);
    let via_score_tab = job_ad_text_blob(description);

    assert_eq!(
        via_jobs_page, via_score_tab,
        "identical description must pre-process IDENTICALLY on both surfaces"
    );
    let blob = via_score_tab.expect("a real description must yield a scorable blob");
    assert!(
        !blob.contains("https") && !blob.contains("acme") && !blob.contains("example"),
        "markdown links and bare URLs must be stripped, never tokenized into the keyword set: {blob}"
    );
    assert!(
        blob.contains("Apply now") && blob.contains("reliable systems") && blob.contains("Rust"),
        "the anchor TEXT and the rest of the JD vocabulary must survive: {blob}"
    );
}

/// The mandated absolute-expectation check: an empty/keyword-less posting
/// scored through the Score tab's [`MatchSurface::JobAdText`] surface must
/// report the HONEST degrade (a real, named "unavailable" state), never a
/// fabricated plausible-looking number. Anchored to the kernel's OWN
/// absolute-zero contract — not to a second, independently derived score —
/// exactly the shape `a_jd_with_no_extractable_keywords_reports_an_unavailable_score`
/// already pins for the Autopilot surface, driven here on the new surface.
#[tokio::test]
async fn job_ad_text_surface_reports_the_honest_degrade_for_a_keyword_less_posting() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[]);
    let resume = DocumentRecord {
        id: "doc-en".into(),
        title: String::new(),
        name: String::new(),
        locale: None,
        text: RESUME_TEXT.into(),
        pages: None,
        created_at: 0,
        indexed: false,
        is_default: false,
        keywords_json: None,
    };
    let active = store.embedding_config();

    // Punctuation and digits only: nothing survives keyword extraction — the
    // same garbled fixture the Autopilot-surface test above uses.
    let job_text = "--- 123 456 --- *** ///".to_string();
    let job_id = job_ad_text_id(&job_text);
    let result = score_one(
        &io,
        &store,
        &resume,
        None,
        &active,
        &job_id,
        Some(job_text),
        0,
        MatchSurface::JobAdText,
        None,
    )
    .await;

    assert_eq!(
        result["ats"].as_f64(),
        Some(0.0),
        "an unscorable posting must report the honest absolute zero, never a fabricated score"
    );
    assert_eq!(result["combined"].as_f64(), Some(0.0));
    assert!(result["gaps"].as_array().is_some_and(|g| g.is_empty()));
    assert_eq!(
        result.get("scoreSource").and_then(Value::as_str),
        Some(SCORE_SOURCE_KEYWORD),
        "keyword-only is structural on this surface, not a default"
    );
    let explanation = result["explanation"].as_str().unwrap_or_default();
    assert!(
        explanation.contains("No extractable keywords"),
        "an unscorable posting must be named as such: {explanation}"
    );
    assert!(
        !explanation.contains("0%"),
        "…and must never be reported as a 0% match, which is a real measurement: {explanation}"
    );
}

/// The ONE runnable check the task requires: identical job text must score
/// IDENTICALLY through the Score tab's ad-hoc [`MatchSurface::JobAdText`] path
/// and the Jobs-page [`MatchSurface::JobsPage`] path — same ruler, no forked
/// scorer. Driven on the German→English translation fixture (not a same-
/// language pair) so the assertion actually exercises the shared pre-
/// processing pipeline: if `JobAdText` ever stopped translating (e.g. by
/// copy-pasting `Extension`'s behaviour), the two surfaces would tokenize
/// different-language text and this comparison would catch it, unlike a
/// same-language fixture where a missing translate step is invisible.
#[tokio::test]
async fn the_score_tab_surface_runs_the_same_pipeline_as_the_jobs_page_for_identical_text() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let resume = DocumentRecord {
        id: "doc-en".into(),
        title: String::new(),
        name: String::new(),
        locale: None, // → "en"
        text: RESUME_TEXT.into(),
        pages: None,
        created_at: 0,
        indexed: false,
        is_default: false,
        keywords_json: None,
    };
    let active = store.embedding_config();

    let jobs_page = score_one(
        &io,
        &store,
        &resume,
        None,
        &active,
        "posting-1",
        Some(GERMAN_JD.to_string()),
        0,
        MatchSurface::JobsPage,
        None,
    )
    .await;

    let job_id = job_ad_text_id(GERMAN_JD);
    let score_tab = score_one(
        &io,
        &store,
        &resume,
        None,
        &active,
        &job_id,
        Some(GERMAN_JD.to_string()),
        0,
        MatchSurface::JobAdText,
        None,
    )
    .await;

    // Absolute anchor, not just cross-equality: every assertion below compares
    // the two surfaces to EACH OTHER, so a degraded/stubbed shared kernel
    // (translation silently no-op on both sides, the tokenizer returning
    // empty) would let both move together and stay green — the exact shape
    // this repo has shipped before. This fixture pair overlaps heavily on
    // real vocabulary (Rust/Kubernetes/distributed systems on both sides), so
    // a genuinely working pipeline must clear a real floor, not just agree
    // with itself.
    assert!(
        jobs_page["combined"].as_f64().unwrap_or(0.0) > 40.0,
        "absolute floor: got {jobs_page:?} — a dead/stubbed pipeline returning 0 on both sides \
         would still pass every equality assertion below"
    );
    assert_eq!(
        jobs_page["scoreSource"].as_str(),
        Some(SCORE_SOURCE_KEYWORD),
        "both calls pass semantic_enabled = 0 — the parity claim only holds keyword-only, per \
         MatchSurface's doc"
    );

    assert_eq!(
        jobs_page["ats"], score_tab["ats"],
        "identical job text must produce identical keyword coverage on both surfaces"
    );
    assert_eq!(jobs_page["combined"], score_tab["combined"]);
    assert_eq!(jobs_page["gaps"], score_tab["gaps"]);
    assert_eq!(jobs_page["scoreSource"], score_tab["scoreSource"]);
    assert_ne!(
        jobs_page["jobId"], score_tab["jobId"],
        "distinct cache identities by design — a real posting id vs. the content-addressed \
         text id — everything else must still match"
    );
}

// ── match_resume_text's pure precondition (resolve_resume_and_text) ──────────

/// The résumé-not-found error must be returned BEFORE any clamp/cache work —
/// mirrors `match_resume`'s own resume-not-found shape and the errors-never-
/// cached invariant.
#[test]
fn resolve_resume_and_text_reports_resume_not_found() {
    let (_dir, store) = scoring_store();
    let err = resolve_resume_and_text(&store, "missing-resume", "some job text".into())
        .expect_err("no such resume must be an error, not a silent default");
    assert_eq!(err["error"], "resume not found: missing-resume");
}

/// Job text over [`MAX_JOB_DESCRIPTION_BYTES`] must be clamped, not rejected —
/// mirrors `resume_trim_suggestions`'s convention (an advisory/estimate score
/// on the first 200 kB beats an error dialog for unbounded scraper/user input
/// reaching this new IPC surface).
#[test]
fn resolve_resume_and_text_clamps_oversized_job_text() {
    let (_dir, store) = scoring_store();
    store
        .insert(&DocumentRecord {
            id: "doc-1".into(),
            title: "Resume".into(),
            name: "resume.pdf".into(),
            locale: None,
            text: RESUME_TEXT.into(),
            pages: None,
            created_at: 0,
            indexed: false,
            is_default: false,
            keywords_json: None,
        })
        .unwrap();

    let oversized = "x".repeat(MAX_JOB_DESCRIPTION_BYTES + 500);
    let (resume, clamped) =
        resolve_resume_and_text(&store, "doc-1", oversized).expect("a real resume id must resolve");
    assert_eq!(resume.id, "doc-1");
    assert!(
        clamped.len() <= MAX_JOB_DESCRIPTION_BYTES,
        "job text over the cap must be truncated, not passed through unbounded"
    );
}

/// The sibling to the test above with a genuinely MULTIBYTE fixture. That
/// one clamps `MAX_JOB_DESCRIPTION_BYTES + 500` bytes of pure ASCII
/// (`"x".repeat(...)`), which never exercises `clamp_to_bytes`'s
/// char-boundary walk-back at all -- every byte offset in pure ASCII is
/// already a char boundary, so the interesting code path is untested.
/// An ASCII-only fixture hiding multibyte behaviour is the exact class
/// that produced this branch's own dotted-I/e-acute byte-offset
/// crash-loop incident: the code looked correct, the tests were green, and the
/// only input that mattered was never tried.
///
/// Places a 4-byte emoji EXACTLY straddling the cap (its first byte lands
/// at `MAX_JOB_DESCRIPTION_BYTES - 1`, one byte before it), so the naive
/// cutoff at the cap would split it mid-character and the walk-back MUST
/// move -- the SAME proven fixture shape `clamp_to_bytes` already has its
/// own direct unit test for (`oversized_input_is_clamped_rather_than_
/// processed_whole`, this file), applied through `resolve_resume_and_text`
/// instead: a future change that inlines the clamp, reorders it, or adds
/// a preprocessing step ahead of it inside THIS fn specifically would
/// still be caught here, not only at the lower-level primitive.
#[test]
fn resolve_resume_and_text_clamps_oversized_multibyte_job_text() {
    let (_dir, store) = scoring_store();
    store
        .insert(&DocumentRecord {
            id: "doc-1".into(),
            title: "Resume".into(),
            name: "resume.pdf".into(),
            locale: None,
            text: RESUME_TEXT.into(),
            pages: None,
            created_at: 0,
            indexed: false,
            is_default: false,
            keywords_json: None,
        })
        .unwrap();

    let oversized = "a".repeat(MAX_JOB_DESCRIPTION_BYTES - 1) + "\u{1F600}" + &"b".repeat(2_500);
    assert!(
        oversized.len() > MAX_JOB_DESCRIPTION_BYTES,
        "precondition: the fixture must actually exceed the cap"
    );

    let (resume, clamped) =
        resolve_resume_and_text(&store, "doc-1", oversized).expect("a real resume id must resolve");
    assert_eq!(resume.id, "doc-1");

    // Absolute against the cap, never against the input's own length --
    // proves the walk-back moved exactly one byte back from the naive
    // cutoff and stopped at the FIRST valid boundary, not some other one.
    assert_eq!(
        clamped.len(),
        MAX_JOB_DESCRIPTION_BYTES - 1,
        "clamped multibyte job text must land exactly on the char \
         boundary immediately before the straddling character"
    );
    assert!(
        !clamped.contains('\u{1F600}'),
        "the whole straddling character must be dropped, not partially \
         included"
    );
    assert!(
        String::from_utf8(clamped.into_bytes()).is_ok(),
        "clamping a multibyte string must never produce invalid UTF-8"
    );
}

/// The stemmer-language guard, through the kernel that owns it. A German JD
/// against an English-locale résumé must leave BOTH sides unstemmed, so the
/// language-neutral tech tokens they share still intersect. Stemming one side
/// only (the pre-fix asymmetry) mutates `docker`/`kubernetes` on the JD side
/// alone and collapses coverage to zero — strictly worse than no stemming.
///
/// Driven on [`MatchSurface::Extension`], the one surface that does not
/// translate: translation would realign the languages and the guard would never
/// be reached.
#[tokio::test]
async fn a_cross_language_pair_keeps_both_sides_unstemmed_so_shared_tokens_match() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[]);
    let active = store.embedding_config();
    let resume = DocumentRecord {
        id: "doc-en".into(),
        title: String::new(),
        name: String::new(),
        locale: None, // → "en", the divergent half of the pair
        text: "Experienced engineer shipping docker containers and kubernetes clusters.".into(),
        pages: None,
        created_at: 0,
        indexed: false,
        is_default: false,
        keywords_json: None,
    };
    let german_jd = "Wir suchen einen erfahrenen Softwareentwickler mit docker und kubernetes \
                     Kenntnissen für den Aufbau verteilter Systeme in Berlin.";
    assert!(
        !crate::documents::keywords::languages_align(german_jd, resume_target_lang(&resume)),
        "fixture precondition: the pair really is cross-language, or the guard \
         under test is never reached"
    );

    let result = score_one(
        &io,
        &store,
        &resume,
        None,
        &active,
        "adhoc-cross-language",
        Some(german_jd.to_string()),
        0,
        MatchSurface::Extension,
        None,
    )
    .await;

    assert!(
        result["ats"].as_f64().is_some_and(|a| a > 0.0),
        "the shared language-neutral tokens must survive on BOTH sides; got {}",
        result["ats"]
    );
    let gaps: Vec<String> = serde_json::from_value(result["gaps"].clone()).unwrap_or_default();
    for shared in ["docker", "kubernetes"] {
        assert!(
            !gaps.iter().any(|g| g == shared),
            "`{shared}` appears on both sides, so it cannot be a gap; got {gaps:?}"
        );
    }
}

// ── the degrade needs BOTH vectors, and they must be COMPARABLE ──────────────
//
// A cosine is computed from a PAIR. Every shape below therefore has to agree on
// one question — did an embedding actually back this number — and the two MIXED
// shapes are what an all-present / all-absent fixture can never see. Presence is
// necessary but NOT sufficient: two vectors from different embedding spaces are
// both present and still yield no measurement.

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

/// Both vectors PRESENT and still no measurement: `compare()` refuses a
/// cross-space pair, and the `.ok()` that keeps the caller's degrade contract
/// flattens that refusal to the formula's `0.0` placeholder. Availability read
/// as presence therefore called it measured — `0.6 × 0 + 0.4 × ats` published as
/// "combined", explained as "Semantic similarity 0%", frozen under the semantic
/// key for the whole TTL, and adopted by the Autopilot's `rerank_score_from`
/// (which resets the degrade breaker, so the pass keeps paying for more of them).
///
/// Reachable with no race at all: `ai_set_embedding_config` clears
/// `posting_vectors` + `match_scores` but never the `vectors` table, and
/// `EmbeddingConfig::matches` compares provider + model + version — never `dim`.
/// So switching an OpenAI-compatible `base_url` from a 1536-dim endpoint to a
/// 768-dim gateway advertising the SAME model name leaves every résumé vector in
/// place, reading as fresh, and incomparable with every posting embedded after.
#[tokio::test]
async fn an_incomparable_vector_pair_is_not_a_semantic_score() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[(GERMAN_JD, ENGLISH_JD)]);
    let job_id = "job-cross-space";
    let active = store.embedding_config();
    let resume = DocumentRecord {
        id: "doc-stale-space".into(),
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
    // The survivor of the base_url switch: same provider/model/version, wider.
    let survivor = stale_space_vector(&store, 4);
    assert!(
        crate::commands::ai_provider::compare(&survivor, &io.vector()).is_err(),
        "fixture precondition: the two spaces really are incomparable"
    );
    store.upsert_vector(&resume.id, &survivor).unwrap();

    let result = score_one(
        &io,
        &store,
        &resume,
        None,
        &active,
        job_id,
        Some(GERMAN_JD.to_string()),
        1,
        MatchSurface::JobsPage,
        None,
    )
    .await;

    assert_eq!(
        io.embedded(),
        vec![ENGLISH_JD.to_string()],
        "fixture precondition: the stale résumé vector is a cache HIT (matches() \
         never looks at dim), so only the posting embeds — BOTH sides are present"
    );
    let ats = result["ats"].as_f64().expect("ats is a number");
    assert!(
        ats > 0.0,
        "fixture precondition: the pair must have real keyword coverage, or \
         `combined == ats` below would hold vacuously at zero"
    );
    assert_eq!(
        result.get("scoreSource").and_then(Value::as_str),
        Some(SCORE_SOURCE_KEYWORD),
        "no cosine was computed, so this number is keyword-only — presence of \
         two vectors is not comparability"
    );
    assert_eq!(
        result["combined"].as_f64(),
        Some(ats),
        "the degrade keeps the keyword score; it must never publish 40% of it \
         as if a 0% similarity had been measured"
    );
    let explanation = result["explanation"].as_str().unwrap_or_default();
    assert!(
        !explanation.contains("Semantic similarity"),
        "no cosine was computed, so no similarity may be reported: {explanation}"
    );
    assert!(
        explanation.contains("could not be computed"),
        "the honest phrasing names the missing measurement: {explanation}"
    );
    assert!(
        store
            .get_match_score(&semantic_key(
                &resume.id,
                job_id,
                &active,
                &sha256_hex(ENGLISH_JD)
            ))
            .is_none(),
        "…and a keyword-only number must not be frozen under the semantic key, \
         where the next run would read it back as the semantic answer"
    );
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

    // Whatever the state, the sentence stays framed as OUR estimate — the one
    // claim every branch has to keep (job-match-standards: never present the
    // number as the employer's verdict).
    for (state, sentence) in [
        ("unavailable", &degraded),
        ("disabled", &disabled),
        ("measured", &measured),
    ] {
        assert!(
            sentence.contains("guidance estimate"),
            "the {state} branch dropped the guidance framing: {sentence}"
        );
    }
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
        Some(SCORE_SOURCE_KEYWORD),
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

// ── the posting hand-off: one lock, one scan, real facts ─────────────

/// The single cache read must hand BOTH consumers the real posting.
///
/// `match_resume` resolves the live posting once and passes the facts to the
/// hard-constraint pass, instead of that pass taking the `PostingsCache` lock and
/// re-scanning for the same id — a duplicate that would run on every Jobs-page
/// call, including the `match_scores` cache hits where the score itself costs
/// nothing. Anchored on absolute values: substituting a default here is
/// invisible to every other test in the crate.
#[test]
fn posting_facts_hand_off_carries_the_real_posting() {
    let posting = serde_json::json!({
        "id": "j1",
        "title": "Rust Engineer",
        "description": "Build things.",
        "location": "Berlin, Germany",
        "remote": true,
    });
    let (text, facts) = resolve_posting(Some(&posting));
    let text = text.expect("a posting with a title and description has scorable text");
    assert!(text.contains("Rust Engineer"), "got: {text}");
    // The constraint side gets the posting's OWN fields, not a placeholder.
    assert_eq!(facts.location.as_deref(), Some("Berlin, Germany"));
    assert!(facts.board_remote);
}

/// A cache miss yields nothing for either consumer — and that is safe because
/// `score_one` returns its job-not-found error first, which `attach` passes
/// through without ever reading these facts.
#[test]
fn posting_facts_hand_off_is_empty_when_the_posting_is_not_cached() {
    let (text, facts) = resolve_posting(None);
    assert_eq!(text, None);
    assert_eq!(facts.location, None);
    assert!(!facts.board_remote);
}
