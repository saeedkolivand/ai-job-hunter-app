use super::*;
use crate::applications::ApplicationStatus;

const NOW: u64 = 1_800_000_000_000;

fn candidate(id: &str, due: Option<u64>, notified: Option<u64>) -> FollowUpCandidate {
    FollowUpCandidate {
        id: id.to_string(),
        status: ApplicationStatus::Applied,
        title: "Engineer".to_string(),
        company: "Acme".to_string(),
        next_action_at: due,
        notified_at: notified,
    }
}

// ── should_notify ────────────────────────────────────────────────────────

#[test]
fn an_overdue_unnotified_reminder_fires() {
    assert!(should_notify(&candidate("a", Some(NOW - 1), None), NOW));
}

#[test]
fn a_reminder_due_exactly_now_fires() {
    // The boundary is inclusive — a reminder set for this instant is due.
    assert!(should_notify(&candidate("a", Some(NOW), None), NOW));
}

#[test]
fn a_future_reminder_does_not_fire() {
    assert!(!should_notify(&candidate("a", Some(NOW + 1), None), NOW));
}

#[test]
fn an_unset_reminder_never_fires() {
    assert!(!should_notify(&candidate("a", None, None), NOW));
    // Even a stale marker on a cleared reminder stays silent.
    assert!(!should_notify(&candidate("a", None, Some(NOW - 1)), NOW));
}

#[test]
fn an_already_notified_due_date_does_not_fire_again() {
    // The dedupe core: the marker is only cleared when the due date moves,
    // so a still-overdue reminder must NOT re-announce on every sweep.
    let c = candidate("a", Some(NOW - 10_000), Some(NOW - 5_000));
    assert!(!should_notify(&c, NOW));
}

#[test]
fn rescheduling_clears_the_marker_and_the_new_date_fires_once() {
    // `update_fields` nulls `notified_at` when the due date changes; from
    // this function's side that is simply a due row with no marker again.
    let rescheduled = candidate("a", Some(NOW - 1), None);
    assert!(should_notify(&rescheduled, NOW));
    let after_firing = candidate("a", Some(NOW - 1), Some(NOW));
    assert!(!should_notify(&after_firing, NOW));
}

#[test]
fn a_closed_pursuit_does_not_nag() {
    for status in [
        ApplicationStatus::Accepted,
        ApplicationStatus::Rejected,
        ApplicationStatus::Withdrawn,
    ] {
        let mut c = candidate("a", Some(NOW - 1), None);
        c.status = status;
        assert!(
            !should_notify(&c, NOW),
            "{status:?} is terminal — a stale reminder must stay silent"
        );
    }
    // `ghosted` is soft-terminal (a ghosted pursuit can revive), so it still
    // reminds — this is the exact `is_terminal` split.
    let mut ghosted = candidate("a", Some(NOW - 1), None);
    ghosted.status = ApplicationStatus::Ghosted;
    assert!(should_notify(&ghosted, NOW));
}

// ── due_follow_ups ───────────────────────────────────────────────────────

#[test]
fn a_sweep_selects_only_due_rows() {
    let selected = due_follow_ups(
        vec![
            candidate("due", Some(NOW - 1), None),
            candidate("future", Some(NOW + 60_000), None),
            candidate("already", Some(NOW - 1), Some(NOW - 1)),
            candidate("unset", None, None),
        ],
        NOW,
    );
    let ids: Vec<&str> = selected.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["due"]);
}

#[test]
fn a_sweep_is_capped_and_takes_the_most_overdue_first() {
    // One more than the cap, deliberately handed over newest-first so the
    // sort (not the input order) decides.
    let candidates: Vec<FollowUpCandidate> = (0..=MAX_PER_SWEEP)
        .map(|i| candidate(&format!("app{i}"), Some(NOW - (i as u64 + 1) * 1000), None))
        .collect();
    let selected = due_follow_ups(candidates, NOW);
    assert_eq!(selected.len(), MAX_PER_SWEEP, "one sweep is bounded");
    assert_eq!(
        selected[0].id,
        format!("app{MAX_PER_SWEEP}"),
        "the longest-overdue reminder must win the cap"
    );
    // The one that missed the cap keeps no marker, so the next sweep takes it.
    assert!(
        !selected.iter().any(|c| c.id == "app0"),
        "the least-overdue row is deferred, not dropped"
    );
}

// ── follow_up_body ───────────────────────────────────────────────────────

#[test]
fn body_degrades_gracefully_when_a_side_is_missing() {
    assert_eq!(follow_up_body("Engineer", "Acme"), "Engineer · Acme");
    assert_eq!(follow_up_body("", "Acme"), "Acme");
    assert_eq!(follow_up_body("Engineer", "  "), "Engineer");
    assert_eq!(follow_up_body("  ", ""), "Untitled application");
}

// ── claim_due (against a real store — no AppHandle needed) ───────────────

use crate::applications::ApplicationMeta;
use tempfile::TempDir;

/// Track one application and give it a follow-up due at `due`.
fn tracked_with_reminder(store: &ApplicationStore, company: &str, due: u64) -> String {
    let id = store
        .track_manual(
            "",
            "",
            &ApplicationMeta {
                company: company.to_string(),
                title: "Engineer".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
    store
        .update_fields(
            &id,
            None,
            Some(Some(due)),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    id
}

#[test]
fn a_claimed_reminder_is_not_claimed_again_by_the_next_sweep() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = tracked_with_reminder(&store, "Acme", NOW - 1);

    let first: Vec<String> = claim_due(&store, NOW).into_iter().map(|c| c.id).collect();
    assert_eq!(
        first,
        vec![id.clone()],
        "an overdue reminder is claimed once"
    );

    // The claim stamped the marker, so the very next sweep — same due date,
    // still overdue — must return nothing. This is the whole dedupe contract
    // end-to-end (read → claim → marker), not just `should_notify` in memory.
    assert!(
        claim_due(&store, NOW).is_empty(),
        "a claimed reminder must not be announced twice"
    );

    // Rescheduling re-arms it exactly once.
    store
        .update_fields(
            &id,
            None,
            Some(Some(NOW - 2)),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(claim_due(&store, NOW).len(), 1, "a new due date re-arms");
}

#[test]
fn a_terminal_pursuit_is_never_claimed_and_keeps_its_marker_clear() {
    let dir = TempDir::new().unwrap();
    let store = ApplicationStore::open(dir.path()).unwrap();
    let id = tracked_with_reminder(&store, "Acme", NOW - 1);
    store
        .set_status(&id, ApplicationStatus::Rejected, "")
        .unwrap();

    assert!(
        claim_due(&store, NOW).is_empty(),
        "a rejected pursuit must not nag"
    );
    // …and it is skipped WITHOUT being stamped, so reviving the pursuit
    // leaves the reminder announceable rather than permanently silenced.
    assert_eq!(
        store
            .follow_up_candidates()
            .first()
            .and_then(|c| c.notified_at),
        None,
        "a skipped terminal row must not be marked notified"
    );
    store
        .set_status(&id, ApplicationStatus::Interviewing, "")
        .unwrap();
    assert_eq!(claim_due(&store, NOW).len(), 1, "a revived pursuit reminds");
}
