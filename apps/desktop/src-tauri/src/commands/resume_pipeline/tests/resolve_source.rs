use crate::ipc_contracts::resume_pipeline::ResumePipelineRunRequest;
use serde_json::json;

/// **ID wins, even when the request also carries text** — the core of the
/// no-silent-fallback rule. Mutation check: swap the `if` to check `resume_text`
/// first and this fails.
#[test]
fn resume_source_prefers_the_id_even_when_text_is_also_present() {
    assert_eq!(
        super::super::resume_source("res-1", "a whole résumé"),
        Some(super::super::resolve::ResumeSource::Store("res-1"))
    );
}

#[test]
fn resume_source_uses_text_only_when_the_id_is_empty() {
    assert_eq!(
        super::super::resume_source("", "a whole résumé"),
        Some(super::super::resolve::ResumeSource::Text("a whole résumé"))
    );
}

/// Neither an id nor usable text — `execute` turns this into a validation
/// error rather than starting a run with no résumé at all. Whitespace-only
/// counts as empty, matching every other trim-then-check field on this
/// command.
#[test]
fn resume_source_is_none_when_both_are_empty_or_whitespace() {
    assert_eq!(super::super::resume_source("", ""), None);
    assert_eq!(super::super::resume_source("   ", "\n\t"), None);
}

#[test]
fn job_source_prefers_the_id_even_when_text_is_also_present() {
    assert_eq!(
        super::super::job_source("job-9", "a whole job ad"),
        Some(super::super::resolve::JobSource::Cache("job-9"))
    );
}

#[test]
fn job_source_uses_text_only_when_the_id_is_empty() {
    assert_eq!(
        super::super::job_source("", "a whole job ad"),
        Some(super::super::resolve::JobSource::Text("a whole job ad"))
    );
}

#[test]
fn job_source_is_none_when_both_are_empty_or_whitespace() {
    assert_eq!(super::super::job_source("", ""), None);
    assert_eq!(super::super::job_source("  ", "\n"), None);
}

/// The TEXT path's posting identity comes from the REQUEST's own
/// `jobTitle`/`companyName`/`board`/`jobUrl` fields — there is no cached
/// posting to read them off of on this path, unlike `job_meta_for`.
/// `location` has no wire field on this path and is always empty.
///
/// Mutation check: swap two field mappings (e.g. `company`/`title`) and this
/// fails.
#[test]
fn job_meta_from_request_reads_the_identity_fields_off_the_clamped_request() {
    let req: ResumePipelineRunRequest = serde_json::from_value(json!({
        "resumeText": "a whole résumé",
        "jobAdText": "a whole job ad",
        "jobTitle": "Staff Engineer",
        "companyName": "Acme Corp",
        "board": "linkedin",
        "jobUrl": "https://boards.example/jobs/1",
    }))
    .expect("deserializes");
    let clamped = super::super::clamp_request(&req);
    let meta = super::super::job_meta_from_request(&clamped);
    assert_eq!(meta.title, "Staff Engineer");
    assert_eq!(meta.company, "Acme Corp");
    assert_eq!(meta.board, "linkedin");
    assert_eq!(meta.url, "https://boards.example/jobs/1");
}

/// **The persist fix (plan risk item 5).** The `Cache` path stays empty
/// (unchanged — `persist_document`'s own doc explains why), and the `Text`
/// path carries the text this run was actually built from. Mutation check:
/// return the text on the `Cache` arm too and this fails.
#[test]
fn job_ad_for_persist_is_empty_on_the_cache_path_and_carries_the_text_on_the_text_path() {
    assert_eq!(
        super::super::job_ad_for_persist(super::super::resolve::JobSource::Cache("job-9")),
        ""
    );
    assert_eq!(
        super::super::job_ad_for_persist(super::super::resolve::JobSource::Text("a whole job ad")),
        "a whole job ad"
    );
}

#[test]
fn resolve_resume_returns_the_looked_up_text_on_a_store_hit() {
    let choice = super::super::resolve::ResumeSource::Store("res-1");
    let resolved = super::super::resolve::resolve_resume(choice, |id| {
        (id == "res-1").then(|| "RESUME TEXT".to_string())
    })
    .expect("a hit resolves");
    assert_eq!(resolved, "RESUME TEXT");
}

/// The reviewer's own reproduction: build the choice through `resume_source`
/// with BOTH an id and text present (so it still picks `Store` — ID WINS),
/// then resolve it against a lookup that finds nothing. `resolve_resume` has
/// no path back to the text `resume_source` saw and discarded — only `id`
/// and `lookup` are in its scope — so this MUST error.
#[test]
fn resolve_resume_errors_on_a_store_miss_even_though_the_request_also_carried_text() {
    let choice =
        super::super::resume_source("res-1", "a whole résumé text").expect("an id is present");
    let err = super::super::resolve::resolve_resume(choice, |_| None).expect_err(
        "a Store miss must error, never silently read the text resume_source saw but discarded",
    );
    assert!(matches!(err, crate::error::AppError::Validation(_)));
}

#[test]
fn resolve_resume_resolves_the_text_arm_without_ever_calling_lookup() {
    let choice = super::super::resolve::ResumeSource::Text("PASTED RESUME");
    let resolved = super::super::resolve::resolve_resume(choice, |_| {
        panic!("the Text arm must never call lookup")
    })
    .expect("the Text arm always resolves");
    assert_eq!(resolved, "PASTED RESUME");
}
