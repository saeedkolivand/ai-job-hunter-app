use super::*;

#[test]
fn first_crash_reloads() {
    assert!(should_reload(None, Instant::now()));
}

#[test]
fn crash_inside_cooldown_is_a_loop_and_does_not_reload() {
    let t = Instant::now();
    assert!(!should_reload(
        Some(t),
        t + RELOAD_COOLDOWN - Duration::from_secs(1)
    ));
}

#[test]
fn crash_after_cooldown_reloads_again() {
    let t = Instant::now();
    assert!(should_reload(Some(t), t + RELOAD_COOLDOWN));
}

#[cfg(any(windows, target_os = "linux"))]
#[test]
fn admit_records_the_reload_and_blocks_the_next_crash() {
    let last = std::sync::Mutex::new(None);
    assert!(admit(&last));
    assert!(last.lock().unwrap().is_some());
    assert!(!admit(&last));
}

#[test]
fn clock_going_backwards_does_not_panic_and_blocks() {
    let t = Instant::now();
    assert!(!should_reload(Some(t + Duration::from_secs(5)), t));
}
