//! Bridge server bootstrap — split from `mod.rs` (R8 relief): binds the loopback listener and
//! spawns [`super::connection::handle_connection`] per accepted socket. See `mod.rs`'s own
//! module doc for the full security model.

use std::net::Ipv4Addr;

use tauri::{AppHandle, Manager};
use tokio::net::TcpListener;

use super::connection::handle_connection;
use super::{BridgeState, PORT_RANGE};

/// Spawn the bridge server via the Tauri async runtime. Fire-and-forget: a
/// bind failure logs and leaves the bridge disabled (port stays `None`) — it
/// never panics or blocks boot. Call once from the Tauri `setup`, which runs on
/// the main thread with **no** ambient Tokio reactor in scope — so this routes
/// through [`spawn_detached`] ([`tauri::async_runtime::spawn`]), the house idiom
/// for spawning from a sync/no-runtime context (a bare `tokio::spawn` here
/// panics with "there is no reactor running").
pub fn start(app: AppHandle) {
    spawn_detached(async move {
        let Some(state) = app.try_state::<BridgeState>() else {
            log::warn!("[extension_bridge] BridgeState not managed — bridge disabled");
            return;
        };

        let listener = match bind_listener().await {
            Some((listener, port)) => {
                state.set_port(Some(port));
                log::info!("[extension_bridge] listening on 127.0.0.1:{port}");
                listener
            }
            None => {
                log::warn!(
                    "[extension_bridge] no free port in {}..={} — bridge disabled",
                    PORT_RANGE.start(),
                    PORT_RANGE.end()
                );
                return;
            }
        };

        loop {
            match listener.accept().await {
                Ok((stream, _peer)) => {
                    let conn_app = app.clone();
                    spawn_detached(async move {
                        handle_connection(conn_app, stream).await;
                    });
                }
                Err(e) => {
                    log::warn!("[extension_bridge] accept error (continuing): {e}");
                }
            }
        }
    });
}

/// Fire-and-forget spawn through the Tauri async runtime. Unlike a bare
/// `tokio::spawn`, this does **not** require an ambient Tokio reactor in the
/// caller's scope, so it is safe to call from the sync `setup` hook (which runs
/// on the main thread with no runtime). This is the house idiom shared with
/// `updater`/`tray`/`autopilot_scheduler`. Isolated as a one-line helper so the
/// no-runtime spawn path is exercisable from a plain `#[test]` (no ambient
/// runtime) — a regression to bare `tokio::spawn` would panic that test.
pub(super) fn spawn_detached<F>(fut: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    tauri::async_runtime::spawn(fut);
}

/// Try each port in [`PORT_RANGE`] in order; return the first that binds.
async fn bind_listener() -> Option<(TcpListener, u16)> {
    probe_ports(PORT_RANGE).await
}

/// Probe an explicit, ordered port range on loopback and return the first that
/// binds (or `None` if all are busy). Factored out of [`bind_listener`] so the
/// fallback/graceful-disable logic is testable against a caller-controlled range
/// of known-busy/known-free ports instead of the fixed [`PORT_RANGE`] (whose
/// availability on CI is non-deterministic). Behaviorally identical to the prior
/// inline loop — same order, same first-binds-wins, same `None` when exhausted.
async fn probe_ports(range: std::ops::RangeInclusive<u16>) -> Option<(TcpListener, u16)> {
    for port in range {
        let addr = (Ipv4Addr::LOCALHOST, port);
        if let Ok(listener) = TcpListener::bind(addr).await {
            return Some((listener, port));
        }
    }
    None
}

#[cfg(test)]
mod tests;
