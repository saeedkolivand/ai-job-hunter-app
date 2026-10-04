use super::*;

// ── the budget is charged at the call, on the bytes the call consumes ────────
//
// These drive the REAL kernel (`score_one`) against a REAL `DocumentStore`, with
// the two provider-reaching effects behind `ScoreIo`. That composition is the
// point: the defect they replace was a charge PREDICATE evaluated by the caller
// on the PRE-translation blob, which no fixture could catch while raw text ==
// embedded text. Here the fake translator TRANSFORMS the text, so a charge
// decided on anything other than what the embed consumes shows up as a count.

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
    let resume = resume_doc("doc-1", RESUME_TEXT);

    let result = score(
        &io,
        &store,
        &resume,
        "job-1",
        GERMAN_JD,
        1,
        MatchSurface::JobsPage,
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
