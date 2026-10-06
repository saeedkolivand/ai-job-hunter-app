/**
 * Client to the desktop bridge, native-messaging first with a `ws` fallback.
 *
 * Two transports behind one {@link BridgeTransport} seam:
 *
 * 1. **Native messaging** (preferred). The browser spawns the desktop exe as a
 *    native host (`app.aijobhunter.bridge`) which relays stdio ↔ the running
 *    app's loopback bridge. Survives Firefox HTTPS-Only Mode (which upgrades the
 *    extension's `ws://127.0.0.1` to `wss://` and breaks the socket path).
 * 2. **WebSocket** (fallback). The desktop binds `127.0.0.1` on the first free
 *    port in the range below (see
 *    `apps/desktop/src-tauri/src/extension_bridge/mod.rs::PORT_RANGE`). Used when
 *    the native host isn't registered (old/never-installed app).
 *
 * Either way we hold a SINGLE transport. On connect we run the v2 mutual HMAC
 * handshake (`hello` → `challenge` → `auth{proof}` → `auth.ok{serverProof}`) —
 * the pairing token is used only as an HMAC key and is NEVER put on the wire —
 * then send token-free `import.request` / `profile.get` envelopes over the
 * now-authenticated socket, correlating replies by `reqId`.
 *
 * Lifecycle note (MV3): the background service worker can be evicted at any
 * time, tearing down this client. The background entry re-creates it on wake
 * and when the popup opens, so this class assumes it may be short-lived and
 * keeps no cross-eviction state beyond the in-flight `reqId` map (which dies
 * with the worker — callers re-issue on the fresh instance). An OPEN native-
 * messaging port (or ws socket) is itself a sanctioned reason the SW may
 * outlive Chrome's normal idle-eviction timer — this now covers a whole
 * streaming `answer.assist` exchange, not just a quick import/profile
 * round-trip, so a multi-second draft has the same lifecycle guarantee an
 * import always had. If the worker IS evicted mid-stream anyway (a hard
 * kill, not just idle eviction), `background.ts`'s `assistBuffer` is lost
 * with it — the popup's reattach query then simply finds nothing, same as
 * a session that never streamed.
 */

// The implementation lives in `./bridge/` (one module per concern):
// connection (lifecycle + handshake), client (verbs), requests/results-* (reply
// plumbing + guards), transport. This file stays the module path importers use.
export { BridgeClient } from './bridge/client';
export type { BridgePhase, BridgeStatus } from './bridge/connection';
