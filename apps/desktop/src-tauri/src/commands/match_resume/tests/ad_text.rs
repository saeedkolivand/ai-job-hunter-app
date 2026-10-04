use super::*;

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
    let resume = resume_doc("doc-en", RESUME_TEXT);

    // Punctuation and digits only: nothing survives keyword extraction — the
    // same garbled fixture the Autopilot-surface test above uses.
    let job_text = "--- 123 456 --- *** ///".to_string();
    let job_id = job_ad_text_id(&job_text);
    let result = score(
        &io,
        &store,
        &resume,
        &job_id,
        &job_text,
        0,
        MatchSurface::JobAdText,
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

/// The Score tab's `semantic_enabled` bit now reaches [`score_one`] and flips
/// it exactly like the Jobs page's does — [`MatchSurface::JobAdText`] is no
/// longer wired to a hardcoded `0`. Driven directly against the kernel, the
/// SAME convention `the_score_tab_surface_runs_the_same_pipeline_as_the_jobs_page_for_identical_text`
/// below uses: `score_resume_against_text`/`match_resume_text` are thin
/// `AppHandle` wrappers with no test harness in this crate, so the flag's
/// effect is pinned where it actually branches.
#[tokio::test]
async fn the_score_tab_semantic_flag_flips_score_source_when_a_real_embedding_pair_exists() {
    let (_dir, store) = scoring_store();
    let io = FakeScoreIo::new(&store, &[]);
    let resume = resume_doc("doc-en", RESUME_TEXT);
    let job_text =
        "Senior Rust engineer with Kubernetes and distributed systems experience.".to_string();
    let job_id = job_ad_text_id(&job_text);

    let result = score(
        &io,
        &store,
        &resume,
        &job_id,
        &job_text,
        1, // semantic_enabled — the flag this surface used to hardcode to 0
        MatchSurface::JobAdText,
    )
    .await;

    assert_eq!(
        result.get("scoreSource").and_then(Value::as_str),
        Some(SCORE_SOURCE_COMBINED),
        "a caller-supplied semantic_enabled=1 with a real embedding pair must produce a \
         combined score on THIS surface too: {result:?}"
    );
}

/// Mutation-pin for the `cacheable = skip_semantic || semantic_available`
/// invariant [`score_one`]'s own doc states, exercised on the newly-reachable
/// JobAdText + semantic path: a degraded embed (offline provider / failed
/// round-trip) must still report `scoreSource: "keyword"`, named as a
/// degrade, never a fabricated `combined` — AND it must NOT be cached under
/// the `semantic_enabled = 1` key. A second call once the provider recovers
/// re-attempts the embed and gets the real answer; if the degrade had been
/// wrongly cached, this would instead read back the frozen `keyword` result
/// forever (the exact bug `score_one`'s `cacheable` comment names).
#[tokio::test]
async fn the_score_tab_degraded_embed_is_keyword_only_and_not_cached_under_the_semantic_key() {
    let (_dir, store) = scoring_store();
    let resume = resume_doc("doc-en", RESUME_TEXT);
    let job_text =
        "Senior Rust engineer with Kubernetes and distributed systems experience.".to_string();
    let job_id = job_ad_text_id(&job_text);

    // First call: the provider is offline — every embed attempt fails.
    let offline = FakeScoreIo::new(&store, &[]).failing();
    let degraded = score(
        &offline,
        &store,
        &resume,
        &job_id,
        &job_text,
        1,
        MatchSurface::JobAdText,
    )
    .await;
    assert_eq!(
        degraded.get("scoreSource").and_then(Value::as_str),
        Some(SCORE_SOURCE_KEYWORD),
        "no embedding was available — this must degrade, never fabricate combined: {degraded:?}"
    );
    let explanation = degraded["explanation"].as_str().unwrap_or_default();
    assert!(
        explanation.contains("could not be computed"),
        "the degrade must be named, not silently swapped for the disabled-scoring copy: \
         {explanation}"
    );

    // Second call, SAME job/résumé identity, provider now back online. If the
    // degraded result above had been wrongly cached under the
    // semantic_enabled=1 key, this would read it back and stay 'keyword' forever.
    let online = FakeScoreIo::new(&store, &[]);
    let recovered = score(
        &online,
        &store,
        &resume,
        &job_id,
        &job_text,
        1,
        MatchSurface::JobAdText,
    )
    .await;
    assert_eq!(
        recovered.get("scoreSource").and_then(Value::as_str),
        Some(SCORE_SOURCE_COMBINED),
        "a degraded run must not poison the semantic cache row — the next run must retry and \
         get the real answer: {recovered:?}"
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
    // No locale → "en".
    let resume = resume_doc("doc-en", RESUME_TEXT);

    let jobs_page = score(
        &io,
        &store,
        &resume,
        "posting-1",
        GERMAN_JD,
        0,
        MatchSurface::JobsPage,
    )
    .await;

    let job_id = job_ad_text_id(GERMAN_JD);
    let score_tab = score(
        &io,
        &store,
        &resume,
        &job_id,
        GERMAN_JD,
        0,
        MatchSurface::JobAdText,
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
