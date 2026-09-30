#[test]
fn resolve_job_returns_the_looked_up_ad_and_meta_on_a_cache_hit() {
    let choice = super::super::resolve::JobSource::Cache("job-9");
    let (ad, meta) = super::super::resolve::resolve_job(
        choice,
        |id| (id == "job-9").then(|| "JOB AD TEXT".to_string()),
        |id| {
            (id == "job-9").then(|| crate::commands::match_resume::JobPostingMeta {
                title: "Staff Engineer".to_string(),
                ..Default::default()
            })
        },
        || panic!("meta_from_request must never run on the Cache arm"),
    )
    .expect("a hit resolves");
    assert_eq!(ad, "JOB AD TEXT");
    assert_eq!(meta.title, "Staff Engineer");
}

/// The job-ad twin of `resolve_resume_errors_on_a_store_miss_…`.
#[test]
fn resolve_job_errors_on_a_cache_miss_even_though_the_request_also_carried_text() {
    let choice =
        super::super::job_source("job-9", "a whole job ad text").expect("an id is present");
    let err = super::super::resolve::resolve_job(
        choice,
        |_| None,
        |_| None,
        || panic!("meta_from_request must never run on the Cache arm"),
    )
    .expect_err(
        "a Cache miss must error, never silently read the text job_source saw but discarded",
    );
    assert!(matches!(err, crate::error::AppError::Validation(_)));
}

/// `sanitize_job_meta` blanks an implausible `company` to `""`, the same
/// "company not known" convention every existing downstream consumer already
/// treats an empty string as. Anchored to the CONCRETE resulting `Option`
/// `research_company_brief`'s own admission expression produces
/// (`(!company.is_empty()).then_some(company)`, `pipeline/resume/stages/
/// cover_letter.rs`), not merely to `is_implausible_company`'s own bool — a
/// garbage company must never reach `CompanyResearch`, and therefore never
/// reach the model's "why this company" paragraph.
///
/// Mutation check: remove the `if crate::scraping::trust::is_implausible_company(…)`
/// guard from `sanitize_job_meta` (return `meta` unchanged) — this test fails
/// immediately (`admitted` becomes `Some("Apply now | LinkedIn")`).
#[test]
fn sanitize_job_meta_blanks_an_implausible_company_before_it_can_reach_company_research() {
    let garbage = crate::commands::match_resume::JobPostingMeta {
        company: "Apply now | LinkedIn".to_string(),
        title: "Staff Engineer".to_string(),
        url: "https://boards.example/jobs/1".to_string(),
        board: "linkedin".to_string(),
    };
    let sanitized = super::super::resolve::sanitize_job_meta(garbage);
    assert_eq!(sanitized.company, "");
    // `title`/`url`/`board` must survive untouched — only `company` is in scope.
    assert_eq!(sanitized.title, "Staff Engineer");
    assert_eq!(sanitized.url, "https://boards.example/jobs/1");
    assert_eq!(sanitized.board, "linkedin");

    let admitted = (!sanitized.company.trim().is_empty()).then_some(sanitized.company.as_str());
    assert_eq!(
        admitted, None,
        "a garbage company must never reach CompanyResearch"
    );
}

/// The legitimate-name counterpart: a real employer name must reach
/// `QualityInput::company_name`/the persisted `AiGenerationRecord.company_name`
/// unchanged, so this boundary can't be satisfied by simply blanking
/// everything.
#[test]
fn sanitize_job_meta_leaves_a_legitimate_company_untouched() {
    let legit = crate::commands::match_resume::JobPostingMeta {
        company: "Acme Corp".to_string(),
        ..Default::default()
    };
    let sanitized = super::super::resolve::sanitize_job_meta(legit);
    assert_eq!(sanitized.company, "Acme Corp");
    let admitted = (!sanitized.company.trim().is_empty()).then_some(sanitized.company.as_str());
    assert_eq!(admitted, Some("Acme Corp"));
}

/// **The error-echo clamp, guarded.** `resolve::echoed`/`ECHO_CHARS_CAP` bound
/// a request-supplied id echoed into a validation-error message — at base
/// the equivalent rule was mutation-checked by `commands/agent.rs`
/// (`clamped_echo`/`ECHO_CAP`), which PR-5 deletes; nothing here re-proved
/// the clamp survived, so `echoed`'s body could regress to `id.to_string()`
/// (no clamp at all) with the whole suite still green — exactly the class of
/// gap this PR's review kept finding. Impact is bounded even so
/// (`JOB_IDENTITY_CAP` already caps `resumeId`/`jobId` at 512 bytes before
/// either ever reaches `echoed`), but a clamp guarded by nothing is a clamp
/// one refactor away from silently doing nothing.
///
/// Mutation-checked, executed: replacing `echoed`'s body with
/// `id.chars().collect()` (dropping the `.take(...)`) fails the second
/// assertion here — the oversized id would come back whole.
#[test]
fn resolve_resume_error_echoes_an_oversized_id_clamped_to_64_chars() {
    let huge_id = "x".repeat(100);
    let choice = super::super::resolve::ResumeSource::Store(&huge_id);
    let err = super::super::resolve::resolve_resume(choice, |_| None)
        .expect_err("a Store miss must error");
    let message = err.to_string();
    assert!(
        message.contains(&"x".repeat(64)),
        "the first 64 chars of the oversized id must still be echoed; got: {message:?}"
    );
    assert!(
        !message.contains(&"x".repeat(65)),
        "the id must be clamped to 64 chars, not echoed in full; got: {message:?}"
    );
}

/// The job-ad twin of the clamp guard above — same primitive, same call
/// shape, `resolve_job`'s own `echoed(id)` site.
#[test]
fn resolve_job_error_echoes_an_oversized_id_clamped_to_64_chars() {
    let huge_id = "y".repeat(100);
    let choice = super::super::resolve::JobSource::Cache(&huge_id);
    let err = super::super::resolve::resolve_job(
        choice,
        |_| None,
        |_| None,
        || panic!("meta_from_request must never run on the Cache arm"),
    )
    .expect_err("a Cache miss must error");
    let message = err.to_string();
    assert!(
        message.contains(&"y".repeat(64)),
        "the first 64 chars of the oversized id must still be echoed; got: {message:?}"
    );
    assert!(
        !message.contains(&"y".repeat(65)),
        "the id must be clamped to 64 chars, not echoed in full; got: {message:?}"
    );
}

#[test]
fn resolve_job_resolves_the_text_arm_via_meta_from_request_without_calling_the_cache_lookups() {
    let choice = super::super::resolve::JobSource::Text("PASTED JOB AD");
    let (ad, meta) = super::super::resolve::resolve_job(
        choice,
        |_| panic!("the Text arm must never call lookup_text"),
        |_| panic!("the Text arm must never call lookup_meta"),
        || crate::commands::match_resume::JobPostingMeta {
            title: "Staff Engineer".to_string(),
            ..Default::default()
        },
    )
    .expect("the Text arm always resolves");
    assert_eq!(ad, "PASTED JOB AD");
    assert_eq!(meta.title, "Staff Engineer");
}

/// **Provenance only on the id path** — the behavioral half (was a
/// source-substring grep test; the DECISION itself is now provable directly).
///
/// Mutation check: return `Some(id)` unconditionally and this fails.
#[test]
fn source_resume_id_for_metrics_is_some_only_on_the_store_path() {
    assert_eq!(
        super::super::resolve::source_resume_id_for_metrics(
            super::super::resolve::ResumeSource::Store("res-1")
        ),
        Some("res-1")
    );
    assert_eq!(
        super::super::resolve::source_resume_id_for_metrics(
            super::super::resolve::ResumeSource::Text("some text")
        ),
        None
    );
}

/// **Call-site wiring, grep-shaped — presence only, never branch semantics**
/// (the behavior is pinned above, on the pure functions directly). `execute`
/// needs an `AppHandle` this crate has no harness for, so "it actually calls
/// the resolve functions rather than a reintroduced inline branch" is
/// otherwise provable only by reading the code.
///
/// Every check below matches the function name plus its open paren, never
/// the argument list: pinning the exact local-variable spelling passed at
/// the call site would be the same lexical-substring anti-pattern this
/// module's pure functions were split out to AVOID for the decision itself
/// (see `resolve.rs`'s module doc) — brittle to a harmless rename or a
/// rustfmt re-wrap of a long call, and no more provable than the name check
/// alone, since this test already disclaims branch semantics.
#[test]
fn execute_routes_resolution_through_the_pure_resolve_functions() {
    let source = include_str!("../run.rs");
    assert!(
        source.contains("resolve::resolve_resume("),
        "execute must resolve the résumé through resolve::resolve_resume"
    );
    assert!(
        source.contains("resolve::resolve_job("),
        "execute must resolve the job ad through resolve::resolve_job"
    );
    assert!(
        source.contains("resolve::source_resume_id_for_metrics("),
        "sourceResumeId must be gated through resolve::source_resume_id_for_metrics"
    );
    assert!(
        source.contains("resolve::run_store_job_url("),
        "the run row's own job_url must route through resolve::run_store_job_url, \
         not reuse the aggregate's job_url directly"
    );
    assert!(
        source.contains("resolve::sanitize_job_meta("),
        "execute must sanitize the resolved posting identity through \
         resolve::sanitize_job_meta before it reaches QualityInput::company_name \
         or the persisted AiGenerationRecord.company_name (A1 hardening plan)"
    );
}
