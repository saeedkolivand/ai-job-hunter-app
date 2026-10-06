/**
 * The worker's one {@link BridgeClient}, the popup-facing connection status,
 * and the pushes to any open popup/side panel. MV3 can evict this context
 * whenever idle, so the client is rebuilt lazily on wake.
 */

import { browser } from '@wxt-dev/browser';

import { BridgeClient } from '../lib/bridge';
import type { ConnectionStatus, PopupResponse } from '../lib/messages';
import { clearToken, getToken } from '../lib/storage';

/** Lazily-built, worker-lifetime-scoped client. Recreated after eviction. */
let client: BridgeClient | null = null;

export function getClient(): BridgeClient {
  if (!client) {
    client = new BridgeClient(
      () => {
        // Best-effort push so an open popup live-updates; ignore "no receiver".
        void broadcastStatus();
      },
      // Provide the stored token so the bridge can perform the auth handshake on connect.
      getToken,
      // The desktop rotated its pairing secret (Settings → "Regenerate", or a
      // factory reset) and told us over the authenticated session. Un-pair
      // through the SAME path the popup's "Unpair" button uses, so a
      // desktop-initiated and a user-initiated un-pair can't drift.
      unpairLocally
    );
  }
  return client;
}

/**
 * Drop the stored pairing token and clear any auth block, leaving the bridge
 * ready to pair again. Shared by the popup's "Unpair" (`clearToken`) and the
 * desktop-initiated `token.revoked` frame.
 */
export async function unpairLocally(): Promise<void> {
  await clearToken();
  getClient().resetForNewToken();
}

/** The refusal every token-gated gesture answers before it touches the page. */
export function notPaired(): PopupResponse {
  return { ok: false, error: 'Not paired. Paste your pairing token first.' };
}

/** Fold raw bridge phase + token presence into the popup-facing status. */
export async function computeStatus(): Promise<ConnectionStatus> {
  const hasToken = (await getToken()) !== null;
  const bridge = getClient().status();

  let phase: ConnectionStatus['phase'];
  if (bridge.phase === 'bad_token') {
    phase = 'bad_token';
  } else if (bridge.phase === 'outdated') {
    // Desktop too old for the v2 handshake → prompt the user to update the app.
    phase = 'outdated';
  } else if (bridge.phase === 'app_not_running') {
    phase = 'app_not_running';
  } else if (bridge.phase === 'searching') {
    phase = 'searching';
  } else if (!hasToken) {
    // Bridge reachable but we have no secret yet → show the pairing screen.
    phase = 'not_paired';
  } else if (!bridge.authenticated) {
    // bridge.phase === 'connected' but this transport never actually ran the
    // v2 handshake — the no-token attach path also reaches 'connected', and a
    // freshly-pasted token briefly sits on that same unauthenticated transport
    // until `resetForNewToken()`'s forced reconnect completes (#1267). Never
    // report "Connected" before the desktop has verified us.
    phase = 'searching';
  } else {
    // bridge.phase === 'connected' AND hasToken AND bridge.authenticated → the
    // mutual handshake actually succeeded.
    phase = 'connected';
  }
  return { phase, port: bridge.port, hasToken };
}

/** Push one message to any listening popup/side panel. A closed port or no
 *  receiver is the normal case, never an error. */
export async function pushToSurfaces(
  build: () => PopupResponse | Promise<PopupResponse>
): Promise<void> {
  try {
    await browser.runtime.sendMessage(await build());
  } catch {
    // No popup/panel open / port closed — fine.
  }
}

function broadcastStatus(): Promise<void> {
  return pushToSurfaces(async () => ({ ok: true, kind: 'status', status: await computeStatus() }));
}

/** "A tracked application just flipped to applied" for the side panel. Sent by
 *  auto-track ONLY on a confirmed `saved → applied` write — see
 *  `PopupResponse`'s `jobStatusChanged` doc. */
export function broadcastJobStatusChanged(url: string): Promise<void> {
  return pushToSurfaces(() => ({ ok: true, kind: 'jobStatusChanged', url }));
}
