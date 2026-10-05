use super::*;

// ── notify_title reflects the widened candidacy ──────────────────────────
//
// MEDIUM fix: the title used to be hardcoded "confirmation" from when the
// matcher only ever considered `Saved` applications; candidacy widened to
// every `is_actionable` status, so a rejection reaching an `Interviewing`
// application must not be announced as a confirmation.

#[test]
fn notify_title_names_the_actual_intent_not_always_confirmation() {
    assert_eq!(
        notify_title(Some(EmailIntent::Confirmation)),
        "Possible application confirmation"
    );
    assert_eq!(
        notify_title(Some(EmailIntent::Rejection)),
        "Possible rejection notice"
    );
    assert_eq!(
        notify_title(Some(EmailIntent::Interview)),
        "Possible interview invite"
    );
    assert_eq!(
        notify_title(Some(EmailIntent::Offer)),
        "Possible offer notice"
    );
    // Distinct from every named-intent title — must never collapse to
    // "confirmation" just because the classifier came back empty.
    let unclassified = notify_title(None);
    assert_ne!(unclassified, notify_title(Some(EmailIntent::Confirmation)));
    assert_ne!(unclassified, notify_title(Some(EmailIntent::Rejection)));
    assert_ne!(unclassified, notify_title(Some(EmailIntent::Interview)));
    assert_ne!(unclassified, notify_title(Some(EmailIntent::Offer)));
}

// ── RATE_LIMITED_MESSAGE ↔ renderer sentinel parity ─────────────────────
//
// `RATE_LIMITED_MESSAGE` and the renderer's `CHECK_NOW_RATE_LIMIT_MESSAGE`
// (`EmailWatchSection/index.tsx`) are independent literals linked only by
// a comment on each side — an `AppError` serializes as plain text over
// IPC, so the renderer discriminates the friendly-copy case by an EXACT
// string match. Editing either alone would pass every test while
// silently breaking that match. Mirrors `extension_bridge::msg::tests::
// message_type_constants_match_ts`'s TS-source-as-text parity approach.

/// Path from this crate's manifest dir (`apps/desktop/src-tauri`) to the
/// renderer file hard-coding the same sentinel text.
const RATE_LIMIT_RENDERER_SOURCE: &str =
    "../src/renderer/features/settings/components/accounts/EmailWatchSection/index.tsx";

#[test]
fn rate_limited_message_matches_the_renderer_sentinel() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(RATE_LIMIT_RENDERER_SOURCE);
    let ts = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    let needle = format!("CHECK_NOW_RATE_LIMIT_MESSAGE = '{RATE_LIMITED_MESSAGE}';");
    assert!(
        ts.contains(&needle),
        "Rust RATE_LIMITED_MESSAGE ({RATE_LIMITED_MESSAGE:?}) not found as `{needle}` \
             in EmailWatchSection/index.tsx's CHECK_NOW_RATE_LIMIT_MESSAGE — the two \
             sentinels are independent literals and must be edited together"
    );
}

// ── should_commit_outcomes (/review HIGH: untested disconnect-mid-tick guard) ──

#[test]
fn should_commit_outcomes_is_false_once_the_account_is_gone() {
    assert!(
        !should_commit_outcomes(false),
        "a vanished account (Disconnect/factory-reset landing mid-tick) must skip \
             BOTH the post-tick writes and notify_match — this is the exact condition \
             `run_check_inner` branches on before either"
    );
}

#[test]
fn should_commit_outcomes_is_true_while_still_connected() {
    assert!(should_commit_outcomes(true));
}

#[test]
fn backoff_doubles_per_failure_and_caps_at_max_backoff() {
    assert_eq!(backoff_interval(0), BASE_CHECK_INTERVAL);
    assert_eq!(backoff_interval(1), Duration::from_secs(30 * 60));
    assert_eq!(backoff_interval(2), Duration::from_secs(60 * 60));
    assert_eq!(backoff_interval(3), Duration::from_secs(2 * 60 * 60));
    // 4 doublings would be 4h > the 2h cap.
    assert_eq!(backoff_interval(4), MAX_BACKOFF);
    // Never grows past the cap no matter how many consecutive failures.
    assert_eq!(backoff_interval(50), MAX_BACKOFF);
}

#[test]
fn never_checked_is_always_due() {
    assert!(is_due(None, 0, now_ms()));
    assert!(is_due(None, 5, now_ms()));
}

#[test]
fn not_due_before_the_base_interval_elapses() {
    let now = 1_000_000_000_000u64;
    let last = now - Duration::from_secs(5 * 60).as_millis() as u64; // 5 min ago
    assert!(!is_due(Some(last), 0, now));
}

#[test]
fn due_once_the_base_interval_has_elapsed_with_no_failures() {
    let now = 1_000_000_000_000u64;
    let last = now - BASE_CHECK_INTERVAL.as_millis() as u64;
    assert!(is_due(Some(last), 0, now));
}

#[test]
fn backoff_delays_due_ness_past_the_base_interval_after_failures() {
    let now = 1_000_000_000_000u64;
    // 20 minutes since the last attempt: past the 15 min base interval,
    // but well under the 30 min (1-failure) backoff interval.
    let last = now - Duration::from_secs(20 * 60).as_millis() as u64;
    assert!(
        is_due(Some(last), 0, now),
        "no failures — base interval alone gates it"
    );
    assert!(
        !is_due(Some(last), 1, now),
        "one failure — 30 min backoff not yet elapsed"
    );
}

// ── classify_tick_outcome (/review LOW) ─────────────────────────────────

#[test]
fn classify_tick_outcome_success_resets_and_rate_limited_is_distinct_from_failure() {
    let ok: AppResult<EmailWatchStatus> = Ok(EmailWatchStatus::default());
    assert_eq!(classify_tick_outcome(&ok), TickOutcome::Success);

    let rate_limited: AppResult<EmailWatchStatus> = Err(AppError::RateLimited(
        "a check already ran recently".to_string(),
    ));
    assert_eq!(
        classify_tick_outcome(&rate_limited),
        TickOutcome::RateLimited,
        "a concurrent-run refusal must not classify as a real failure"
    );

    let real_failure: AppResult<EmailWatchStatus> = Err(AppError::Network(
        "could not connect to the mail server".to_string(),
    ));
    assert_eq!(classify_tick_outcome(&real_failure), TickOutcome::Failure);
}

// ── concurrent-run guard (rust-backend-architect HIGH) ─────────────────
// `RUN_IN_FLIGHT` is a single process-global flag (not per-id, unlike
// autopilot's), so these two tests share state and MUST run serially
// relative to each other or they'd flake against the parallel test runner.

#[test]
#[serial_test::serial]
fn run_guard_blocks_a_second_concurrent_acquire() {
    let first = RunGuard::try_acquire().expect("first acquire succeeds");
    assert!(
        RunGuard::try_acquire().is_none(),
        "a second acquire while one is in flight is blocked (no concurrent runs)"
    );
    drop(first);
    assert!(
        RunGuard::try_acquire().is_some(),
        "after the first guard drops, a new acquire succeeds"
    );
}

#[test]
#[serial_test::serial]
fn run_guard_releases_on_drop_even_after_repeated_acquire_attempts() {
    let guard = RunGuard::try_acquire().expect("acquire succeeds");
    // Several refused attempts while held must not corrupt the flag —
    // each is a no-op read, not a competing claim.
    for _ in 0..3 {
        assert!(RunGuard::try_acquire().is_none());
    }
    drop(guard);
    assert!(RunGuard::try_acquire().is_some());
}

// ── apply_matched_intent reachability (v2 slice 3) ──────────────────────
//
// Mirrors `rate_limited_message_matches_the_renderer_sentinel`'s own
// technique above: read this file's OWN source as text and assert a
// substring, since `apply_matched_intent`'s runtime call site cannot be
// exercised from a unit test (it is buried inside `run_check_inner`,
// which needs a real IMAP round trip — see this module's "No automated
// test for the network-round-trip functions" precedent in
// `email_watch::imap_client`).

#[test]
fn apply_matched_intent_has_a_non_test_caller() {
    // The whole point of this slice: `email_watch::auto_write::
    // apply_matched_intent` must be called from PRODUCTION code, not
    // only from its OWN unit tests (`auto_write::tests`) — a test-only
    // caller would mean the infrastructure exists but never actually
    // runs, exactly the gap v2 slice 2 shipped and this slice closes.
    let source = include_str!("../email_watch_scheduler.rs");
    let production = source
        .split("#[cfg(test)]")
        .next()
        .expect("this file always has content before its first #[cfg(test)]");
    assert!(
        production.contains("apply_matched_intent("),
        "apply_matched_intent must be called from this file's production \
             code — the auto-write path exists to actually run, not just to \
             be unit-tested"
    );
}

/// HIGH fix: an `Ok(true)` write from `apply_matched_intent` (a real,
/// committed status transition) must emit `APPLICATIONS_CHANGED` — it is
/// the ONLY thing that makes the write visible on an already-open
/// `/applications/$id` page (`useApplicationEvents`, mounted once in
/// `routes/__root.tsx`, is the sole consumer, and those queries have
/// `refetchOnWindowFocus`/`refetchOnReconnect` both off with no
/// `refetchInterval`). Every OTHER backend-initiated application write
/// (`extension_bridge::import_flow`, `extension_bridge::status_update`)
/// already emits it; this call site was the exception. Scoped to the
/// literal `Ok(true) =>` / `Ok(false) =>` arms (not "does this string
/// appear anywhere in the file") so removing the emit from THIS call
/// site fails the test even though `APPLICATIONS_CHANGED` legitimately
/// appears elsewhere for the other emitters — and so an emit that moved
/// into the wrong arm (firing on a gated no-op) fails it too.
#[test]
fn a_successful_auto_write_emits_applications_changed_but_a_gated_noop_does_not() {
    let source = include_str!("../email_watch_scheduler.rs");
    let call_site = source
        .split("crate::email_watch::auto_write::apply_matched_intent(")
        .nth(1)
        .expect("apply_matched_intent is still called from this file");
    let ok_true_arm = call_site
        .split("Ok(true) =>")
        .nth(1)
        .expect("the match on apply_matched_intent's result still has an Ok(true) arm")
        .split("Ok(false) =>")
        .next()
        .expect("Ok(true) is still followed by an Ok(false) arm");
    assert!(
        ok_true_arm.contains("emit_event") && ok_true_arm.contains("APPLICATIONS_CHANGED"),
        "a successful (Ok(true)) apply_matched_intent write must emit \
             APPLICATIONS_CHANGED so an open application page refreshes without \
             requiring navigation away and back"
    );
    let ok_false_arm = call_site
        .split("Ok(false) =>")
        .nth(1)
        .expect("the match on apply_matched_intent's result still has an Ok(false) arm")
        .split("Err(e) =>")
        .next()
        .expect("Ok(false) is still followed by an Err arm");
    assert!(
        !ok_false_arm.contains("emit_event"),
        "a gated no-op (Ok(false) — opt-in off, unauthorized, no intent, no \
             valid transition, rejected target, or a lost CAS race) must stay \
             silent; emitting for a write that never happened is its own, \
             milder lie to the UI"
    );
}
