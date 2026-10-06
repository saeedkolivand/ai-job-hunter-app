/**
 * Shared harness for the side-panel suites. `sidepanel.ts` runs at module load
 * (DOM lookups via `byId`, the `tabs.onActivated` / focus listeners, the first
 * `follow()`), so a suite builds the panel DOM and imports it once through
 * {@link bootPanel}; everything it mounts is a recording stub from
 * `test-mocks.ts`, registered by the suite via `vi.mock`.
 */

import { vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import { subscribeAnswerState } from '../../lib/answer-state';
import { PANEL_WINDOW_ID } from './test-mocks';

export { PANEL_WINDOW_ID };

/** Flush the module-load `resolvePanelWindowId().then(...)` chain. */
export const flush = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

/** The DOM `sidepanel.ts` queries via `byId` at module load — rebuilt fresh before
 *  every `vi.resetModules()` + reimport. */
export function buildPanelDom(): void {
  document.body.innerHTML =
    '<header><h1 class="title">AI Job Hunter</h1>' +
    '<div id="connection-pill-host"><button id="btn-settings"></button></div></header>' +
    '<section id="view-connected" hidden>' +
    '<p id="trust-line" hidden></p>' +
    '<div id="auto-save-notice" hidden><p id="auto-save-notice-text"></p>' +
    '<button id="auto-save-notice-dismiss"></button></div>' +
    '<div id="tabs-host"></div>' +
    '</section>' +
    '<div id="connection-views-host"></div>';
}

/**
 * Build the DOM and import `sidepanel.ts` once. Everything under test — the
 * `tabs.onActivated` listener, the `mountJobTools`/`mountConnectionStatus` calls
 * and the deps they were handed — is recorded on the mocks by that import,
 * before any test body runs. Vitest 5 turned `clearMocks` on by default (a
 * `vi.clearAllMocks()` before every test), which wipes exactly that history (the
 * migration guide names module-load recording as the most affected pattern:
 * https://vitest.dev/guide/migration#clearmocks-is-enabled-by-default), so this
 * opts the calling file out; the runner restores the config after the file, and
 * tests still clear per-test history explicitly where they depend on it.
 */
export async function bootPanel(): Promise<void> {
  buildPanelDom();
  vi.setConfig({ clearMocks: false });
  await import('../sidepanel');
}

/** Fire the panel's registered `tabs.onActivated` listener. */
export function activate(tabId: number, windowId = PANEL_WINDOW_ID): void {
  const onActivated = vi.mocked(browser.tabs.onActivated.addListener).mock.calls[0]?.[0];
  if (!onActivated) throw new Error('tabs.onActivated listener not registered');
  onActivated({ tabId, windowId } as never);
}

/** An `AnswerState` for `tabId` (trusted, empty) with `over` merged in. */
export const stateFor = (tabId: number, over: Record<string, unknown> = {}) => ({
  tabId,
  origin: 'https://jobs.example.com',
  scannedAt: 1,
  rows: [],
  stream: null,
  pageChanged: false,
  ...over,
});

/** Capture the NEXT `subscribeAnswerState` subscription's state callback, so a
 *  test controls exactly when (and whether) its first delivery lands. */
export function captureNextSubscription(): (state: unknown) => void {
  let deliver: ((state: unknown) => void) | undefined;
  vi.mocked(subscribeAnswerState).mockImplementationOnce((_tabId, onState) => {
    deliver = onState as (state: unknown) => void;
    return vi.fn();
  });
  return (state) => {
    if (!deliver) throw new Error('subscribeAnswerState callback not captured');
    deliver(state);
  };
}

/** Make the NEXT subscription deliver `state` on a later microtask, like the real one. */
export function deliverOnNextSubscription(state: unknown): void {
  vi.mocked(subscribeAnswerState).mockImplementationOnce((_tabId, onState) => {
    queueMicrotask(() => onState(state as never));
    return vi.fn();
  });
}
