use crate::pipeline::budget::Budget;

/// The run deadline is the LARGER of the budget floor and the effort-scaled
/// allowance, so neither a raised budget nor a high-effort run loses its time.
///
/// Mutation check: return `effort_scaled` unconditionally and the floor
/// assertion fails; return `budget.run_timeout` and the scaled one does.
#[test]
fn the_run_deadline_takes_the_larger_of_the_floor_and_the_scaled_allowance() {
    use std::time::Duration;
    let budget = Budget::RESUME_QUALITY;
    assert_eq!(
        super::super::run_deadline(budget, Duration::from_secs(60)),
        budget.run_timeout,
        "a scaled allowance below the floor must not shorten a run"
    );
    let generous = budget.run_timeout + Duration::from_secs(1_800);
    assert_eq!(super::super::run_deadline(budget, generous), generous);
}
