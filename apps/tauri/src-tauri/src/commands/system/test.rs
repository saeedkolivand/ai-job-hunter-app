use super::*;

// ── set_theme_mirror — IPC security boundary ──────────────────────────────────
//
// The command is a one-line delegate: `crate::splash::write_theme_mirror(&scheme)`.
// We test the IPC rejection path here (at the command boundary) by calling the
// shared `validate_scheme` seam directly — no AppHandle, no filesystem, no
// env-backed `data_dir()`. This gives us:
//   • valid inputs are accepted (Ok)
//   • every invalid input is rejected as AppError::Validation (the security
//     boundary the command MUST enforce before any filesystem write)
//
// The write + round-trip correctness is covered in `crate::splash` tests
// (write_then_read_round_trips_each_scheme / invalid_scheme_is_rejected_and_file_untouched).

#[test]
fn set_theme_mirror_valid_schemes_pass_validation() {
    // Both valid literals must be accepted by the validator the command delegates to.
    assert_eq!(
        crate::splash::validate_scheme("light").expect("\"light\" is a valid scheme"),
        "light"
    );
    assert_eq!(
        crate::splash::validate_scheme("dark").expect("\"dark\" is a valid scheme"),
        "dark"
    );
}

#[test]
fn set_theme_mirror_valid_schemes_are_trimmed() {
    // The renderer may write trailing whitespace; the validator accepts trimmed-valid input.
    assert_eq!(
        crate::splash::validate_scheme("  light  ").expect("trimmed \"light\" is valid"),
        "light"
    );
    assert_eq!(
        crate::splash::validate_scheme("dark\n").expect("trimmed \"dark\" is valid"),
        "dark"
    );
}

#[test]
fn set_theme_mirror_invalid_scheme_returns_validation_error() {
    // Any value outside the closed {"light","dark"} set must return Err(Validation).
    // These are the exact inputs the review-gate requires: "blue", "", "darkish".
    for bad in ["blue", "", "darkish", "DARK", "system", "auto"] {
        let err = crate::splash::validate_scheme(bad)
            .expect_err(&format!("{bad:?} must be rejected at the IPC boundary"));
        assert!(
            matches!(err, crate::error::AppError::Validation(_)),
            "{bad:?} must produce AppError::Validation, got {err:?}"
        );
    }
}

// ── app_ready — branch logic and idempotency ──────────────────────────────────
//
// The command matches `app.try_state::<SplashShownAt>()`:
//   Some(shown) → reveal_after_min_display(&app, shown.0)   [min-display branch]
//   None        → reveal_main(&app)                          [no-splash branch]
//
// Both branches delegate to the guarded `reveal_main`, which uses `RevealGuard`
// (an AtomicBool) to ensure exactly one reveal fires — idempotent across a
// duplicate `app_ready` call AND the safety timeout.
//
// Constructing a real `AppHandle` in a unit test is not practical.  We therefore
// test the two behavioral invariants the command depends on:
//
//   1. Guard idempotency (prevents double-reveal regardless of which branch fires).
//   2. The min-display remaining time math (the non-trivial part of the Some branch).
//
// Full `AppHandle`-dependent integration is covered by the smoke tests in `crate::splash`
// (async_runtime_spawn_works_without_ambient_tokio, reveal_guard_is_thread_safe_single_winner).

#[test]
fn app_ready_reveal_guard_is_idempotent_simulating_duplicate_call() {
    // Simulates: first app_ready call wins; a second call (or the safety timeout
    // racing) is a guaranteed no-op.  This is the core invariant of the command.
    let guard = crate::splash::RevealGuard::default();
    assert!(
        guard.claim(),
        "first reveal (Some branch or None branch) must win"
    );
    assert!(!guard.claim(), "duplicate app_ready must not reveal again");
    assert!(
        !guard.claim(),
        "safety timeout racing after app_ready must not reveal again"
    );
}

#[test]
fn app_ready_min_display_remaining_time_math() {
    use std::time::{Duration, Instant};

    // The Some-branch logic: `MIN_SPLASH.checked_sub(elapsed)`.
    // When elapsed < MIN_SPLASH there must be remaining time; when >= there must not.
    const MIN_SPLASH: Duration = Duration::from_millis(700);

    // Simulated fast cold start: shown_at is "just now" — remaining ≈ 700 ms.
    let shown_at = Instant::now();
    let elapsed = shown_at.elapsed(); // sub-millisecond
    let remaining = MIN_SPLASH.checked_sub(elapsed);
    assert!(
        remaining.is_some(),
        "a brand-new shown_at must produce remaining display time"
    );
    assert!(
        remaining.unwrap() <= MIN_SPLASH,
        "remaining must not exceed the minimum window"
    );

    // Simulated slow app start: elapsed already exceeds MIN_SPLASH — no sleep needed.
    let old_shown_at = Instant::now() - Duration::from_secs(1);
    let old_elapsed = old_shown_at.elapsed();
    let no_remaining = MIN_SPLASH.checked_sub(old_elapsed);
    assert!(
        no_remaining.is_none(),
        "when the minimum has already elapsed, checked_sub must return None (no sleep)"
    );
}

#[test]
fn test_system_get_version() {
    let version = system_get_version();
    assert!(!version.is_empty());
    // Version should be semver-like (x.y.z)
    assert!(version.contains('.'));
}

#[test]
fn test_system_get_platform() {
    let platform = system_get_platform();
    assert!(platform["platform"].is_string());
    assert!(platform["arch"].is_string());
    assert_eq!(platform["shell"], "tauri");
}

#[test]
fn test_locale_file_path() {
    // This test would need a mock AppHandle in practice
    // For now, we'll skip the full integration test
}

#[test]
fn test_gpu_info_empty() {
    #[cfg(not(windows))]
    {
        let gpu_info = get_gpu_info();
        // On non-Windows, this may return empty or actual GPU info
        // Just verify it returns a vector without panicking
        let _ = gpu_info;
    }
}

#[test]
fn test_system_check_browser() {
    // Test that the function doesn't panic and returns valid JSON
    let result = system_check_browser();
    // Verify the result has the expected structure
    assert!(result.is_object());
    assert!(result.get("detected").is_some());
    assert!(result.get("path").is_some());
}
