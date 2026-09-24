//! Keep each autopilot's found jobs bounded (#1277, owner's call: the 500
//! newest, nothing exempt). Without a cap they grew without limit: 531 to
//! 3,946 per autopilot on one machine.

use std::collections::HashSet;

use super::FoundJob;

pub(super) const MAX_FOUND_JOBS: usize = 500;

/// Keep the [`MAX_FOUND_JOBS`] most recently found jobs, dropping the oldest by
/// `found_at`. The kept jobs stay in their existing order; on equal `found_at`
/// the one earlier in the list is kept.
pub(super) fn cap_found_jobs(jobs: &mut Vec<FoundJob>) {
    if jobs.len() <= MAX_FOUND_JOBS {
        return;
    }
    let mut by_age: Vec<usize> = (0..jobs.len()).collect();
    by_age.sort_by(|&a, &b| jobs[b].found_at.cmp(&jobs[a].found_at).then(a.cmp(&b)));
    let keep: HashSet<usize> = by_age.into_iter().take(MAX_FOUND_JOBS).collect();
    let mut position = 0;
    jobs.retain(|_| {
        let kept = keep.contains(&position);
        position += 1;
        kept
    });
}
