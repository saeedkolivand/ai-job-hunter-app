use super::*;

#[test]
fn retry_policy_clamps_to_at_least_one_attempt() {
    assert_eq!(RetryPolicy::new(0).max_attempts, 1);
    assert_eq!(RetryPolicy::new(1).max_attempts, 1);
    assert_eq!(RetryPolicy::new(4).max_attempts, 4);
}
