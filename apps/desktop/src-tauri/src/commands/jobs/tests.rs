use super::*;

// A `reqwest`/keyring-shaped failure: a full URL carrying a query-string
// credential, plus a Windows path an unwound filesystem error tacked on.
// This is the PR #1036 leak class — sanitized in every `log::` call site,
// but `job_fail` reached the renderer raw through `jobs_get`/`jobs_list`
// and the `job.failed` event, a channel that sweep never touched.
const RAW_ERROR: &str = r"error sending request to https://api.example.com/v1?api_key=SECRET123: connection reset while reading C:\Users\alice\AppData\Local\ajh\cache";

#[test]
fn job_fail_sanitizes_before_it_reaches_the_tracker_or_the_event() {
    let mut tracker = JobTracker::default();
    tracker.start("job-1", "test.kind");

    let event_data = fail_in_tracker(&mut tracker, "job-1", RAW_ERROR.to_string(), None);

    // The tracker's own `error` field — what `jobs_get`/`jobs_list` return.
    let tracked = tracker.get("job-1").and_then(|r| r.error.as_deref());
    for leaked in ["SECRET123", "api.example.com", "alice"] {
        assert!(
            !tracked.unwrap_or_default().contains(leaked),
            "tracker.error must not carry {leaked:?}: {tracked:?}"
        );
    }

    // The `job.failed` event payload.
    let emitted = event_data.as_str().unwrap_or_default();
    for leaked in ["SECRET123", "api.example.com", "alice"] {
        assert!(
            !emitted.contains(leaked),
            "job.failed data must not carry {leaked:?}: {emitted}"
        );
    }

    // MUTATION GUARD: a no-op passthrough (`error` written/emitted raw)
    // would leave both destinations byte-identical to `RAW_ERROR` — this
    // only passes when sanitization actually ran.
    assert_ne!(tracked, Some(RAW_ERROR));
    assert_ne!(emitted, RAW_ERROR);
}

#[test]
fn job_fail_with_data_sanitizes_the_message_but_leaves_the_structured_data_alone() {
    let mut tracker = JobTracker::default();
    tracker.start("job-2", "test.kind");

    let structured = json!({ "kind": "timeout", "stage": "resume", "seconds": 45 });
    let event_data = fail_in_tracker(
        &mut tracker,
        "job-2",
        RAW_ERROR.to_string(),
        Some(structured.clone()),
    );

    let tracked = tracker.get("job-2").and_then(|r| r.error.as_deref());
    assert!(
        !tracked.unwrap_or_default().contains("SECRET123"),
        "tracker.error must not carry the credential: {tracked:?}"
    );
    // `data` rides as the event payload UNTOUCHED — it's a structured
    // payload with its own meaning, not free text.
    assert_eq!(event_data, structured);
}

#[test]
fn job_fail_leaves_an_already_friendly_message_unchanged() {
    // `resume_pipeline::hooks::timeout_message`'s shape: plain English
    // prose with no path/URL/credential tokens. Sanitizing at the
    // `job_fail` boundary must not mangle it.
    let mut tracker = JobTracker::default();
    tracker.start("job-3", "test.kind");
    let msg = "The \"resume\" step didn't get a response within 45s. Try a faster model or \
                    a lower effort level.";

    let event_data = fail_in_tracker(&mut tracker, "job-3", msg.to_string(), None);

    assert_eq!(
        tracker.get("job-3").and_then(|r| r.error.as_deref()),
        Some(msg)
    );
    assert_eq!(event_data.as_str(), Some(msg));
}
