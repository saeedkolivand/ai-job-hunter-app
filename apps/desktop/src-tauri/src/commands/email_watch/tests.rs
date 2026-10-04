use super::{is_check_now_rate_limited, strip_ascii_whitespace, MIN_CHECK_NOW_GAP_MS};

#[test]
fn strip_ascii_whitespace_removes_googles_4_group_spacing() {
    assert_eq!(
        strip_ascii_whitespace("abcd efgh ijkl mnop"),
        "abcdefghijklmnop"
    );
}

#[test]
fn strip_ascii_whitespace_is_a_no_op_without_whitespace() {
    assert_eq!(
        strip_ascii_whitespace("abcdefghijklmnop"),
        "abcdefghijklmnop"
    );
}

#[test]
fn strip_ascii_whitespace_also_removes_leading_trailing_and_tabs() {
    assert_eq!(strip_ascii_whitespace("  ab\tcd\n"), "abcd");
}

#[test]
fn check_now_is_never_rate_limited_when_nothing_has_run_yet() {
    assert!(!is_check_now_rate_limited(None, 1_000_000));
}

#[test]
fn check_now_is_rate_limited_within_the_min_gap() {
    let now = 1_000_000_000u64;
    let last = now - (MIN_CHECK_NOW_GAP_MS / 2);
    assert!(is_check_now_rate_limited(Some(last), now));
}

#[test]
fn check_now_is_allowed_once_the_min_gap_has_elapsed() {
    let now = 1_000_000_000u64;
    let last = now - MIN_CHECK_NOW_GAP_MS;
    assert!(!is_check_now_rate_limited(Some(last), now));
}
