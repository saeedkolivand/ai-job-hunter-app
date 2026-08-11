//! Central cancellation-token registry, keyed by job/run id.
//!
//! Pays off the `TODO(arch)` that sat in `commands::agent::agent_run`: agent
//! runs borrowed [`crate::scraping::ScraperEngine`]'s private job-token map
//! because that is where `jobs_cancel` happened to dispatch. That worked, but it
//! tied a domain-neutral concern (a user pressed Cancel on job `X`) to the
//! scraper. The registry is that concern, extracted — the engine now DELEGATES
//! to it and everyone else (agent runs today, pipeline runs next) registers
//! against the same shared instance, so one `jobs_cancel` still reaches every
//! job kind.
//!
//! **Ownership rule (unchanged, and the reason `cancel` does not remove):**
//! whoever registers a slot removes it. Cancelling leaves the slot IN PLACE so a
//! cancel that lands before the run reaches its worker is not lost — see
//! [`crate::jobs::cancel::CancelRegistry::cancel`] and
//! [`crate::jobs::cancel::CancelRegistry::get_or_register`].
//!
//! L1, Tauri-free: a `HashMap` behind a `tokio::sync::Mutex`, exactly the shape
//! (and the `.await` points) the engine held inline, so every existing cancel
//! path keeps its behavior byte-for-byte.

use std::collections::HashMap;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Live cancellation tokens, keyed by job/run id.
#[derive(Default)]
pub struct CancelRegistry {
    tokens: Mutex<HashMap<String, CancellationToken>>,
}

impl CancelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `token` under `id` so [`Self::cancel`] can reach it. Replaces any
    /// existing registration for the same id (last writer wins), matching the
    /// `HashMap::insert` semantics every caller already relies on.
    ///
    /// **Why last-writer-wins needs no generation/handle:** ids are MINTED per
    /// run by [`crate::db::new_job_id`] (`job-{uuid-v4}`) and never derived from
    /// anything stable like a job URL — `commands::scrape::scrape_boards`,
    /// `commands::autopilot::autopilot_run` and `commands::agent::agent_run` are
    /// the three registering call sites, each minting one id per invocation, and
    /// the engine only ever [`Self::get_or_register`]s the id it was passed. So
    /// two LIVE runs cannot share a key, and the "run A's late `unregister`
    /// removes run B's token" interleaving has no way to arise. The one id used
    /// by two owners — Autopilot pre-registering the token its own nested scrape
    /// then reuses — is one run, resolved by `get_or_register`'s `we_minted`
    /// flag rather than by identity.
    pub async fn register(&self, id: &str, token: CancellationToken) {
        self.tokens.lock().await.insert(id.to_string(), token);
    }

    /// Remove a registration. Idempotent; an unknown id is a no-op.
    ///
    /// This is the ONLY remover: every owner calls it on EVERY exit path
    /// (success, error, and cancel), which is what keeps the map from growing
    /// for the lifetime of the process.
    pub async fn unregister(&self, id: &str) {
        self.tokens.lock().await.remove(id);
    }

    /// Cancel the token registered under `id`, if any. Unknown id → no-op.
    ///
    /// Cancels the slot **in place** — it is deliberately NOT removed. Removing
    /// it loses any cancel that lands before the run reaches its worker: that
    /// worker mints a FRESH (un-cancelled) token when it finds no slot (see
    /// [`Self::get_or_register`]), so the run would proceed as if nothing had
    /// been cancelled. Callers do real async work in that window (a geocode
    /// backfill, a semaphore wait of several seconds), so it is not theoretical.
    ///
    /// Idempotent: [`CancellationToken::cancel`] on an already-cancelled token
    /// is a no-op, so a double-cancel (two clicks, or a cancel racing a
    /// caller-driven cancel) changes nothing.
    pub async fn cancel(&self, id: &str) {
        let tokens = self.tokens.lock().await;
        if let Some(token) = tokens.get(id) {
            token.cancel();
        }
    }

    /// The token for `id`, minting and registering a fresh one when there is
    /// none. Returns `(token, we_minted)` — `we_minted == false` means the slot
    /// was PRE-REGISTERED by someone else, who therefore still owns removing it.
    ///
    /// The lookup and the insert happen under ONE lock hold on purpose: as two
    /// calls this is check-then-act, and the interleaving loses a cancel exactly
    /// like the remove-on-cancel bug above.
    pub async fn get_or_register(&self, id: &str) -> (CancellationToken, bool) {
        let mut tokens = self.tokens.lock().await;
        match tokens.get(id) {
            Some(existing) => (existing.clone(), false),
            None => {
                let token = CancellationToken::new();
                tokens.insert(id.to_string(), token.clone());
                (token, true)
            }
        }
    }

    /// How many tokens are currently registered — the leak check (a finished run
    /// must leave nothing behind).
    pub async fn live_count(&self) -> usize {
        self.tokens.lock().await.len()
    }
}

#[cfg(test)]
mod test;
