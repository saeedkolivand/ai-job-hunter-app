/**
 * Background service worker / event page — the entry. Owns the single
 * `BridgeClient` to the desktop loopback bridge and answers the popup's
 * `runtime.onMessage` requests; the handlers live in `./background/`.
 *
 * MV3 lifecycle: this context can be evicted whenever idle, so all state is
 * reconstructed lazily on wake (`getClient()`), and we re-probe on
 * `runtime.onStartup`, `onInstalled`, and whenever the popup sends its first
 * message. Every listener below MUST be registered synchronously at the top
 * level of this module, in this order — a listener registered after an `await`
 * misses the event that woke the worker.
 */

import { browser } from '@wxt-dev/browser';

import { getClient } from './background/bridge-client';
import { installContextMenu, onContextMenuClicked } from './background/context-menu';
import { onRuntimeMessage } from './background/message-listener';
import { clearAnswerState, updateAnswerState } from './lib/answer-state';

// Exported ONLY so `background/auto-track.test.ts` can pin this literal against
// the imported `lib/submit-watch.ts` const — a future edit to one side can't
// silently break routing.
export { SUBMIT_DETECTED_MSG } from './background/guards';

browser.runtime.onMessage.addListener(onRuntimeMessage);

if (browser.contextMenus) browser.contextMenus.onClicked.addListener(onContextMenuClicked);

/**
 * A navigation in a tab invalidates that tab's answer state for WRITING (the
 * `activeTab` grant may be gone and the scanned fields may be gone with it),
 * but not for READING — decision 3 keeps the rows and replaces the write
 * controls. Deliberately conservative: without the `tabs` permission the url is
 * not readable here, so a same-origin navigation flips the flag too. The cost
 * is one extra toolbar click; the alternative is a write control that silently
 * does nothing.
 */
browser.tabs.onUpdated.addListener((tabId, changeInfo) => {
  if (changeInfo.status !== 'loading') return;
  void updateAnswerState(tabId, (state) =>
    state.pageChanged ? null : { ...state, pageChanged: true }
  );
});

// The tab is gone, and so is anything its rows referred to.
browser.tabs.onRemoved.addListener((tabId) => {
  void clearAnswerState(tabId);
});

// Re-probe on the lifecycle wake points so a freshly-started worker reconnects.
browser.runtime.onStartup.addListener(() => {
  void getClient().ensureConnected();
  installContextMenu();
});
browser.runtime.onInstalled.addListener(() => {
  void getClient().ensureConnected();
  installContextMenu();
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
