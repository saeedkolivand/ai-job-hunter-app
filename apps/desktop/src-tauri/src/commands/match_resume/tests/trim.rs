use super::*;

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
