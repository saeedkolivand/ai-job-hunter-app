//! Which re-surfaced jobs keep their prior note this run (#1446, slice 3).

use std::collections::{HashMap, HashSet};

use crate::autopilot::{needs_regeneration, Autopilot, FoundJob};
use crate::documents::keywords::description_is_blank;
use crate::scraping::boards::common::canonical_job_key;

fn key_of(j: &FoundJob) -> String {
    canonical_job_key(&j.url, &j.title, &j.company)
}

/// Canonical keys the notes loop must NOT pay for: every prior job, except one whose note is
/// stamped against a résumé or description that no longer matches. The description compared is
/// the one the merge will keep: the incoming copy's when it has real text, else the prior row's.
/// A legacy unstamped note stays settled (labelled stale on read, never mass-regenerated), and a
/// row held for identity review keeps its content, so its note cannot change either.
pub(crate) fn settled_note_keys(autopilot: &Autopilot, incoming: &[FoundJob]) -> HashSet<String> {
    let resume = autopilot.resume_text.as_deref().unwrap_or("");
    let incoming_by_key: HashMap<String, &FoundJob> =
        incoming.iter().map(|j| (key_of(j), j)).collect();
    autopilot
        .found_jobs
        .iter()
        .filter_map(|prior| {
            let key = key_of(prior);
            let fresh = incoming_by_key
                .get(&key)
                .and_then(|j| j.description.as_deref())
                .filter(|d| !description_is_blank(Some(d)));
            let description = fresh.or(prior.description.as_deref());
            let regenerate =
                prior.identity_review.is_none() && needs_regeneration(prior, resume, description);
            (!regenerate).then_some(key)
        })
        .collect()
}

/// Give a re-surfaced job with a blank description the prior row's text — the description the
/// merge keeps — so a regenerated note is stamped against the text it will sit beside.
pub(crate) fn fill_blank_descriptions(autopilot: &Autopilot, incoming: &mut [FoundJob]) {
    let prior: HashMap<String, &str> = autopilot
        .found_jobs
        .iter()
        .filter_map(|p| Some((key_of(p), p.description.as_deref()?)))
        .filter(|(_, d)| !description_is_blank(Some(d)))
        .collect();
    for job in incoming
        .iter_mut()
        .filter(|j| description_is_blank(j.description.as_deref()))
    {
        if let Some(d) = prior.get(&key_of(job)) {
            job.description = Some((*d).to_string());
        }
    }
}

#[cfg(test)]
mod tests;
