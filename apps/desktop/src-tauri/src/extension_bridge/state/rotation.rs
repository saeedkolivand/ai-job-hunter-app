//! Pairing-token rotation + the revocation broadcast subscription — split from `state.rs`
//! (R8 relief; a second `impl BridgeState` block, legal since a private field stays visible
//! to the defining module's descendants, and `state::rotation` is one).

use std::sync::atomic::Ordering;

use super::super::persist::{new_token, persist_token};
use super::BridgeState;
use crate::observability::sanitize_reason;

impl BridgeState {
    /// Rotate the pairing token: REVOKE every live pairing, generate a new
    /// secret, persist it, and return it. The single rotation path — Settings →
    /// "Regenerate" ([`crate::commands::extension_bridge::extension_bridge_regenerate_token`])
    /// and the factory-reset hook ([`crate::data_store::Resettable`]) both come
    /// through here, so revocation can't apply to one and not the other.
    ///
    /// Order, all four steps under ONE hold of the token lock:
    /// 1. signal [`Self::subscribe_revoke`]'s receivers — each live connection
    ///    task sends `token.revoked` on its socket **if that socket is
    ///    authenticated** and then closes it (see `handle_connection`);
    /// 2. zero the live-connection count — no pairing survives a rotation, so
    ///    [`Self::is_connected`] must read false immediately rather than
    ///    waiting on every socket's teardown;
    /// 3. bump [`Self::rotation_epoch`], which is what stops a revoked socket's
    ///    LATE teardown from decrementing a count that a newly re-paired
    ///    browser now owns (see [`Self::dec_connected_for_epoch`]);
    /// 4. rotate + persist the secret.
    ///
    /// **TWO invariants are load-bearing here — a refactor must preserve BOTH:**
    /// - **The single lock hold around steps 1–4.** `revoke_tx.send()` must not
    ///   move outside it. The lock is what orders the revoke against the swap:
    ///   any reader that observes the NEW token is guaranteed to observe a
    ///   revoke that was ALREADY broadcast, so no socket can read a fresh token
    ///   and still be missed by the signal.
    /// - **Subscribe-at-accept** (in `handle_connection`), because the proof is
    ///   verified OUTSIDE this lock: a socket can still authenticate on a stale
    ///   token clone read just before step 1. Because its receiver already
    ///   existed, `broadcast::Sender::send` buffered the signal for it, so its
    ///   very next read-loop iteration resolves `Revoked` and tears it down.
    ///
    /// Drop either one and that window reopens. Any socket that re-runs the v2
    /// handshake afterwards with the old token fails the client-proof check and
    /// must re-pair with the new value.
    pub fn regenerate_token(&self) -> String {
        let fresh = new_token();
        {
            // One hold, four steps — see the doc above for why BOTH this lock
            // scope and the subscribe-at-accept in `handle_connection` are
            // load-bearing. The lock alone does NOT exclude a concurrent
            // handshake (`advance_auth` verifies outside it, via a `token()`
            // clone); it orders the revoke ahead of the swap. The buffered
            // broadcast is what catches a socket that authenticated on the
            // stale clone, and `dec_connected_for_epoch` is what stops that
            // socket's late teardown from stealing a newer pairing's count.
            let mut token = self.token.lock();
            let _ = self.revoke_tx.send(());
            self.connected.store(0, Ordering::Relaxed);
            self.rotation_epoch.fetch_add(1, Ordering::Relaxed);
            *token = fresh.clone();
        }
        if let Err(e) = persist_token(&self.data_dir, &fresh) {
            let reason = sanitize_reason(&e.to_string());
            log::warn!("[extension_bridge] failed to persist regenerated token: {reason}");
        }
        fresh
    }

    /// Subscribe to the pairing-revocation signal — one receiver per accepted
    /// connection, taken at accept time (BEFORE the handshake) so a rotation
    /// that races the handshake can never slip past a socket. Only sockets that
    /// exist at rotation time are signalled: a broadcast is an edge, never
    /// replayed state, so a connection opened after the rotation is not told
    /// anything (it is already handshaking against the new token).
    pub(in crate::extension_bridge) fn subscribe_revoke(
        &self,
    ) -> tokio::sync::broadcast::Receiver<()> {
        self.revoke_tx.subscribe()
    }
}

#[cfg(test)]
mod tests;
