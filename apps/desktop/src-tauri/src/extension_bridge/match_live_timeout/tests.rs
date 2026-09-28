use super::*;
use serde_json::json;

// ── timed (the import-reply-scoring timeout mechanism) ───────────────────
// `score_import_posting` has no injectable delay seam (it reaches into a
// real AppHandle/DocumentStore end to end), so these exercise the generic
// timeout wrapper directly — the HIGH "bound the import-reply scoring"
// fix's actual testable boundary, per the task's own fallback note.

#[tokio::test(start_paused = true)]
async fn timed_returns_err_when_the_future_exceeds_the_cap() {
    let cap = std::time::Duration::from_millis(100);
    let out = timed(cap, async {
        tokio::time::sleep(cap * 2).await;
        Some(42.0_f64)
    })
    .await;
    assert!(
        out.is_err(),
        "a future that exceeds the cap must yield the raw timeout Err, not the eventual value"
    );
}

#[tokio::test]
async fn timed_returns_the_value_when_within_the_cap() {
    let out = timed(std::time::Duration::from_millis(50), async {
        Some(7.0_f64)
    })
    .await;
    assert_eq!(
        out.unwrap(),
        Some(7.0),
        "a fast future must pass its value through unchanged"
    );
}

#[tokio::test]
async fn timed_distinguishes_a_fast_none_from_a_timeout() {
    // A None that resolves WELL within the cap must be `Ok(None)` — NOT
    // the `Err` a genuine timeout produces. This is exactly the
    // distinction `score_import_posting_bounded` logs at different levels
    // (a fast None must never be misreported as a timeout).
    let out: Result<Option<f64>, _> =
        timed(std::time::Duration::from_millis(50), async { None }).await;
    assert_eq!(out.unwrap(), None);
}

// ── score_or_timeout (the HIGH "bound interactive scoring" fix) ──────────
// `resolve_match_live` has no injectable delay seam either (real
// AppHandle/DocumentStore end to end) — same fallback boundary as `timed`
// above: exercise the wrapper directly with a synthetic scoring future.

#[tokio::test(start_paused = true)]
async fn score_or_timeout_yields_the_sentinel_on_a_genuine_hang() {
    let out = score_or_timeout(async {
        tokio::time::sleep(SCORE_TIMEOUT * 2).await;
        json!({ "combined": 99.0 })
    })
    .await;
    let err = out.expect_err("a hang must never block the resolve path — must refuse");
    assert_eq!(
        err.to_string(),
        SCORE_FAILED_MESSAGE,
        "a timeout must yield the fixed sentinel, not the eventual value nor a raw timeout error"
    );
}

#[tokio::test]
async fn score_or_timeout_passes_through_a_fast_result() {
    let out = score_or_timeout(async { json!({ "combined": 42.0, "ats": 30.0 }) }).await;
    let result = out.expect("a fast result within the cap must pass through");
    assert_eq!(result["combined"], 42.0);
}

#[tokio::test]
async fn score_or_timeout_refuses_an_unexpected_error_shape() {
    let out = score_or_timeout(async { json!({ "error": "job not found" }) }).await;
    assert_eq!(
        out.unwrap_err().to_string(),
        SCORE_FAILED_MESSAGE,
        "an internal error shape must never leak onto the wire — same fixed sentinel as a timeout"
    );
}
