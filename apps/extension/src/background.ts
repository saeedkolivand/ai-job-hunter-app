/**
 * Background service worker / event page.
 *
 * Owns the single {@link BridgeClient} to the desktop loopback bridge and
 * answers the popup's `runtime.onMessage` requests. MV3 lifecycle: this context
 * can be evicted whenever idle, so all state is reconstructed lazily on wake
 * (`getClient()`), and we re-probe on `runtime.onStartup`, `onInstalled`, and
 * whenever the popup sends its first message.
 */

import { browser } from '@wxt-dev/browser';

import type { ExtensionImportRequest } from '@ajh/shared';

import { BridgeClient } from './lib/bridge';
import type { ConnectionStatus, PopupRequest, PopupResponse } from './lib/messages';
import { clearToken, getToken, looksLikeToken, setToken } from './lib/storage';

/** Lazily-built, worker-lifetime-scoped client. Recreated after eviction. */
let client: BridgeClient | null = null;

function getClient(): BridgeClient {
  if (!client) {
    client = new BridgeClient(
      () => {
        // Best-effort push so an open popup live-updates; ignore "no receiver".
        void broadcastStatus();
      },
      // Provide the stored token so the bridge can perform the auth handshake on connect.
      getToken
    );
  }
  return client;
}

/** Fold raw bridge phase + token presence into the popup-facing status. */
async function computeStatus(): Promise<ConnectionStatus> {
  const hasToken = (await getToken()) !== null;
  const bridge = getClient().status();

  let phase: ConnectionStatus['phase'];
  if (bridge.phase === 'bad_token') {
    phase = 'bad_token';
  } else if (bridge.phase === 'app_not_running') {
    phase = 'app_not_running';
  } else if (bridge.phase === 'searching') {
    phase = 'searching';
  } else if (!hasToken) {
    // Bridge reachable but we have no secret yet → show the pairing screen.
    phase = 'not_paired';
  } else {
    // bridge.phase === 'connected' AND hasToken, meaning the auth handshake succeeded.
    phase = 'connected';
  }
  return { phase, port: bridge.port, hasToken };
}

/** Push the current status to any listening popup (no-op if none is open). */
async function broadcastStatus(): Promise<void> {
  try {
    const status = await computeStatus();
    const message: PopupResponse = { ok: true, kind: 'status', status };
    await browser.runtime.sendMessage(message);
  } catch {
    // No popup open / port closed — fine.
  }
}

/** Resolve the active tab's URL for an import. */
async function activeTabUrl(): Promise<string> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
  const url = tab?.url ?? '';
  if (!url) throw new Error('Could not read the current tab URL.');
  return url;
}

/**
 * Scan mode: inject the capture script into the active tab and return its
 * `outerHTML`. Requires `scripting` + `activeTab` (granted on the click).
 */
async function captureActiveTabHtml(): Promise<string> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
  const tabId = tab?.id;
  if (typeof tabId !== 'number') throw new Error('No active tab to scan.');

  const results = await browser.scripting.executeScript({
    target: { tabId },
    files: ['content.js'],
  });
  const html = results[0]?.result;
  if (typeof html !== 'string' || html.length === 0) {
    throw new Error('Could not capture the page DOM.');
  }
  return html;
}

/** Run an import, always attempting to capture the rendered DOM first. */
async function runImport(applied: boolean): Promise<PopupResponse> {
  const token = await getToken();
  if (!token) {
    return { ok: false, error: 'Not paired. Paste your pairing token first.' };
  }

  const url = await activeTabUrl();
  const payload: ExtensionImportRequest = { url, applied };
  // Always try to capture the authenticated DOM so the desktop can parse it
  // without re-fetching (which would hit bot-walls on LinkedIn/Indeed/Glassdoor).
  // Fall back to URL-only if executeScript is blocked (restricted pages).
  try {
    payload.html = await captureActiveTabHtml();
  } catch {
    // ponytail: restricted page or scripting permission denied — URL-only fallback
  }

  const result = await getClient().importJob(token, payload);
  return { ok: true, kind: 'import', result };
}

/** Central popup-request dispatcher. Never throws — maps errors to `ok:false`. */
async function handleRequest(req: PopupRequest): Promise<PopupResponse> {
  try {
    switch (req.kind) {
      case 'getStatus': {
        // Opening the popup is a good moment to (re)probe the bridge.
        void getClient().ensureConnected();
        const status = await computeStatus();
        return { ok: true, kind: 'status', status };
      }
      case 'setToken': {
        if (!looksLikeToken(req.token)) {
          return {
            ok: false,
            error:
              'Invalid token format. Paste the full 64-character hex token from the desktop app.',
          };
        }
        await setToken(req.token);
        // Reset any bad-token block so the bridge will attempt auth with the new token.
        getClient().resetForNewToken();
        void getClient().ensureConnected();
        return { ok: true, kind: 'token' };
      }
      case 'clearToken': {
        await clearToken();
        // Also reset the bad-token block so the bridge returns to searching state.
        getClient().resetForNewToken();
        return { ok: true, kind: 'token' };
      }
      case 'reconnect': {
        await getClient().ensureConnected();
        return { ok: true, kind: 'status', status: await computeStatus() };
      }
      case 'import':
        return await runImport(req.applied);
      default: {
        // Exhaustiveness guard — a new PopupRequest variant must be handled.
        const _never: never = req;
        return { ok: false, error: `Unknown request: ${JSON.stringify(_never)}` };
      }
    }
  } catch (err) {
    return { ok: false, error: err instanceof Error ? err.message : String(err) };
  }
}

// ── wiring ────────────────────────────────────────────────────────────────────

browser.runtime.onMessage.addListener(
  (message: unknown): Promise<PopupResponse> => handleRequest(message as PopupRequest)
);

// Re-probe on the lifecycle wake points so a freshly-started worker reconnects.
browser.runtime.onStartup.addListener(() => {
  void getClient().ensureConnected();
});
browser.runtime.onInstalled.addListener(() => {
  void getClient().ensureConnected();
});

// Kick an initial probe when the worker first loads.
void getClient().ensureConnected();

// Apply a pending update immediately once the browser has already downloaded it,
// instead of waiting for the next natural SW restart.
// ponytail: onUpdateAvailable only fires when an update is already staged — we
// are not pulling the update, just collapsing the apply delay.
browser.runtime.onUpdateAvailable.addListener(() => {
  browser.runtime.reload();
});

// Chrome-only: nudge the browser to check for an update now so the download
// starts sooner. requestUpdateCheck is absent in Firefox, so feature-detect.
// ponytail: single startup nudge only — the browser already polls periodically.
if (typeof browser.runtime.requestUpdateCheck === 'function') {
  void browser.runtime.requestUpdateCheck().catch((err: unknown) => {
    // Non-fatal — update checks may be rate-limited or unavailable. Surface a
    // sanitized warning to the SW console (no telemetry leaves the device) so a
    // persistent updater regression stays observable instead of fully silent.
    console.warn('[ajh] update check failed:', err instanceof Error ? err.name : 'unknown');
  });
}

// Ensure this file is treated as an ES module (Chrome SW `type: module`).
export {};
