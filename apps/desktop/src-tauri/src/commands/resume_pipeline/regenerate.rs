//! The write surface: [`resume_pipeline_regenerate_section`] and
//! [`resume_pipeline_resolve_fabrication`], plus [`ensure_latest_run`] — the
//! rule that the document a run's write commands act on belongs to the
//! NEWEST run of that posting, not to the run whose id was passed.

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::ai_generations::{AiGenerationRecord, AiGenerationStore};
use crate::commands::ai::redact_for_provider;
use crate::documents::DocumentStore;
use crate::error::{AppError, AppResult};
use crate::ipc_contracts::resume_pipeline::{
    ResumePipelineRegenerateSectionRequest, ResumePipelineResolveFabricationRequest,
};
use crate::pipeline::resume::projects;
use crate::pipeline::resume::stages::{self, regenerate_one_section, SectionOutcome};
use crate::pipeline::resume::types::SectionKey;
use crate::pipeline::runs::{PipelineRunStore, RunRow};
use crate::pipeline::Completer;

use super::read::detail;
use super::{report, STATUS_COMPLETED, STATUS_NEEDS_REVIEW};

/// The DOCUMENT a run's write commands act on belongs to the NEWEST run of that
/// posting, not to the run whose id was passed.
///
/// `pipeline_runs` keeps one row per run, but every run of a posting merges into
/// the SAME `ai_generations` aggregate (see the module doc), so `listForJob`
/// legitimately advertises three runs while `find_for_job` can only ever return
/// one document — the newest run's. A `regenerateSection` against an older run
/// id would therefore rewrite the NEWEST document and hand back a detail whose
/// `resumeText` was never that run's; a `resolveFabrication` would record a
/// verdict against findings from a report the run never produced.
///
/// Refusing is the honest answer, and the smallest one: making it work needs a
/// per-run document, which is a schema change and a second copy of every
/// résumé. Read-only `get` still returns the older run's own row — its status,
/// metrics and stage trail are genuinely its — with the aggregate document
/// alongside; the contract's doc comment says so.
///
/// An UNLINKED run (empty `job_url`) is exempt: it has no aggregate at all, so
/// the "no saved résumé" error below is the accurate one.
///
/// **The rule is about newer RUNS, not about newer TEXT** — deliberately, and
/// the message is worded that way. The user editing the document (applying a
/// "Remove", or any hand edit) moves `ai_generations.resume_text` while this
/// stays the newest run, so `resolveFabrication` and `regenerateSection` keep
/// working on their own posting; the report simply reads stale until a re-check.
/// Refusing an edited-but-latest run would strand the exact review the panel is
/// asking the user to finish.
pub(super) fn ensure_latest_run(store: &PipelineRunStore, row: &RunRow) -> AppResult<()> {
    if row.job_url.trim().is_empty() {
        return Ok(());
    }
    let newest = store
        .runs_for_job(&row.job_url)
        .into_iter()
        .find(|candidate| candidate.kind == row.kind);
    match newest {
        // Deliberately does NOT say "open the latest run": the newest run may
        // still be running, or may have failed before it saved anything, so
        // pointing the user at a document that does not exist yet is the second
        // wrong answer after the edit itself.
        Some(newest) if newest.id != row.id => Err(AppError::Validation(
            "There is a newer run for this posting, and all of its runs share one saved \
             résumé — only the newest one can change it. If that run is still going, wait \
             for it to finish."
                .to_string(),
        )),
        _ => Ok(()),
    }
}

/// Re-generate ONE section of a finished run and splice it back.
///
/// **`"header"` is rejected here, at the boundary**, and not by a special case:
/// [`SectionKey::from_wire`] runs the generated `is_pipeline_section_key`
/// grammar, which has no header token — so the contact header the editor owns
/// at export time (ADR-0021) is unreachable from this command by construction,
/// along with every other invented section name.
///
/// **Admission-limited like every other provider-calling command.** It makes a
/// full completion (plus a re-validation) per click, and
/// `PROVIDER_DAILY_MAX` is a per-DAY total, not a rate — so an unadmitted
/// button is a renderer loop that can burn a day's ceiling in seconds. It takes
/// the same `agent_run` bucket as the run itself, and `acquire` (not
/// `acquire_queued`): this is a click, and a click that has to wait behind a
/// 45-minute run should be refused with a retriable error, not parked.
#[tauri::command]
pub async fn resume_pipeline_regenerate_section(
    app: AppHandle,
    req: ResumePipelineRegenerateSectionRequest,
) -> AppResult<Value> {
    let _guard = app.state::<Arc<crate::limits::Limiter>>().inner().acquire(
        "agent_run",
        crate::limits::AGENT_RUN_RATE_MAX,
        crate::limits::AGENT_RUN_CONCURRENCY_MAX,
    )?;

    let key = SectionKey::from_wire(&req.section_key).ok_or_else(|| {
        AppError::Validation(format!(
            "{:?} is not a section this pipeline can regenerate. The contact header is owned \
             by the editor at export time and is never model-written.",
            req.section_key
        ))
    })?;

    let store = app.state::<PipelineRunStore>();
    let row = store
        .run(&req.run_id)
        .ok_or_else(|| AppError::Validation(format!("run not found: {}", req.run_id)))?;
    ensure_latest_run(&store, &row)?;
    let generations = app
        .try_state::<AiGenerationStore>()
        .ok_or_else(|| AppError::Storage("the generation store is unavailable".to_string()))?;
    // The PROPERTY, not the proxy: a run that never wrote a résumé has no
    // section to regenerate, however full the posting's aggregate is. Checked
    // before the record lookup so a cover-letter-only run is refused for the
    // true reason rather than falling through to the emptiness filter below,
    // which an earlier run's saved résumé would satisfy. See
    // `run_wrote_a_resume`.
    if !run_wrote_a_resume(&row) {
        return Err(AppError::Validation(
            "this run generated a cover letter, not a résumé, so it has no section to              regenerate"
                .to_string(),
        ));
    }
    let record = generations
        .find_for_job(&row.job_url)
        .filter(|record| !record.resume_text.trim().is_empty())
        .ok_or_else(|| {
            AppError::Validation(
                "this run has no saved résumé to regenerate a section of".to_string(),
            )
        })?;

    let span = crate::observability::Span::begin(
        "pipeline:resume",
        format!("op=regenerate_section key={}", key.to_wire()),
    );
    let (source, source_is_provenanced) = source_resume_for(&app, &row, &record);
    // A whole-section REWRITE using `repair`'s prompt and grounding — the ONLY
    // path now that the max-depth per-entry artifact rebuild is gone. It
    // follows `repair`'s own override, the stage whose prompt this uses.
    // Bound ONCE: the call and the secret strip below must see the same
    // provider/base URL even if settings change mid-call.
    let completer = Completer::from_active_for_stage(&app, stages::REPAIR_STAGE)?;
    let outcome = regenerate_one_section(
        &completer,
        &source,
        &record.target_language,
        &record.resume_text,
        key,
        // No validator issues on this path: the user, not a report,
        // asked for the change. The note carries the "why", fenced.
        &[],
        req.note.as_deref(),
        // The button keeps the provider's own default effort.
        None,
    )
    .await;
    // The provider's error text reaches the review panel verbatim: strip the
    // stage provider's stored key / base-URL secrets and shape-redact (#1346).
    let outcome = redact_for_provider(
        &app,
        completer.provider_id().credential_key(),
        completer.base_url(),
        outcome,
    )?;
    let spliced = match outcome {
        SectionOutcome::Replaced(spliced) => spliced,
        SectionOutcome::Unusable => {
            span.end(false);
            return Err(AppError::Provider(
                "The model's replacement section came back empty or truncated, so nothing was \
                 changed. Try again."
                    .to_string(),
            ));
        }
        // No provider call was made — say so, rather than blaming the model for
        // a section this document does not have.
        SectionOutcome::Missing => {
            span.end(false);
            return Err(AppError::Validation(format!(
                "this résumé has no {} section to regenerate",
                key.to_wire()
            )));
        }
    };

    let (spliced, projects_normalize_skipped) =
        normalize_regenerated_projects(key, &source, source_is_provenanced, spliced);

    // The merge rule again: this save writes `resume_text`, so it carries a
    // FRESH report over the spliced document — never the stale one the panel
    // was showing.
    let (report, letter) = crate::pipeline::resume::stages::validate_documents(
        spliced.clone(),
        source,
        record.job_ad.clone(),
        record.top_requirements.clone(),
        record.target_language.clone(),
        record.cover_letter_text.clone(),
    )
    .await?;
    let wrapper = report::build(
        &row.depth,
        crate::db::now_ms(),
        Some((&report, &spliced)),
        letter
            .as_ref()
            .map(|letter| (letter, record.cover_letter_text.as_str())),
    );
    let needs_review = report::still_needs_review(&wrapper, &spliced, &record.cover_letter_text);
    // ONE write, not two: the merge rule above says the text and its report
    // move together, and two statements leave a window where a crash (or a
    // failing second statement) persists a document with the PREVIOUS
    // document's report — the exact state the rule exists to make impossible.
    generations.update_text_and_report(&record.id, spliced, wrapper)?;
    // Content-free (ADR-027): a code, never the document — the same reason
    // vocabulary `Draft::run`'s ledger artifact uses, so a declined Projects
    // normalization on a manual regenerate is as observable as it is on a run.
    span.end_with(
        &format!(
            "issues={} blocking={}{}",
            report.issues.len(),
            report::has_criticals(&report),
            projects_normalize_skipped
                .map(|reason| format!(" projectsNormalizeSkipped={reason}"))
                .unwrap_or_default()
        ),
        true,
    );

    // The mirror of `resolve_fabrication`'s clearing arm: a regenerated
    // section can INTRODUCE a fresh finding on a run whose row still says
    // `completed`, and the panel keys its headline (and whether the review
    // block renders at all) on that row — so leaving it untouched would
    // suppress the very review the regeneration just created.
    if let Some(next) = recomputed_status(&row.status, needs_review) {
        let mut row = row.clone();
        row.status = next.to_string();
        store.upsert_run(&row)?;
        return Ok(detail(&app, &row));
    }
    Ok(detail(&app, &row))
}

/// The status a run row should move to after its persisted wrapper changed —
/// `None` when it should not move at all.
///
/// Only the two REVIEW-terminal states convert into each other: a wrapper
/// write can un-clean a `completed` run (a regenerated section introducing a
/// fresh fabrication) and can finish a `needsReview` one (the last verdict
/// landing). `failed` and `cancelled` describe how the RUN ended, which no
/// amount of report movement rewrites — and `running` is not terminal.
pub(super) fn recomputed_status(current: &str, needs_review: bool) -> Option<&'static str> {
    let next = if needs_review {
        STATUS_NEEDS_REVIEW
    } else {
        STATUS_COMPLETED
    };
    (matches!(current, STATUS_COMPLETED | STATUS_NEEDS_REVIEW) && current != next).then_some(next)
}

/// Record the user's Remove/Keep verdict on ONE surviving fabrication finding.
///
/// Nothing is removed here — the decision is RECORDED. Removing a bullet is a
/// text edit the user makes (or accepts) in the editor; a command that silently
/// deleted lines from a document on a single click would be exactly the
/// "nothing is removed silently" rule inverted.
#[tauri::command]
pub async fn resume_pipeline_resolve_fabrication(
    app: AppHandle,
    req: ResumePipelineResolveFabricationRequest,
) -> AppResult<Value> {
    if req.decision != "remove" && req.decision != "keep" {
        return Err(AppError::Validation(format!(
            "unknown fabrication decision: {}",
            req.decision
        )));
    }
    let store = app.state::<PipelineRunStore>();
    let row = store
        .run(&req.run_id)
        .ok_or_else(|| AppError::Validation(format!("run not found: {}", req.run_id)))?;
    // Same aggregate, same rule (see `ensure_latest_run`): a verdict recorded
    // against an older run would land in the NEWEST run's report.
    ensure_latest_run(&store, &row)?;
    let generations = app
        .try_state::<AiGenerationStore>()
        .ok_or_else(|| AppError::Storage("the generation store is unavailable".to_string()))?;
    let record = generations
        .find_for_job(&row.job_url)
        .ok_or_else(|| AppError::Validation("this run has no saved report".to_string()))?;

    if let Some(updated) =
        report::record_decision(&record.quality_report, &req.issue_key, &req.decision)
    {
        // The run leaves `needsReview` only when NOTHING is blocking any more:
        // every flagged bullet decided — with the document AGREEING with every
        // Remove (a recorded-but-unapplied removal is intent, not fact; see
        // `report::entry_resolved`) — and no Critical the review cannot clear
        // (`factual.dropped_role` names an absence, so it is not in the panel —
        // and a run that flipped to `completed` because every *reviewable*
        // finding was decided would present a résumé that silently lost an
        // employer as clean). `record.resume_text` is current: the live panel
        // applies the removal edit BEFORE recording the verdict.
        let needs_review =
            report::still_needs_review(&updated, &record.resume_text, &record.cover_letter_text);
        generations.update_quality_report(&record.id, updated)?;
        if let Some(next) = recomputed_status(&row.status, needs_review) {
            let mut row = row.clone();
            row.status = next.to_string();
            store.upsert_run(&row)?;
            return Ok(detail(&app, &row));
        }
    }
    Ok(detail(&app, &row))
}

/// The SOURCE résumé a re-validation must measure against, plus whether it is
/// a REAL provenance hit rather than the fallback.
///
/// A generation record stores the OUTPUT, not the input it was built from, so
/// the source is looked up through the id the run recorded as provenance (see
/// `execute`'s `sourceResumeId`). When that document is gone — deleted,
/// replaced, or the run predates the field — the fallback is the generated text
/// itself: measured against itself, the factual checks find nothing, so the
/// re-validated report comes back WEAKER than the run's original rather than
/// wrong. Weaker-and-honest beats a fabricated Critical against a source this
/// document was never written from.
///
/// The `bool` is why this returns a pair rather than the string alone: that
/// fallback is fine for VALIDATION (grading against your own output finds
/// nothing, so it degrades safely), but it is not a WRITE authority — seeding
/// `projects::normalize_projects` from the generated text would "restore"
/// links out of the document it is about to overwrite, i.e. codify whatever
/// the last generation happened to say as fact. Callers that write must check
/// this flag; callers that only validate may ignore it.
fn source_resume_for(app: &AppHandle, row: &RunRow, record: &AiGenerationRecord) -> (String, bool) {
    let hit = serde_json::from_str::<Value>(&row.metrics_json)
        .ok()
        .and_then(|metrics| {
            metrics
                .get("sourceResumeId")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .and_then(|id| app.state::<DocumentStore>().get(&id));
    match hit {
        Some(document) => (document.text, true),
        None => (record.resume_text.clone(), false),
    }
}

/// Whether `row`'s run actually WROTE the résumé the posting's aggregate now
/// holds — `metrics_json.resumeInRun`, written by `execute`.
///
/// **Absent means `true`.** Every run that predates the cover-letter-only mode
/// wrote a résumé, and there is no migration touching those rows; reading a
/// missing key as `false` would refuse a section regenerate on every historic
/// run in the store.
///
/// This exists because the check it replaces was a PROXY. `regenerateSection`
/// asked whether the aggregate's `resume_text` is non-empty — which answers
/// "does this posting have a résumé", not "did this run write one". Those were
/// the same question until `includeResume` arrived: a cover-letter-only run is
/// the posting's newest (so `ensure_latest_run` passes) and the aggregate still
/// carries an EARLIER run's résumé (so the emptiness filter passes), and the
/// command would then splice, re-validate and persist a section of a document
/// the run never produced — spending a provider call and overwriting the
/// posting's report on the way.
pub(super) fn run_wrote_a_resume(row: &RunRow) -> bool {
    serde_json::from_str::<Value>(&row.metrics_json)
        .ok()
        .and_then(|metrics| metrics.get("resumeInRun").and_then(Value::as_bool))
        .unwrap_or(true)
}

/// The pure decision behind `resume_pipeline_regenerate_section`'s Projects
/// normalization — pulled out of the command so it is unit-testable without
/// an `AppHandle`/store. Deterministic, zero-cost: a REGENERATE-PROJECTS
/// click can rewrite the Projects section too (the whole-section rewrite
/// fallback is a free-text rewrite, not scoped by content), so the same
/// code-owned normalization the run itself applies must run here before the
/// fresh report is computed — otherwise a click could persist an altered
/// project link the run would never have let through.
///
/// Scoped to `key == Projects` ONLY: normalizing on every OTHER section's
/// regenerate would silently revert the user's own manual edits to Projects
/// every time they regenerate, say, Skills.
///
/// Gated on `source_is_provenanced`: [`source_resume_for`] falls back to the
/// run's own GENERATED text when `sourceResumeId` is missing/deleted, which
/// is fine for grading (measuring text against itself finds nothing) but not
/// for WRITING — seeding the normalizer from the document it is about to
/// overwrite would "restore" whatever the last generation said as if it were
/// the candidate's own source.
///
/// Returns the (possibly normalized) text PLUS a content-free reason
/// (ADR-027) when normalization declined to run — `key != Projects` reports
/// none (there was nothing to attempt), but every other decline is
/// observable, the same way `Draft::run`'s `apply_projects_normalization`
/// reports one on the run's ledger. The caller folds it into the command's
/// own `Span`.
pub(super) fn normalize_regenerated_projects(
    key: SectionKey,
    source: &str,
    source_is_provenanced: bool,
    spliced: String,
) -> (String, Option<&'static str>) {
    if key != SectionKey::Projects {
        return (spliced, None);
    }
    if !source_is_provenanced {
        return (spliced, Some("unprovenanced_source"));
    }
    let (project_seeds, seed_skip_reason) = projects::seed_projects_for_normalize(source);
    match projects::normalize_projects_outcome(&spliced, &project_seeds) {
        projects::ProjectsNormalizeOutcome::Applied(text, _) => (text, None),
        projects::ProjectsNormalizeOutcome::Skipped(reason) => (spliced, Some(reason)),
        projects::ProjectsNormalizeOutcome::NoOp => (spliced, seed_skip_reason),
    }
}
