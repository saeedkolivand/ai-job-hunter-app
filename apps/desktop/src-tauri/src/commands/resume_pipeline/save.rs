//! Whether — and on what terms — a finished run's documents may REPLACE the
//! posting's saved ones.
//!
//! Split out of [`super`] as one unit because that is what it is: three rules
//! and the two user-facing refusals they produce, shared by
//! [`super::persist_document`] (which acts on the verdict) and [`super::execute`]
//! (which has to report it). Keeping them together is what makes "these two
//! callers cannot disagree about whether this run saved" a property of the
//! module rather than a comment.

use crate::pipeline::resume::types::SectionKey;

/// What [`super::persist_document`] will do with this run's document, decided
/// from the two documents, the posting url, and which documents the run was
/// asked for.
///
/// Three outcomes rather than a bool, because two of them mean opposite things
/// to the RUN: `Nothing` is benign (an unlinked run is session-only by design,
/// and a run that produced none of the documents it was asked for already
/// reports that on its own path), while `Refused` means the run produced a
/// document and it was rejected — which must not come out as a successful
/// completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaveVerdict {
    Save,
    /// There was nothing to save, or nowhere to save it.
    Nothing,
    /// There WAS a document, and it was rejected — for the user-facing reason
    /// carried here, surfaced verbatim by `execute` (see
    /// [`LOST_WORK_HISTORY_MESSAGE`]/[`LEAKED_FENCE_TAG_MESSAGE`]).
    Refused(&'static str),
}

/// [`SaveVerdict::Refused`] reason: [`is_persistable`] rejected the draft for
/// dropping the source's whole work history.
pub(crate) const LOST_WORK_HISTORY_MESSAGE: &str =
    "The generated résumé came back without any of your work \
    history, so your saved document was left unchanged. Try again.";

/// [`SaveVerdict::Refused`] reason: the résumé or letter echoed one of the
/// internal prompt-fence wrapper tags (`<generated_resume>`,
/// `<candidate_resume>`, …) instead of real content — see
/// [`crate::prompt_fence::contains_fence_tag`]. `draft`/`cover_letter` are the
/// SOLE producers of these documents (unlike `humanize`/`repair`, which can
/// discard a bad candidate and keep the last-good text they started from), so
/// `save_verdict` is the last chokepoint that can catch a leak here before it
/// reaches the saved aggregate and the exported PDF. Refusing to save —
/// rather than saving with a flagged report — is the deliberate choice: it
/// costs the run producing nothing this time, but a raw framework artifact
/// reaching an employer is worse than a document the user has to retry, and
/// it keeps this gate consistent with [`LOST_WORK_HISTORY_MESSAGE`] above
/// rather than inventing a second, weaker failure mode for a defect that is
/// arguably more visible to the reader.
pub(crate) const LEAKED_FENCE_TAG_MESSAGE: &str =
    "The generated document came back with an internal \
    formatting artifact that must never reach your résumé or cover letter, so your saved \
    document was left unchanged. Try again.";

/// One definition of "will this save", shared by `persist_document` (which
/// acts on it) and [`super::execute`] (which has to report it). `letter` is checked
/// independently of `draft`: `persist_document` writes both documents into
/// ONE `AiGenerationRecord`, so there is no lower-granularity save to fall
/// back to — a defect in EITHER document refuses the whole save, the same way
/// [`is_persistable`] already treats a résumé-only defect as blocking it.
///
/// **`resume_in_run` scopes the two RÉSUMÉ rules, and nothing else.** A
/// cover-letter-only run (`includeResume: false`) reaches here with an empty
/// `draft` BY DESIGN, and both résumé rules read an empty draft as a failure:
/// the emptiness check would return [`SaveVerdict::Nothing`] and throw away the
/// letter the run just paid for, and [`is_persistable`] would then call it
/// `Refused` — a red "your résumé came back without any of your work history"
/// banner on a run that never asked for a résumé. Neither question applies to a
/// document that was deliberately not written, so neither is asked. The
/// fence-tag check below is NOT scoped: it guards both documents, and it is the
/// last chokepoint before an internal prompt artifact reaches an exported one.
pub(crate) fn save_verdict(
    source_resume: &str,
    draft: &str,
    letter: &str,
    job_url: &str,
    resume_in_run: bool,
) -> SaveVerdict {
    if job_url.trim().is_empty() {
        return SaveVerdict::Nothing;
    }
    // "Nothing to save" is asked of the document this run was ASKED to write,
    // never of the résumé unconditionally. Deliberately NOT
    // `draft.is_empty() && letter.is_empty()`: that reads the same for a
    // cover-only run but silently CHANGES a résumé-bearing one whose draft came
    // back empty — today that saves nothing even when a letter is present, and
    // it must keep saving nothing, because `is_persistable` below can no longer
    // speak for a draft that isn't there.
    let produced_nothing = if resume_in_run {
        draft.trim().is_empty()
    } else {
        letter.trim().is_empty()
    };
    if produced_nothing {
        return SaveVerdict::Nothing;
    }
    // Source-RELATIVE, and only for a run that HAS a résumé: an ABSENT résumé
    // is not a lost work history, it is a run that was never asked for one.
    if resume_in_run && !is_persistable(source_resume, draft) {
        return SaveVerdict::Refused(LOST_WORK_HISTORY_MESSAGE);
    }
    // `humanize`/`sections` already gate their OWN rewrite candidates against
    // an echoed fence tag (`is_usable_rewrite`, the splice guard in
    // `sections.rs`) — but `draft`/`cover_letter` are what FIRST produce this
    // text, and neither had a shape check of its own before this gate.
    if crate::prompt_fence::contains_fence_tag(draft)
        || crate::prompt_fence::contains_fence_tag(letter)
    {
        return SaveVerdict::Refused(LEAKED_FENCE_TAG_MESSAGE);
    }
    SaveVerdict::Save
}

/// Whether a run's document may OVERWRITE the posting's saved one.
///
/// One rule, and it is RELATIVE TO THE SOURCE: **refuse when the candidate's
/// own résumé has an employment section and the generated document does not.**
/// Everything else about completeness is a judgement the report already makes
/// visible (a missing employer is a `factual.dropped_role` Critical and
/// `needsReview`), but a document that lost ALL of a real work history is not a
/// shorter résumé — it is not one — and this save has no versioning to undo it
/// with.
///
/// **Why the source and not the run's outcome.** Two earlier versions of this
/// gate read the run instead of the documents, and each was wrong in its own
/// direction:
///
/// * keying on the recorded stop REASON refused a perfectly good run whose
///   source simply has no employment section (a new graduate, an academic CV)
///   the moment its repair round hit the daily cap — `stages::repair` records
///   `Budgeted`/`RunTimeout` for a stop it RECOVERED from and then returns
///   `Ok(())`. Status `completed`, document unchanged, no explanation anywhere;
/// * keying on the run's OUTCOME missed the case the gate exists for. The
///   removed `max` depth's now-deleted per-section fan-out treated a
///   daily-cap refusal as `StoppedReason::Budgeted`, broke, and returned
///   `Ok(())` — and nothing downstream converted that into an `Err`. So a max
///   run that hit the cap right after Summary produced a summary-only
///   document with `outcome == Ok`, and it overwrote the saved résumé.
///
/// Comparing the two DOCUMENTS answers both at once and needs no run state:
/// a source with no work history can never trip it, and a truncated draft
/// over a real one always does — however the run happened to end.
///
/// Both sides go through the SAME [`sections::find`] seam, so the
/// undated-entry caveat (an entry with no date column is not a
/// `LineKind::JobEntry`) applies equally to each and cannot create a false
/// asymmetry.
pub(crate) fn is_persistable(source_resume: &str, draft: &str) -> bool {
    has_work_history(draft) || !has_work_history(source_resume)
}

/// Whether `text` has an employment section with anything under it.
///
/// The SECTION with a body, not a per-entry line range: `LineKind::JobEntry`
/// legitimately fails for an entry with no date column, and a résumé whose
/// dates the source never carried is a real document rather than an empty
/// one.
fn has_work_history(text: &str) -> bool {
    let split = crate::pipeline::resume::stages::sections::split(text);
    let lines: Vec<&str> = text.lines().collect();
    crate::pipeline::resume::stages::sections::find(&split, SectionKey::Experience(0)).is_some_and(
        |section| {
            section
                .text(&lines)
                .lines()
                .skip(1)
                .any(|line| !line.trim().is_empty())
        },
    )
}
