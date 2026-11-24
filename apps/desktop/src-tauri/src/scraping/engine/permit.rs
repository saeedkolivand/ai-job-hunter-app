//! The engine-permit wait, made observable: a caller that has to WAIT for a
//! permit (concurrency limit reached) is told so before it blocks, and again
//! when it gets the permit — so the job can show `queued`, not `running`.
//! `scraping/` stays free of `commands::*`: the caller injects a [`QueueHook`].

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, TryAcquireError};
use tokio_util::sync::CancellationToken;

use super::ScraperEngine;

/// What the permit wait reports to its caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueEvent {
    /// No permit was free; the call is about to wait behind `ahead` others.
    Parked { ahead: usize },
    /// A parked call got its permit. Never fired for a call that never parked,
    /// nor for one cancelled while waiting.
    Granted,
}

pub type QueueHook = Arc<dyn Fn(QueueEvent) + Send + Sync>;

/// Counts a waiter for exactly as long as it is parked; dropping (including a
/// dropped/cancelled future) decrements.
struct WaiterGuard<'a>(&'a AtomicUsize);

impl<'a> WaiterGuard<'a> {
    /// Registers a waiter and returns how many were already waiting.
    fn enter(counter: &'a AtomicUsize) -> (Self, usize) {
        let ahead = counter.fetch_add(1, Ordering::SeqCst);
        (Self(counter), ahead)
    }
}

impl Drop for WaiterGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl ScraperEngine {
    /// One engine permit for a whole multi-board batch. Free permit → returned
    /// at once, hook untouched. Otherwise the hook gets `Parked` BEFORE the
    /// await and `Granted` after; a cancel while parked ends the wait with `Err`.
    pub(super) async fn acquire_permit(
        &self,
        cancel: &CancellationToken,
        hook: Option<&QueueHook>,
    ) -> anyhow::Result<OwnedSemaphorePermit> {
        let sem = self.semaphore.load_full();
        match sem.clone().try_acquire_owned() {
            Ok(permit) => return Ok(permit),
            Err(TryAcquireError::Closed) => {
                return Err(anyhow::anyhow!("scraper engine semaphore closed"));
            }
            Err(TryAcquireError::NoPermits) => {}
        }

        let (_waiting, ahead) = WaiterGuard::enter(&self.waiting);
        if let Some(h) = hook {
            h(QueueEvent::Parked { ahead });
        }
        let permit = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(anyhow::anyhow!("scrape cancelled")),
            p = sem.acquire_owned() => {
                p.map_err(|_| anyhow::anyhow!("scraper engine semaphore closed"))?
            }
        };
        if let Some(h) = hook {
            h(QueueEvent::Granted);
        }
        Ok(permit)
    }

    /// Callers currently parked on the permit wait.
    #[cfg(test)]
    pub(super) fn waiting_count(&self) -> usize {
        self.waiting.load(Ordering::SeqCst)
    }
}
