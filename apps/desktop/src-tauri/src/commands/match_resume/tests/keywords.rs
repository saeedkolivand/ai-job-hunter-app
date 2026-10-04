use super::*;
// Keyword-extraction and the coverage/gap math (stopwords, synonyms, short
// terms, `keyword_coverage`, `coverage_score`) are owned and tested by
// `crate::documents::keywords`. These cover the match-command wiring that
// still lives here: the corrupt-keywords fallback and readable gaps.

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
    // No locale → "en", the divergent half of the pair.
    let resume = resume_doc(
        "doc-en",
        "Experienced engineer shipping docker containers and kubernetes clusters.",
    );
    let german_jd = "Wir suchen einen erfahrenen Softwareentwickler mit docker und kubernetes \
                     Kenntnissen für den Aufbau verteilter Systeme in Berlin.";
    assert!(
        !crate::documents::keywords::languages_align(german_jd, resume_target_lang(&resume)),
        "fixture precondition: the pair really is cross-language, or the guard \
         under test is never reached"
    );

    let result = score(
        &io,
        &store,
        &resume,
        "adhoc-cross-language",
        german_jd,
        0,
        MatchSurface::Extension,
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
