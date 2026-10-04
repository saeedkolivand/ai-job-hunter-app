//! The `ai_generations` record types and the per-job merge of two records.
//!
//! Split out of [`super`] to keep the store body under the architecture LOC cap
//! (`tests/architecture.rs` R8). Nothing here touches SQLite.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::quality_report::merge_quality_report;
use crate::db::now_ms;

/// One answered application question, stored on the application record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApplicationAnswer {
    pub id: String,
    pub question: String,
    pub answer: String,
}

/// One AI-suggested question the candidate can ASK the interviewer (distinct from
/// the answered application questions above). Stored on the application record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InterviewQuestion {
    pub id: String,
    pub question: String,
    /// Why this question lands well / what it signals to the interviewer.
    pub why: String,
    /// Target interviewer — `recruiter` | `hiringManager` | `team` | `leadership`
    /// | `general` (open-typed; an unknown value is treated as `general`).
    pub audience: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiGenerationRecord {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
    // GenerationMeta fields
    #[serde(rename = "candidateName")]
    pub candidate_name: String,
    #[serde(rename = "jobTitle")]
    pub job_title: String,
    #[serde(rename = "companyName")]
    pub company_name: String,
    #[serde(rename = "resumeLanguage")]
    pub resume_language: String,
    #[serde(rename = "jobAdLanguage")]
    pub job_ad_language: String,
    #[serde(rename = "targetLanguage")]
    pub target_language: String,
    pub mismatch: bool,
    #[serde(rename = "topRequirements")]
    pub top_requirements: Vec<String>, // stored as JSON
    // Generation settings
    pub mode: String,
    // Content
    #[serde(rename = "resumeText")]
    pub resume_text: String,
    #[serde(rename = "coverLetterText")]
    pub cover_letter_text: String,
    #[serde(rename = "jobAd")]
    pub job_ad: String,
    // Application link — makes the record the single "application" aggregate: the
    // job it targets and the board it came from. `job_url` is what derives a found
    // job's `applied` flag (a matching url means the user generated for it).
    #[serde(rename = "jobUrl", default)]
    pub job_url: String,
    #[serde(default)]
    pub board: String,
    // Application extras — answered questions and the company-research brief used,
    // so the record is the full auditable application aggregate. Stored as JSON.
    #[serde(rename = "applicationAnswers", default)]
    pub application_answers: Vec<ApplicationAnswer>,
    #[serde(rename = "companyBrief", default)]
    pub company_brief: String,
    /// AI-suggested "questions to ask the interviewer" — the second assistant,
    /// distinct from `application_answers` above. Stored as JSON.
    #[serde(rename = "interviewQuestions", default)]
    pub interview_questions: Vec<InterviewQuestion>,
    /// The apply-by-email draft generated in the Application detail tab: the
    /// subject line and the body, kept as two plain columns (they are edited and
    /// copied independently by the UI, so a JSON blob would only add parsing).
    #[serde(rename = "emailSubject", default)]
    pub email_subject: String,
    #[serde(rename = "emailBody", default)]
    pub email_body: String,
    /// Parent Application FK (NULL when unlinked). Carried through export/import so
    /// a backup round-trip preserves the application↔generation link; otherwise
    /// `remove_for_application`/`detach_application` stop matching restored rows.
    #[serde(
        rename = "applicationId",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub application_id: Option<String>,
    /// Serialized JSON wrapper `{schemaVersion, pipeline, generatedAt, resume?,
    /// coverLetter?}` (renderer-owned shape) holding the deterministic
    /// content-quality report(s) — `''` = no report at all. A save merges its
    /// incoming wrapper onto the existing one PER TOP-LEVEL KEY
    /// (see [`merge_quality_report`]): a letter-only save overlays
    /// `coverLetter` (plus the envelope fields it always carries) and leaves a
    /// stored `resume` sub-report untouched, and vice versa. Each per-document
    /// slot carries its report AND its `sourceTextHash` together IN the slot
    /// (`{report, sourceTextHash}`) — the hash must live inside the top-level
    /// key it anchors, because this merge overlays whole top-level keys: a
    /// sibling hash map would be wholesale-replaced by a single-doc save,
    /// silently orphaning the OTHER doc's staleness anchor. The renderer uses
    /// the hash to flag a slot stale against the CURRENT résumé/letter text —
    /// this store never clears a report on a text edit, so staleness display
    /// is entirely the renderer's read-time job. A manual post-save text edit via
    /// `AiGenerationUpdateRequest` (`update_texts`) deliberately does NOT touch
    /// this column either, for the same reason. `default` keeps a
    /// pre-migration exported bundle importable.
    #[serde(rename = "qualityReport", default)]
    pub quality_report: String,
}

/// Merge an incoming per-job save into the existing application row: keep the
/// existing id + first-seen time, and take each incoming field only when it
/// carries content, so independent saves (résumé, cover, answers, brief) layer
/// onto one aggregate instead of clobbering each other. Pure — unit-tested.
pub(super) fn merge_application(
    existing: AiGenerationRecord,
    incoming: AiGenerationRecord,
) -> AiGenerationRecord {
    let pick = |inc: String, ex: String| if inc.trim().is_empty() { ex } else { inc };
    // `mismatch` is derived from the resume/jobAd language pair, so it is only
    // meaningful when the incoming save actually carries that pair. An
    // answers-only / interview-only save leaves the language fields blank and its
    // `mismatch` defaults to `false` — not a real verdict.
    let incoming_has_languages =
        !incoming.resume_language.trim().is_empty() && !incoming.job_ad_language.trim().is_empty();
    // Subject and body are ONE draft: the email surface always writes both
    // together, so they merge together. Picking them independently let a
    // regeneration whose output broke the `Subject:` line contract (parsed
    // subject = "") keep the PREVIOUS subject glued onto the NEW body — a
    // mismatched email the user never wrote. Either field carrying content means
    // "this save owns the draft"; neither means the save is about something else
    // (résumé, answers, interview questions) and the stored draft is kept.
    let (email_subject, email_body) =
        if incoming.email_subject.trim().is_empty() && incoming.email_body.trim().is_empty() {
            (existing.email_subject, existing.email_body)
        } else {
            (incoming.email_subject, incoming.email_body)
        };
    AiGenerationRecord {
        id: existing.id,
        created_at: existing.created_at,
        candidate_name: pick(incoming.candidate_name, existing.candidate_name),
        job_title: pick(incoming.job_title, existing.job_title),
        company_name: pick(incoming.company_name, existing.company_name),
        resume_language: pick(incoming.resume_language, existing.resume_language),
        job_ad_language: pick(incoming.job_ad_language, existing.job_ad_language),
        target_language: pick(incoming.target_language, existing.target_language),
        // Follow the incoming verdict ONLY when this save computed the language
        // pair — mirroring how the language fields themselves merge (`pick`), so a
        // corrected regeneration (`mismatch=false`) clears a stale warning while a
        // content-less save can't clobber a real prior verdict. The old
        // `incoming || existing` made a `true` permanent.
        mismatch: if incoming_has_languages {
            incoming.mismatch
        } else {
            existing.mismatch
        },
        top_requirements: if incoming.top_requirements.is_empty() {
            existing.top_requirements
        } else {
            incoming.top_requirements
        },
        mode: pick(incoming.mode, existing.mode),
        resume_text: pick(incoming.resume_text, existing.resume_text),
        cover_letter_text: pick(incoming.cover_letter_text, existing.cover_letter_text),
        job_ad: pick(incoming.job_ad, existing.job_ad),
        job_url: pick(incoming.job_url, existing.job_url),
        board: pick(incoming.board, existing.board),
        application_answers: if incoming.application_answers.is_empty() {
            existing.application_answers
        } else {
            incoming.application_answers
        },
        company_brief: pick(incoming.company_brief, existing.company_brief),
        interview_questions: if incoming.interview_questions.is_empty() {
            existing.interview_questions
        } else {
            incoming.interview_questions
        },
        // Merged as one atomic draft — see the binding above.
        email_subject,
        email_body,
        application_id: incoming.application_id.or(existing.application_id),
        quality_report: merge_quality_report(incoming.quality_report, existing.quality_report),
    }
}

pub fn make_generation_id() -> String {
    format!("gen-{}-{}", now_ms(), &Uuid::new_v4().to_string()[..8])
}
