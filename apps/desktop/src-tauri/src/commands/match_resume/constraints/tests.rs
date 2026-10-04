use super::*;

fn posting(location: Option<&str>, board_remote: bool) -> PostingFacts {
    PostingFacts {
        location: location.map(str::to_string),
        board_remote,
    }
}

fn candidate(location: Option<&str>) -> CandidateFacts {
    CandidateFacts {
        location: location.map(str::to_string),
    }
}

/// The one shipped check, by id.
fn only(checks: &[ConstraintCheck]) -> &ConstraintCheck {
    assert_eq!(checks.len(), 1, "exactly one constraint ships today");
    assert_eq!(checks[0].id(), "preferredLocation");
    &checks[0]
}

/// Status for one (preference, posting location, board remote flag) triple.
fn status_of(pref: &str, posting_location: &str, board_remote: bool) -> ConstraintStatus {
    let checks = evaluate(
        &posting(Some(posting_location), board_remote),
        &candidate(Some(pref)),
    );
    only(&checks).status()
}

mod contract;
mod location;
mod payload;
