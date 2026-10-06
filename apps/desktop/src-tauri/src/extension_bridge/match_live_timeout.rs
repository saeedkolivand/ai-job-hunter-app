//! Scoring-timeout primitives — split from `match_live.rs` (R8 relief): [`timed`] is the generic
//! bound-a-future helper both [`score_or_timeout`] (the interactive "Check fit" path) and
//! `match_live_import_score::score_import_posting_bounded` share; [`score_or_timeout`] additionally
//! collapses a genuine timeout and an unexpected scorer error onto the SAME fixed sentinel.

use serde_json::Value;

use crate::error::{AppError, AppResult};

/// Fixed sentinel — scoring failed with an unexpected internal shape OR
/// exceeded [`SCORE_TIMEOUT`]. One constant (not two call-site copies) so a
/// genuine hang and an internal failure are indistinguishable on the wire —
/// see [`score_or_timeout`].
const SCORE_FAILED_MESSAGE: &str = "Could not score this posting. Please retry.";

/// Wall-clock cap on the keyword-only scorer — shared by
/// [`score_import_posting`] (the import's own WS reply budget the extension
/// enforces client-side is ~30s, `lib/bridge/constants.ts`'s `REQUEST_TIMEOUT_MS`; scoring must
/// never eat meaningfully into that) AND [`score_or_timeout`] (the
/// interactive "Check fit" path — a hung/slow scorer must never block
/// `handle_connection`'s single-socket serial frame loop, which awaits each
/// verb synchronously, so a hang here would stall every subsequent frame on
/// that connection). Deliberately generous relative to the keyword-only
/// path's typical cost — a hard backstop, not a normal-path limit.
pub(super) const SCORE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Race `fut` against `cap`, returning tokio's raw timeout `Result` so a
/// caller can tell "genuinely timed out" (`Err`) apart from "finished in time
/// but yielded `None`" (`Ok(None)`) — [`score_import_posting_bounded`] logs
/// the two cases at different levels. Isolated as its own generic helper
/// because `score_import_posting` has no injectable delay seam (it reaches
/// into `AppHandle`/`DocumentStore` end to end), so THIS is the testable
/// boundary for the timeout mechanism itself (see the tests below).
pub(super) async fn timed<F, T>(
    cap: std::time::Duration,
    fut: F,
) -> Result<Option<T>, tokio::time::error::Elapsed>
where
    F: std::future::Future<Output = Option<T>>,
{
    tokio::time::timeout(cap, fut).await
}

/// Await `scoring` (a [`score_keyword_only`]-shaped future) bounded by
/// [`SCORE_TIMEOUT`] — the HIGH "bound interactive scoring" fix.
/// [`resolve_match_live`] used to await [`score_keyword_only`] unbounded, and
/// `handle_connection`'s frame loop awaits each verb synchronously, so a
/// hang/slowdown there would block every subsequent frame on that socket, not
/// just this request. Collapses BOTH a genuine timeout AND
/// `score_adhoc_keyword_only`'s only error branch ("job not found" —
/// unreachable here since `job_text` is always `Some`, but never let an
/// internal string reach the wire regardless of how it triggered) onto the
/// SAME fixed [`SCORE_FAILED_MESSAGE`] sentinel. Isolated as its own generic
/// helper (mirrors [`timed`]'s own isolation) because `resolve_match_live` has
/// no injectable delay seam (a real `AppHandle`/`DocumentStore` end to end) —
/// THIS is the testable boundary (see the tests below).
pub(super) async fn score_or_timeout<F>(scoring: F) -> AppResult<Value>
where
    F: std::future::Future<Output = Value>,
{
    match timed(SCORE_TIMEOUT, async { Some(scoring.await) }).await {
        Ok(Some(result)) if result.get("error").is_none() => Ok(result),
        Ok(Some(_)) => {
            log::warn!("[extension_bridge] match.live scoring returned an unexpected error shape");
            Err(AppError::Validation(SCORE_FAILED_MESSAGE.to_string()))
        }
        _ => {
            log::warn!(
                "[extension_bridge] match.live scoring exceeded {:?}; refusing with the fixed sentinel",
                SCORE_TIMEOUT
            );
            Err(AppError::Validation(SCORE_FAILED_MESSAGE.to_string()))
        }
    }
}

#[cfg(test)]
mod tests;
