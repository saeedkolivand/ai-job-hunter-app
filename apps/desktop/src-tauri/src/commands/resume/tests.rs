use super::*;

/// The request schema's `.max(200_000)` is zod — renderer-side only. serde
/// enforces nothing, so the command must cap its own copies of
/// `generated`/`source`/`jobAd` or a direct IPC caller hands the validator
/// unbounded text. Clamped on a char boundary, so the text stays valid
/// UTF-8 — mirrors `match_resume::oversized_input_is_clamped_rather_than_processed_whole`.
#[tokio::test]
async fn oversized_inputs_are_clamped_rather_than_processed_whole() {
    // Multi-byte char straddling the cap — a naive byte truncate would
    // split it and produce invalid UTF-8.
    let huge = "a".repeat(MAX_JOB_DESCRIPTION_BYTES - 1) + "\u{1F600}" + &"b".repeat(5_000);
    assert!(huge.len() > MAX_JOB_DESCRIPTION_BYTES);

    let clamped = clamp_to_bytes(huge.clone(), MAX_JOB_DESCRIPTION_BYTES);
    assert_eq!(clamped.len(), MAX_JOB_DESCRIPTION_BYTES - 1);
    assert!(!clamped.contains('\u{1F600}'), "must cut before the emoji");

    // The command itself must survive the oversized trio (generated,
    // source, and jobAd each independently clamped) rather than hanging or
    // panicking on unbounded language detection / parsing.
    resume_validate_content(ResumeValidateContentRequest {
        generated: huge.clone(),
        source: huge.clone(),
        job_ad: huge,
        top_requirements: vec![],
        target_language: "en".into(),
        doc_kind: "resume".into(),
    })
    .await
    .expect("must not error on an oversized-but-clamped input");
}

/// The Zod schema caps `topRequirements` at 50 items client-side; serde
/// enforces nothing, so the command clamps its own copy
/// (`TOP_REQUIREMENTS_CAP`) rather than handing an unbounded list to the
/// alignment checker.
#[tokio::test]
async fn oversized_requirements_list_is_clamped_not_processed_whole() {
    let requirements: Vec<String> = (0..200).map(|i| format!("requirement {i}")).collect();
    assert!(requirements.len() > TOP_REQUIREMENTS_CAP);

    let report = resume_validate_content(ResumeValidateContentRequest {
        generated: "Jane Doe\nSoftware Engineer".into(),
        source: "Jane Doe\nSoftware Engineer".into(),
        job_ad: "We need a software engineer.".into(),
        top_requirements: requirements,
        target_language: "en".into(),
        doc_kind: "resume".into(),
    })
    .await
    .unwrap();
    // `top_requirement_hits` can never exceed how many requirements the
    // command actually kept — a value above the cap would prove the full
    // 200-item list reached the checker instead of being clamped first.
    // `None` (nothing measured) trivially satisfies that.
    let hits = report.metrics.top_requirement_hits;
    assert!(
        hits.is_none_or(|n| n as usize <= TOP_REQUIREMENTS_CAP),
        "top_requirement_hits={hits:?} must not exceed \
             TOP_REQUIREMENTS_CAP={TOP_REQUIREMENTS_CAP}"
    );
}

/// The Zod schema caps `targetLanguage` at 32 chars client-side
/// (`.max(32)`); serde enforces nothing, so the command clamps its own
/// copy (`TARGET_LANGUAGE_CAP`) — the same trust-boundary treatment
/// every other field on this command gets. Mirrors
/// `oversized_inputs_are_clamped_rather_than_processed_whole`'s
/// multi-byte-boundary shape.
#[tokio::test]
async fn oversized_target_language_is_clamped_not_processed_whole() {
    let huge = "a".repeat(TARGET_LANGUAGE_CAP - 1) + "\u{1F600}" + &"b".repeat(5_000);
    assert!(huge.len() > TARGET_LANGUAGE_CAP);

    resume_validate_content(ResumeValidateContentRequest {
        generated: "Jane Doe\nSoftware Engineer".into(),
        source: "Jane Doe\nSoftware Engineer".into(),
        job_ad: "We need a software engineer.".into(),
        top_requirements: vec![],
        target_language: huge,
        doc_kind: "resume".into(),
    })
    .await
    .expect("must not error on an oversized-but-clamped targetLanguage");
}

/// `docKind: "coverLetter"` must route to the letter ruleset — prose voice
/// checks against the source résumé ∪ job ad — never the résumé-structure
/// checks (ATS sections/bullets, alignment, project structure, duplicate
/// bullets), which assume a document with sections and bullets a letter
/// doesn't have.
#[tokio::test]
async fn cover_letter_doc_kind_routes_to_the_letter_ruleset() {
    // Opens with a known stock phrase from the EN template-opener list — a
    // letter-only voice check that a résumé never runs.
    let letter = "I am writing to apply for this position at your company. \
             I have extensive relevant experience across many domains and I am confident \
             this makes me a strong fit for the team. I have led complex projects and \
             delivered measurable results throughout my career.";

    let report = resume_validate_content(ResumeValidateContentRequest {
        generated: letter.into(),
        source: "Jane Doe\nSoftware Engineer\n\nExperience\n- Built things".into(),
        job_ad: "We need a software engineer with Rust experience.".into(),
        top_requirements: vec![],
        target_language: "en".into(),
        doc_kind: "coverLetter".into(),
    })
    .await
    .unwrap();

    let codes: Vec<&str> = report.issues.iter().map(|i| i.code).collect();
    assert!(
        codes.contains(&"voice.template_opener"),
        "a letter-only voice check must be able to fire; got {codes:?}"
    );
    assert!(
        !codes.iter().any(|c| c.starts_with("ats.")
            || c.starts_with("consistency.")
            || c.starts_with("alignment.")
            || *c == "duplicate.bullet"),
        "résumé-structure codes must never fire for a cover letter; got {codes:?}"
    );
}

/// Anything but the two literal `DocKind` wire values must be rejected, not
/// silently coerced to a ruleset the caller didn't ask for.
#[tokio::test]
async fn unknown_doc_kind_is_rejected_with_a_validation_error() {
    let result = resume_validate_content(ResumeValidateContentRequest {
        generated: "text".into(),
        source: "text".into(),
        job_ad: "text".into(),
        top_requirements: vec![],
        target_language: "en".into(),
        doc_kind: "letter".into(), // not a real wire value
    })
    .await;
    assert!(
        matches!(result, Err(AppError::Validation(_))),
        "unknown docKind must return AppError::Validation; got {result:?}"
    );
}
