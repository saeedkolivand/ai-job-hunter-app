use super::super::super::test_support::bridge_state;

/// Closes the TOCTOU the previous read-then-merge order left open (PR #1209 review): a
/// `settings.set` disabling `saveAnswersOnSubmit` must never land between the consent read and
/// the merge for an in-flight AUTO `answers.save`. `with_answers_save_consent_locked` holds
/// `optin_write_lock` — the SAME lock every consent setter already shares — across both, so a
/// concurrent flip can only run before this critical section starts or after it ends, never
/// interleaved with it.
#[test]
fn with_answers_save_consent_locked_blocks_a_concurrent_optin_flip() {
    let (_dir, state) = bridge_state();
    assert!(state.set_autofill_enabled(true));
    assert!(state.set_save_answers_on_submit_enabled(true));
    let state = std::sync::Arc::new(state);

    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let held = std::sync::Arc::clone(&state);
    let critical_section = std::thread::spawn(move || {
        held.with_answers_save_consent_locked(|autofill, save_on_submit| {
            entered_tx.send(save_on_submit).unwrap();
            release_rx.recv().unwrap();
            autofill && save_on_submit
        })
    });

    let seen = entered_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("the closure must run and report the pre-flip value");
    assert!(
        seen,
        "read the opt-in as still ON before any concurrent flip"
    );

    let setter = std::sync::Arc::clone(&state);
    let disable = std::thread::spawn(move || setter.set_save_answers_on_submit_enabled(false));

    // Give the setter thread every opportunity to run: if `optin_write_lock` were NOT held
    // across the read above, this trivial swap+persist would finish in microseconds. Still
    // unfinished after this margin proves it is genuinely blocked on the same lock, not merely
    // unscheduled.
    std::thread::sleep(std::time::Duration::from_millis(150));
    assert!(
        !disable.is_finished(),
        "a concurrent opt-in flip must block on `optin_write_lock`, not run while the \
         check-and-merge critical section still holds it"
    );

    release_tx.send(()).unwrap();
    assert!(
        critical_section.join().unwrap(),
        "the merge used the pre-flip (still-enabled) consent, exactly once"
    );
    assert!(
        disable.join().unwrap(),
        "the flip applies once the lock is free"
    );
    assert!(
        !state.save_answers_on_submit_enabled(),
        "the flip took effect only after the critical section released the lock"
    );
}
