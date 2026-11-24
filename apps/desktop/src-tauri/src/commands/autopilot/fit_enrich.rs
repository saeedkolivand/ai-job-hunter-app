//! Read-time derivation of `FoundJob::notes_stale` (#1446): a note is stale when the résumé a run
//! would use NOW, or the job's description, no longer matches the stamp stored with the note.

use tauri::AppHandle;

use super::guards::resume_for_run;
use crate::autopilot::{notes_stale, Autopilot};

/// Mark every found job's `notes_stale` against `resume`.
pub(super) fn mark_notes_stale(ap: &mut Autopilot, resume: &str) {
    for job in ap.found_jobs.iter_mut() {
        job.notes_stale = notes_stale(job, resume);
    }
}

/// [`mark_notes_stale`] over a list, resolving each autopilot's effective résumé (its own text,
/// else the default document). Skips autopilots with no notes, so most never read the document.
pub(super) fn enrich_notes_stale(app: &AppHandle, list: &mut [Autopilot]) {
    for ap in list
        .iter_mut()
        .filter(|a| a.found_jobs.iter().any(|j| j.assistant_notes.is_some()))
    {
        let resume = resume_for_run(app, ap.resume_text.as_deref()).unwrap_or_default();
        mark_notes_stale(ap, &resume);
    }
}

#[cfg(test)]
mod tests;
