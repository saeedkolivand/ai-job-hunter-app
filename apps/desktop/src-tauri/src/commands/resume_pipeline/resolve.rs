//! `execute`'s input resolution: the clamp + the ID-WINS decision between a
//! server-side lookup and renderer-supplied text, for both the résumé and the
//! job ad. Split out of `mod.rs` to stay under R8's per-module LOC cap
//! (`docs/architecture-rules.md`) — every item here is `pub(crate)` because
//! `execute`/`persist_document` (in `mod.rs`) and this module's own tests
//! (`test.rs`, a sibling under `resume_pipeline`) all need them, not because
//! anything outside `commands::resume_pipeline` does.

use crate::ipc_contracts::resume_pipeline::ResumePipelineRunRequest;

/// The renderer-supplied free text of one run request, CLAMPED server-side.
///
/// The wire schema's `.max(…)` caps are Zod, and Zod does not run on this
/// transport (`tauri-client` calls `invoke` directly, no parse on the way out),
/// so serde accepts whatever a direct IPC caller sends. Every other command
/// that takes this same text mirrors its caps in Rust —
/// `commands::resume::resume_validate_content` is where these constants live,
/// and they are HOISTED from there rather than re-declared, because two copies
/// of "50 requirements, 300 bytes each" is exactly how one of them drifts.
pub(crate) struct ClampedRequest {
    pub(crate) job_url: String,
    pub(crate) target_language: String,
    pub(crate) top_requirements: Vec<String>,
    pub(crate) cover_letter: String,
    /// The id-less résumé path (`resumeText`) — read only when
    /// [`resume_source`] resolves to [`ResumeSource::Text`].
    pub(crate) resume_text: String,
    /// The id-less job-ad path (`jobAdText`) — read only when [`job_source`]
    /// resolves to [`JobSource::Text`].
    pub(crate) job_ad_text: String,
    /// Posting identity for the text path only — mirrors what
    /// `job_meta_for` would have read off the cached posting.
    pub(crate) job_title: String,
    pub(crate) company_name: String,
    pub(crate) board: String,
}

// `include_cover_letter` is a plain `bool` — no free text to clamp, so it rides
// straight from `req` into `QualityInput` at the call site rather than through
// `ClampedRequest`, which exists only for fields that need one.

/// Byte cap on the request's `jobUrl` — mirrors the schema's `.max(2_048)`.
/// This value is a STORAGE key (the run row's retention partition and the
/// aggregate lookup), so an unbounded one writes an unbounded row.
pub(crate) const JOB_URL_CAP: usize = 2_048;

/// Byte cap on the request's `jobTitle`/`companyName` — mirrors the schema's
/// `.max(512)`. Only read on the TEXT path ([`JobSource::Text`]): the id path
/// resolves these from the postings cache server-side and never trusts the
/// request's own copy, so there is nothing to hoist these from — they are new
/// fields with no prior command carrying the same shape.
pub(crate) const JOB_IDENTITY_CAP: usize = 512;
/// Byte cap on the request's `board` — mirrors the schema's `.max(64)`. Board
/// identifiers are short slugs (`"linkedin"`, `"indeed"`, an aggregator name),
/// hence the much smaller cap than [`JOB_IDENTITY_CAP`].
pub(crate) const BOARD_CAP: usize = 64;

/// Clamp every renderer-supplied free-text field of a run request. Pure, so the
/// caps are a test rather than a claim.
pub(crate) fn clamp_request(req: &ResumePipelineRunRequest) -> ClampedRequest {
    use crate::applications::{clamp_to_bytes, MAX_JOB_DESCRIPTION_BYTES};
    use crate::commands::resume::{
        TARGET_LANGUAGE_CAP, TOP_REQUIREMENTS_CAP, TOP_REQUIREMENT_BYTES_CAP,
    };

    ClampedRequest {
        job_url: clamp_to_bytes(req.job_url.clone(), JOB_URL_CAP),
        target_language: clamp_to_bytes(req.target_language.clone(), TARGET_LANGUAGE_CAP),
        top_requirements: req
            .top_requirements
            .iter()
            .take(TOP_REQUIREMENTS_CAP)
            .map(|r| clamp_to_bytes(r.clone(), TOP_REQUIREMENT_BYTES_CAP))
            .collect(),
        cover_letter: clamp_to_bytes(req.cover_letter_text.clone(), MAX_JOB_DESCRIPTION_BYTES),
        // Same cap class as `resumeText`/`jobText` elsewhere
        // (`ResumeTrimSuggestionsRequestSchema`, `MatchResumeRequest`'s own
        // clamp) and as `cover_letter` above — all three are "a whole
        // résumé/posting", never a snippet.
        resume_text: clamp_to_bytes(req.resume_text.clone(), MAX_JOB_DESCRIPTION_BYTES),
        job_ad_text: clamp_to_bytes(req.job_ad_text.clone(), MAX_JOB_DESCRIPTION_BYTES),
        job_title: clamp_to_bytes(req.job_title.clone(), JOB_IDENTITY_CAP),
        company_name: clamp_to_bytes(req.company_name.clone(), JOB_IDENTITY_CAP),
        board: clamp_to_bytes(req.board.clone(), BOARD_CAP),
    }
}

/// Where one run's résumé text comes from — the pure half of the ID-WINS rule
/// `execute` applies. Pure and total over the two clamped inputs, so the rule
/// is a test rather than a claim; the actual `DocumentStore` lookup needs an
/// `AppHandle` this crate has no harness for, so it stays in `execute`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResumeSource<'a> {
    /// A nonempty `resumeId` — look this up; a miss is a HARD ERROR, never a
    /// fallback to `resumeText`, even when the request also carries one.
    Store(&'a str),
    /// `resumeId` was empty and `resumeText` was not.
    Text(&'a str),
}

/// `None` when both are empty (or whitespace-only) — `execute` turns that into
/// a validation error, since a run needs a résumé from somewhere.
pub(crate) fn resume_source<'a>(
    resume_id: &'a str,
    resume_text: &'a str,
) -> Option<ResumeSource<'a>> {
    if !resume_id.trim().is_empty() {
        return Some(ResumeSource::Store(resume_id));
    }
    (!resume_text.trim().is_empty()).then_some(ResumeSource::Text(resume_text))
}

/// The job-ad twin of [`ResumeSource`] — same ID-WINS rule, over `jobId`
/// (the live postings cache) and `jobAdText`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JobSource<'a> {
    /// A nonempty `jobId` — look this up in the postings cache; a miss is a
    /// HARD ERROR, never a fallback to `jobAdText`.
    Cache(&'a str),
    /// `jobId` was empty and `jobAdText` was not.
    Text(&'a str),
}

pub(crate) fn job_source<'a>(job_id: &'a str, job_ad_text: &'a str) -> Option<JobSource<'a>> {
    if !job_id.trim().is_empty() {
        return Some(JobSource::Cache(job_id));
    }
    (!job_ad_text.trim().is_empty()).then_some(JobSource::Text(job_ad_text))
}

/// The posting identity `execute` builds on the TEXT path, where there is no
/// cached posting for `job_meta_for` to read. Pure so the field mapping is a
/// test rather than a claim — `location` has no wire field (the cache's own
/// `job_meta_for` populates it from the posting, which the text path has no
/// equivalent of) and is always empty here.
pub(crate) fn job_meta_from_request(
    clamped: &ClampedRequest,
) -> crate::commands::match_resume::JobPostingMeta {
    crate::commands::match_resume::JobPostingMeta {
        company: clamped.company_name.clone(),
        title: clamped.job_title.clone(),
        url: clamped.job_url.clone(),
        board: clamped.board.clone(),
        location: String::new(),
    }
}

/// What `persist_document` (`mod.rs`) should write into the aggregate's
/// `job_ad` column. `Cache` stays empty — deliberately, per that fn's own doc:
/// the postings cache is keyed by the live job id, which a finished run no
/// longer holds, so persisting it here would not fix the staleness it already
/// documents. `Text` DOES have the text this run was actually built from, so
/// it persists it — `merge_application`'s `pick` keeps the existing value
/// when this is empty, so the `Cache` arm's empty string is a no-op on an
/// existing aggregate, not an overwrite.
pub(crate) fn job_ad_for_persist(source: JobSource<'_>) -> String {
    match source {
        JobSource::Cache(_) => String::new(),
        JobSource::Text(text) => text.to_string(),
    }
}
