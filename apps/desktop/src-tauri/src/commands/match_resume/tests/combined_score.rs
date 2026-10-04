use super::*;

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
    let result = score(
        &io,
        &store,
        &resume,
        job_id,
        COVERED_JD,
        1,
        MatchSurface::Autopilot,
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

    // Punctuation and digits only: nothing survives keyword extraction.
    let result = score(
        &io,
        &store,
        &resume,
        "autopilot:garbled",
        "--- 123 456 --- *** ///",
        0,
        MatchSurface::Autopilot,
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
    let resume = resume_doc("doc-stale-space", RESUME_TEXT);
    // The survivor of the base_url switch: same provider/model/version, wider.
    let survivor = stale_space_vector(&store, 4);
    assert!(
        crate::commands::ai_provider::compare(&survivor, &io.vector()).is_err(),
        "fixture precondition: the two spaces really are incomparable"
    );
    store.upsert_vector(&resume.id, &survivor).unwrap();

    let result = score(
        &io,
        &store,
        &resume,
        job_id,
        GERMAN_JD,
        1,
        MatchSurface::JobsPage,
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
        &score(
            &io,
            &store,
            &resume,
            "autopilot:off",
            GERMAN_JD,
            0,
            MatchSurface::Autopilot,
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
