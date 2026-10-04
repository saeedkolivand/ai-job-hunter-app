use super::*;

#[test]
fn status_from_id_is_relaxed_and_round_trips() {
    for &s in ApplicationStatus::ALL {
        assert_eq!(ApplicationStatus::from_id(s.as_id()), s);
    }
    assert_eq!(
        ApplicationStatus::from_id("some_future_stage"),
        ApplicationStatus::Saved
    );
}

#[test]
fn terminal_and_pre_apply_classification() {
    assert!(ApplicationStatus::Accepted.is_terminal());
    assert!(ApplicationStatus::Rejected.is_terminal());
    assert!(ApplicationStatus::Withdrawn.is_terminal());
    assert!(!ApplicationStatus::Ghosted.is_terminal());
    assert!(!ApplicationStatus::Applied.is_terminal());
    assert!(ApplicationStatus::Saved.is_pre_apply());
    assert!(!ApplicationStatus::Applied.is_pre_apply());
}

/// Parity guard: the Rust stage registry order/ids must match the shared-TS
/// `APPLICATION_STAGES`. The expected list is HARD-CODED from the TS `as const`
/// so any drift on either side fails the build (see
/// packages/shared/src/types/index.ts → APPLICATION_STAGES).
#[test]
fn rust_stage_registry_matches_shared_ts() {
    let expected = [
        "saved",
        "applied",
        "screening",
        "interviewing",
        "offer",
        "accepted",
        "rejected",
        "ghosted",
        "withdrawn",
    ];
    let actual: Vec<&str> = ApplicationStatus::ALL.iter().map(|s| s.as_id()).collect();
    assert_eq!(
        actual, expected,
        "ApplicationStatus::ALL drifted from shared-TS APPLICATION_STAGES"
    );
}
