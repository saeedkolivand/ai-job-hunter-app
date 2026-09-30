//! [`persist_document`] — merge a finished run's document + fresh quality
//! report into the per-job `ai_generations` aggregate.

use tauri::{AppHandle, Manager};

use crate::ai_generations::{make_generation_id, AiGenerationRecord, AiGenerationStore};
use crate::pipeline::resume::QualityCtx;

use super::report;
use super::resolve::ClampedRequest;
use super::save::{save_verdict, SaveVerdict};

/// Merge the finished document + a FRESH quality report into the per-job
/// aggregate, returning the wrapper that was written.
///
/// The Phase-1 merge rule, mechanically: **any save writing `resume_text`
/// carries a fresh `quality_report`.** They go in the same record for exactly
/// that reason — a save that wrote the text and left the report to a second
/// call would leave a window where the panel describes the previous document.
///
/// `None` when there was nothing to save — a run that failed before validating
/// EITHER document, or one whose [`save_verdict`] came back anything but
/// `Save`: a record with no text and no report is not an aggregate update, it
/// is noise.
///
/// **An empty `draft` is not by itself "nothing to save".** A
/// cover-letter-only run (`includeResume: false`) reaches here with one by
/// design, and it saves: `resume_text` goes in empty, which
/// `ai_generations::pick` reads as "keep the stored value", and the wrapper
/// omits the `resume` key entirely, which the merge reads the same way. The
/// posting's previously tailored résumé — and every Keep/Remove verdict
/// recorded against it — survives the save untouched.
///
/// **An UNLINKED run (no resolvable `jobUrl`) saves nothing.** The aggregate is
/// keyed by posting url, so a row with an empty one is unreachable by every
/// reader in the app (`find_for_job` looks up a url; `applied_job_urls` filters
/// them out) and unreachable by the retention prune, which partitions on
/// `(job_url, kind)` — a permanent, invisible row holding a full résumé. The
/// plan calls an unlinked generation session-only, and the run row + the stream
/// the user watched are that session.
///
/// ## A STOPPED run overwrites the saved document, and what that trades
///
/// This save is an overwrite: there is one `ai_generations` row per posting and
/// no versioning, so whatever a run persists REPLACES what the user had. Since
/// a deadline-stopped run keeps whatever it already produced (a `repair` round
/// that ran out of time mid-fan-out still KEEPS its accumulated corrections and
/// returns `Ok`, and `hooks::apply_stop` lets the free `validate` boundary run
/// past a clock that expired during the preceding paid stage), that includes
/// PARTIAL documents — which is deliberate, and worth stating rather than
/// discovering:
///
/// * **The kept trade.** A run stopped near the end still has a real, checked
///   draft — a missing correction is a visible Critical the report already
///   describes, the run lands `needsReview`, and the user is looking at a
///   document they can see is unfinished. That beats discarding everything the
///   run had already paid for.
/// * **The refused trade** ([`is_persistable`]). A document that lost ALL of a
///   work history the SOURCE has is not a short résumé, it is not a résumé —
///   and overwriting a good previous document with it is a loss the review
///   panel cannot describe and the user cannot undo. Nothing is saved, and the
///   previous document survives. Source-RELATIVE on purpose: a candidate whose
///   own résumé has no employment section is a real input, not a failure.
///
/// `job_ad_for_persist` is [`super::resolve::job_ad_for_persist`]'s output:
/// empty on the `Cache` path (unchanged — see that fn's doc for why), the
/// request's own job-ad text on the `Text` path. `merge_application`'s `pick`
/// keeps the existing aggregate value whenever the incoming one is empty, so
/// the `Cache` path's empty string never erases a `job_ad` an earlier save
/// wrote.
///
/// **Establishes the Application FK, the same way [`ai_generations_save`]
/// does — this writer used to skip that step entirely.** ADR 0001 demoted
/// the generation to a child Document of an Application, but
/// `ai_generations::merge_application`'s `application_id:
/// incoming.application_id.or(existing.application_id)` only ever PRESERVES
/// an id that already arrived on the row; nothing in this pipeline ever
/// SET one, so the first staged-pipeline save for a posting with no prior
/// linked generation persisted with `application_id` permanently `NULL` —
/// invisible to every reader that joins/filters `ai_generations` by
/// `applicationId`. The résumé masked it: it has its own run-scoped read
/// path (`PipelineRunDetail.resumeText`, joined by run id, no FK involved),
/// but the cover letter has no such channel and is unreachable for that
/// whole class of application.
///
/// **Read-only lookup, never create-on-miss** — the same posture as
/// [`crate::applications::ApplicationStore::link_orphaned_generations`] (own
/// doc), for the same reason: a staged run is always launched FROM an
/// existing Application's page, so it's already there by the time this runs.
/// A run takes minutes; if the user deletes that Application while it's in
/// flight, an upsert would silently resurrect it the moment the run lands.
/// A miss here just leaves `application_id: None` — the row stays findable
/// by `AiGenerationStore::find_for_job` (keyed on `job_url`, not the FK),
/// only Application-scoped readers miss it, exactly like the error path
/// below.
///
/// [`ai_generations_save`]: crate::commands::ai_generations::ai_generations_save
pub(super) fn persist_document(
    app: &AppHandle,
    job_url: &str,
    meta: &crate::commands::match_resume::JobPostingMeta,
    clamped: &ClampedRequest,
    job_ad_for_persist: &str,
    ctx: &QualityCtx<'_>,
    depth: &str,
) -> Option<String> {
    // At least ONE document must have been validated. This used to be
    // `ctx.report.as_ref()?` — the résumé's report as the gate for the whole
    // save — which on a cover-letter-only run (no résumé, so no résumé report
    // by design; see `stages::validate`) would discard a fully generated,
    // validated letter without a word anywhere.
    if ctx.report.is_none() && ctx.letter_report.is_none() {
        return None;
    }
    // The gate gates `ctx.letter` — what `cover_letter_text` below actually
    // stores — NOT `ctx.letter_text()`'s fallback to the user's own pasted
    // reference letter (see the identical comment on `execute`'s own
    // `save_verdict` call, which this one must agree with). `letter_text` is
    // still what the wrapper below is built over: the report legitimately
    // describes whichever text `report::build` was given, stage output or
    // fallback, and that's what `letter_text()` is for.
    let letter_text = ctx.letter_text();
    if save_verdict(
        ctx.input.source_resume,
        &ctx.draft,
        &ctx.letter,
        job_url,
        ctx.input.include_resume,
    ) != SaveVerdict::Save
    {
        return None;
    }
    let wrapper = report::build(
        depth,
        crate::db::now_ms(),
        // `Option`, not an unconditional `Some`: `report::build`'s own doc —
        // "a document this run did not validate contributes NO key at all" —
        // is what keeps a cover-letter-only run's wrapper from overwriting the
        // posting's stored `resume` slot with an empty one. The merge overlays
        // whatever keys the wrapper carries, so the older résumé's report and
        // its fabrication verdicts survive untouched.
        ctx.report
            .as_ref()
            .map(|report| (report, ctx.draft.as_str())),
        ctx.letter_report
            .as_ref()
            .map(|letter| (letter, letter_text)),
    );
    let store = app.try_state::<AiGenerationStore>()?;
    // Read-only lookup, never create-on-miss — see this function's own doc.
    // A miss (no Application, or the store isn't running) just leaves the FK
    // unset; the row is still found by `AiGenerationStore::find_for_job`
    // (keyed on `job_url`, not the FK) — only Application-scoped readers
    // miss it, and a later save retries the same idempotent lookup.
    let application_id = app
        .try_state::<crate::applications::ApplicationStore>()
        .and_then(|apps| {
            let normalized = crate::applications::normalize_job_url(job_url);
            apps.find_by_job_url(&normalized).map(|found| found.id)
        });
    let record = AiGenerationRecord {
        id: make_generation_id(),
        created_at: crate::db::now_ms(),
        target_language: clamped.target_language.clone(),
        // The RESOLVED list, not `clamped.top_requirements` (the request's
        // own, usually empty today): see `QualityCtx::top_requirements`'s
        // doc. Persisting the request's list here is what let an empty save
        // silently freeze whatever a PRIOR run had saved
        // (`ai_generations::merge_application`'s pick-non-empty merge).
        top_requirements: ctx.top_requirements(),
        resume_text: ctx.draft.clone(),
        // ONLY the stage-generated letter, never the fallback: `save_application`'s
        // merge-upsert treats an empty `cover_letter_text` as "keep the
        // existing value" (`ai_generations::pick`), so writing the legacy
        // validate-only request text here would overwrite whatever letter the
        // posting's aggregate already had with a document this run never
        // generated.
        cover_letter_text: ctx.letter.clone(),
        job_ad: job_ad_for_persist.to_string(),
        job_url: job_url.to_string(),
        board: meta.board.clone(),
        company_name: meta.company.clone(),
        job_title: meta.title.clone(),
        quality_report: wrapper.clone(),
        application_id,
        ..empty_record()
    };
    match store.save_application(record) {
        Ok(_) => Some(wrapper),
        Err(e) => {
            // Non-fatal, and logged rather than swallowed: the run's text is
            // already in the renderer's hands via the stream, so failing here
            // would discard a good document because a merge-upsert lost a race.
            log::warn!(
                "[pipeline] could not persist the generated résumé (non-fatal): {}",
                crate::observability::sanitize_reason(&e.to_string())
            );
            None
        }
    }
}

/// The all-empty record the pipeline's own save fills three fields of.
///
/// `merge_application` keeps the EXISTING value for every field an incoming
/// record leaves empty, so an aggregate's cover letter, answers, interview
/// questions and company brief survive a résumé-only save untouched. Written as
/// one helper rather than `Default` because `AiGenerationRecord` has no
/// meaningful default id or timestamp.
pub(super) fn empty_record() -> AiGenerationRecord {
    AiGenerationRecord {
        id: String::new(),
        created_at: 0,
        candidate_name: String::new(),
        job_title: String::new(),
        company_name: String::new(),
        resume_language: String::new(),
        job_ad_language: String::new(),
        target_language: String::new(),
        mismatch: false,
        top_requirements: Vec::new(),
        mode: String::new(),
        resume_text: String::new(),
        cover_letter_text: String::new(),
        job_ad: String::new(),
        job_url: String::new(),
        board: String::new(),
        application_answers: Vec::new(),
        company_brief: String::new(),
        interview_questions: Vec::new(),
        email_subject: String::new(),
        email_body: String::new(),
        application_id: None,
        quality_report: String::new(),
    }
}
