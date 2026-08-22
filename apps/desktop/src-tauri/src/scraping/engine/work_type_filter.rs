//! Central, conservative work-type post-filter — the work-type sibling of
//! [`super::location_filter`], deliberately the same shape and the same
//! conservatism.
//!
//! Every posting's work arrangement is classified from DECLARED data only —
//! `extra["workType"]`, the value each board writes at parse time (per-board
//! mapping is a later phase; today no board writes it, so every posting reads
//! as [`WorkTypeVerdict::Unknown`]). There is deliberately no text inference:
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

/// Lowercase, strip `-`/`_`/whitespace, then match. Returns `None` for
/// anything unrecognised — NEVER a default; a spelling drift silently routing
/// a whole board to the wrong bucket is worse than an admitted
/// [`WorkTypeVerdict::Unknown`]. Every ATS spells this differently: `on-site`
/// (Lever's README) vs `onsite` (Lever's actual wire) vs `OnSite` (Ashby) vs
/// `ONSITE` (SmartRecruiters) vs `on_site` (Workable) vs `On-site` (LinkedIn's
/// feed) — one normalizer so every board's per-parse mapping (a later phase)
/// reaches the SAME matcher instead of six drifting copies.
pub(crate) fn parse_work_type(raw: &str) -> Option<WorkType> {
    let folded: String = raw
        .chars()
        .filter(|c| !matches!(c, '-' | '_') && !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    match folded.as_str() {
        "remote" => Some(WorkType::Remote),
        "hybrid" => Some(WorkType::Hybrid),
        "onsite" => Some(WorkType::OnSite),
        _ => None,
    }
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
mod test {
    use super::*;
    use std::collections::HashMap;

    fn posting(work_type: Option<&str>) -> JobPosting {
        let mut extra = HashMap::new();
        if let Some(wt) = work_type {
            extra.insert("workType".to_string(), serde_json::json!(wt));
        }
        JobPosting {
            id: "b:1".into(),
            external_id: None,
            title: "Engineer".into(),
            company: "Acme".into(),
            location: None,
            url: "https://acme.example/1".into(),
            source: "b".into(),
            description: None,
            requirements: None,
            posted_at: None,
            captured_at: 0,
            extra,
        }
    }

    // ── parse_work_type: truth table over every real vendor spelling ──────────

    #[test]
    fn parse_work_type_recognises_every_real_vendor_spelling() {
        let cases: &[(&str, WorkType)] = &[
            ("on-site", WorkType::OnSite), // Lever's README
            ("onsite", WorkType::OnSite),  // Lever's actual wire
            ("OnSite", WorkType::OnSite),  // Ashby
            ("ONSITE", WorkType::OnSite),  // SmartRecruiters
            ("on_site", WorkType::OnSite), // Workable
            ("On-site", WorkType::OnSite), // LinkedIn's feed
            ("remote", WorkType::Remote),
            ("Remote", WorkType::Remote),
            ("hybrid", WorkType::Hybrid),
            ("Hybrid", WorkType::Hybrid),
        ];
        for (raw, want) in cases {
            assert_eq!(
                parse_work_type(raw),
                Some(*want),
                "parse_work_type({raw:?}) must resolve to {want:?}"
            );
        }
    }

    #[test]
    fn parse_work_type_returns_none_for_unrecognised_input_never_a_default() {
        for raw in ["unspecified", "Zzz", "", "remote-ish", "part-time"] {
            assert_eq!(
                parse_work_type(raw),
                None,
                "parse_work_type({raw:?}) must be None, not a guessed default"
            );
        }
    }

    // ── work_type_mismatch: Unknown never drops, for every wanted set ─────────

    #[test]
    fn unknown_verdict_never_drops_for_any_wanted_set() {
        let undeclared = posting(None);
        let unrecognised = posting(Some("flexible")); // declared but unparseable
        let wanted_sets: &[&[WorkType]] = &[
            &[],
            &[WorkType::Remote],
            &[WorkType::Hybrid],
            &[WorkType::OnSite],
            &[WorkType::Remote, WorkType::Hybrid],
            &[WorkType::Remote, WorkType::OnSite],
            &[WorkType::Hybrid, WorkType::OnSite],
            &[WorkType::Remote, WorkType::Hybrid, WorkType::OnSite],
        ];
        for wanted in wanted_sets {
            assert_eq!(work_type_verdict(&undeclared), WorkTypeVerdict::Unknown);
            assert!(
                !work_type_mismatch(&undeclared, wanted),
                "undeclared posting must never drop for wanted={wanted:?}"
            );
            assert_eq!(work_type_verdict(&unrecognised), WorkTypeVerdict::Unknown);
            assert!(
                !work_type_mismatch(&unrecognised, wanted),
                "unrecognised-value posting must never drop for wanted={wanted:?}"
            );
        }
    }

    // ── empty wanted set is a no-op ────────────────────────────────────────────

    #[test]
    fn empty_wanted_set_drops_nothing() {
        for wt in [Some("remote"), Some("hybrid"), Some("on-site"), None] {
            assert!(
                !work_type_mismatch(&posting(wt), &[]),
                "empty wanted must never drop {wt:?}"
            );
        }
    }

    // ── a decided verdict absent from wanted DOES drop ─────────────────────────

    #[test]
    fn decided_verdict_drops_exactly_when_absent_from_wanted() {
        let remote = posting(Some("remote"));
        let hybrid = posting(Some("hybrid"));
        let onsite = posting(Some("on-site"));

        // Kept: declared type is in the wanted set.
        assert!(!work_type_mismatch(&remote, &[WorkType::Remote]));
        assert!(!work_type_mismatch(
            &hybrid,
            &[WorkType::Hybrid, WorkType::OnSite]
        ));
        assert!(!work_type_mismatch(&onsite, &[WorkType::OnSite]));

        // Dropped: declared type is absent from the wanted set.
        assert!(work_type_mismatch(&remote, &[WorkType::OnSite]));
        assert!(work_type_mismatch(
            &hybrid,
            &[WorkType::Remote, WorkType::OnSite]
        ));
        assert!(work_type_mismatch(
            &onsite,
            &[WorkType::Remote, WorkType::Hybrid]
        ));

        // Selecting all three behaves like selecting none: nothing decided drops.
        let all = [WorkType::Remote, WorkType::Hybrid, WorkType::OnSite];
        assert!(!work_type_mismatch(&remote, &all));
        assert!(!work_type_mismatch(&hybrid, &all));
        assert!(!work_type_mismatch(&onsite, &all));
    }

    // ── filter_postings: accurate drop count, order preserved ─────────────────

    #[test]
    fn filter_postings_counts_drops_and_keeps_order() {
        let postings = vec![
            posting(Some("remote")),  // keep
            posting(Some("on-site")), // drop
            posting(None),            // keep (undeclared)
            posting(Some("hybrid")),  // drop
            posting(Some("remote")),  // keep
        ];
        let (kept, dropped) = filter_postings(postings, &[WorkType::Remote]);
        assert_eq!(dropped, 2);
        assert_eq!(kept.len(), 3);
        assert_eq!(
            kept.iter()
                .map(|p| p.extra.get("workType").and_then(|v| v.as_str()))
                .collect::<Vec<_>>(),
            vec![Some("remote"), None, Some("remote")]
        );
    }

    #[test]
    fn filter_postings_with_empty_wanted_returns_zero_drops() {
        let postings = vec![
            posting(Some("on-site")),
            posting(None),
            posting(Some("hybrid")),
        ];
        let (kept, dropped) = filter_postings(postings, &[]);
        assert_eq!(dropped, 0);
        assert_eq!(kept.len(), 3);
    }
}
