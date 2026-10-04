//! Central, conservative work-type post-filter — the work-type sibling of
//! [`super::location_filter`], deliberately the same shape and the same
//! conservatism.
//!
//! Every posting's work arrangement is classified from DECLARED data only —
//! `extra["workType"]`, the value each board writes at parse time. Board
//! coverage drifts as boards are added or their upstream fields change, so
//! each board's own parse function under `scraping/boards/` is authoritative
//! for what that board declares — no count is repeated here, because one
//! already rotted. A board that declares nothing on the endpoint we call
//! simply never writes the key, and its postings read as
//! [`WorkTypeVerdict::Unknown`].
//!
//! There is deliberately no text inference:
//! keyword matching flips true on "this role is NOT remote" and
//! "remote-first culture, 3 days in office" just as readily as it flips true
//! on the real thing, and there is no way to tell the two apart from outside
//! the posting.
//!
//! [`WorkTypeVerdict::Unknown`] is a value, not `Option` sugar, and it is
//! KEPT — never dropped — by policy. It is the MAJORITY state, not an edge
//! case: Greenhouse and Personio declare no workplace field at all, and
//! roughly three quarters of freehire's corpus carries no `work_mode`.
//! Dropping an undeclared posting would silently discard a large share of
//! genuinely-remote jobs no board bothered to label — the same conservative
//! posture [`super::location_filter`] already ships (never drop what wasn't
//! POSITIVELY contradicted).
//!
//! This module does not read [`super::location_filter::REMOTE_MARKERS`] and
//! must not start to: that list's one caller treats a hit as "never drop this
//! row" for a LOCATION comparison, a different question with a different
//! safe direction, and widening it here would silently change a shipped
//! feature with nothing failing.
//!
//! Wired end-to-end: [`parse_work_type`] backs the `ScrapeBoardsRequest`/
//! `AutopilotTarget` IPC boundary, every board that declares a workplace
//! value writes `extra["workType"]` through it, and [`filter_postings`] (via
//! [`work_type_mismatch`]/[`work_type_verdict`]) is the central post-filter
//! `scraping::engine::mod` composes into its `keep_item` predicate, mirroring
//! `location_filter`.

use crate::scraping::types::{JobPosting, WorkType};

/// Thin alias for [`WorkType::from_str`] — the canonical parser now lives on
/// `WorkType` itself in `scraping::types` (the module's front door, per
/// `docs/architecture-rules.md` §L1) so a caller outside `scraping::engine`
/// reaches it via `str::parse`/`WorkType::from_str` directly instead of
/// importing this module's internals. Kept here, at this exact name, only so
/// this module's OWN tests and the per-board callers in `scraping::boards::*`
/// (which already sit inside the same L1 domain) keep reading
/// `parse_work_type(s)` rather than churning every call site for a rename.
/// Returns `None` for anything unrecognised — NEVER a default; see
/// [`WorkType::from_str`]'s doc for why.
pub(crate) fn parse_work_type(raw: &str) -> Option<WorkType> {
    raw.parse().ok()
}

/// What reading a posting's declared work arrangement actually established.
/// FOUR answers, not three — [`WorkTypeVerdict::Unknown`] is a distinct
/// outcome from a decided one, and collapsing the two is how an absent fact
/// becomes a stated one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkTypeVerdict {
    Remote,
    Hybrid,
    OnSite,
    /// No workplace field was declared, or a declared value this normalizer
    /// doesn't recognise. Never a pass and never a fail — see the module doc
    /// for why this is KEPT, not dropped.
    Unknown,
}

/// Reads ONLY `posting.extra["workType"]` — the value the board DECLARED at
/// parse time. Deliberately no fallback to `extra["remote"]`, location text,
/// or the description: see the module doc for why text inference is not
/// built here.
pub(crate) fn work_type_verdict(posting: &JobPosting) -> WorkTypeVerdict {
    posting
        .extra
        .get("workType")
        .and_then(|v| v.as_str())
        .and_then(parse_work_type)
        .map(|wt| match wt {
            WorkType::Remote => WorkTypeVerdict::Remote,
            WorkType::Hybrid => WorkTypeVerdict::Hybrid,
            WorkType::OnSite => WorkTypeVerdict::OnSite,
        })
        .unwrap_or(WorkTypeVerdict::Unknown)
}

/// True ONLY when the posting has a DECIDED verdict absent from `wanted`.
/// [`WorkTypeVerdict::Unknown`] NEVER drops — this is the load-bearing policy
/// decision (see the module doc); a mutation that removes this must fail a
/// test. An empty `wanted` is a no-op: nothing was requested, so nothing is
/// filtered (the same "empty/absent = no filter" the shared contract
/// documents).
pub(crate) fn work_type_mismatch(posting: &JobPosting, wanted: &[WorkType]) -> bool {
    if wanted.is_empty() {
        return false;
    }
    match work_type_verdict(posting) {
        WorkTypeVerdict::Unknown => false,
        WorkTypeVerdict::Remote => !wanted.contains(&WorkType::Remote),
        WorkTypeVerdict::Hybrid => !wanted.contains(&WorkType::Hybrid),
        WorkTypeVerdict::OnSite => !wanted.contains(&WorkType::OnSite),
    }
}

/// Drop postings whose declared work type is absent from `wanted`, returning
/// the kept postings (in input order) and the number dropped. Pure — see
/// [`work_type_mismatch`].
pub(crate) fn filter_postings(
    postings: Vec<JobPosting>,
    wanted: &[WorkType],
) -> (Vec<JobPosting>, usize) {
    let before = postings.len();
    let kept: Vec<JobPosting> = postings
        .into_iter()
        .filter(|p| !work_type_mismatch(p, wanted))
        .collect();
    let dropped = before - kept.len();
    (kept, dropped)
}

#[cfg(test)]
mod tests;
