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
