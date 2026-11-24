//! Scrape admission: queued slot acquisition for started jobs, plus the
//! rejecting variant for the synchronous resolve. Split out of `run.rs` for R8.

use tauri::{AppHandle, Manager};

use crate::error::AppResult;

/// Queued admission on the shared scrape budget: waits (firing `on_park` once,
/// before the wait) instead of rejecting when the concurrency cap is full. Takes
/// the limiter directly so it is testable without an `AppHandle`. Returns the
/// guard plus whether it parked.
pub(super) async fn acquire_scrape_queued(
    limiter: &std::sync::Arc<crate::limits::Limiter>,
    op: &'static str,
    on_park: impl FnOnce(usize),
) -> AppResult<(crate::limits::ConcurrencyGuard, bool)> {
    limiter
        .acquire_queued(
            op,
            crate::limits::SCRAPE_RATE_MAX,
            crate::limits::SCRAPE_CONCURRENCY_MAX,
            crate::limits::SCRAPE_QUEUE_MAX,
            on_park,
        )
        .await
}

/// [`acquire_scrape_queued`] for a started job: marks it `queued` while parked and
/// dequeues on grant. `cancel` (when the job has a token) abandons the wait;
/// `Ok(None)` means cancelled while parked.
pub(super) async fn wait_for_scrape_slot(
    app: &AppHandle,
    op: &'static str,
    job_id: &str,
    cancel: Option<&tokio_util::sync::CancellationToken>,
) -> AppResult<Option<crate::limits::ConcurrencyGuard>> {
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    let wait = acquire_scrape_queued(&limiter, op, |ahead| {
        crate::commands::jobs::job_queued(app, job_id, ahead);
    });
    let granted = match cancel {
        Some(token) => tokio::select! {
            biased;
            () = token.cancelled() => return Ok(None),
            r = wait => r?,
        },
        None => wait.await?,
    };
    let (guard, parked) = granted;
    if parked {
        crate::commands::jobs::job_dequeued(app, job_id);
    }
    Ok(Some(guard))
}

/// Take one slot on the shared scrape budget (rate + concurrency) under `op`,
/// rejecting when full. Only for the synchronous `scrape_resolve_url`; started
/// jobs use [`wait_for_scrape_slot`]. The returned guard releases on drop.
pub(super) fn acquire_scrape_slot(
    app: &AppHandle,
    op: &'static str,
) -> AppResult<crate::limits::ConcurrencyGuard> {
    let limiter = app
        .state::<std::sync::Arc<crate::limits::Limiter>>()
        .inner()
        .clone();
    limiter.acquire(
        op,
        crate::limits::SCRAPE_RATE_MAX,
        crate::limits::SCRAPE_CONCURRENCY_MAX,
    )
}
