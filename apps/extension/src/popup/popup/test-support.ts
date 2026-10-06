/**
 * Shared harness for the popup suites. `popup.ts` runs its wiring at module
 * load (DOM queries via `byId`, `wire()`, the status listener), so each suite
 * builds the popup DOM and imports the module once through {@link bootPopup}.
 *
 * Each suite declares, BEFORE importing this file's `bootPopup` result:
 *   vi.mock('@wxt-dev/browser', async () => (await import('./browser-mock')).popupBrowserMock());
 *   vi.mock('../../lib/storage', () => ({ looksLikeToken: vi.fn(() => false) }));
 * (`looksLikeToken` is mocked because `connection-status.ts`, mounted for real,
 * imports it.) The pill/retry/pairing behavior has its own suites under
 * `connection-status/`; this folder covers popup.ts's contract with it.
 */

import { vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import type { ConnectionStatus } from '../../lib/messages';

// Built before `popup.ts` is imported so jsdom has the elements its module-level
// `els` constant looks up.
const POPUP_DOM = `
  <div id="view-import" hidden></div>
  <!-- connection-status.ts mounts the pill/retry (with matching ids) into
       this host, and the four non-connected views into
       #connection-views-host, at module load. -->
  <div id="connection-pill-host"></div>
  <div id="connection-views-host"></div>
  <div id="job-card" hidden>
    <p id="job-card-title" hidden></p>
    <span id="applied-status" hidden></span>
    <button id="btn-mark-applied" hidden></button>
  </div>
  <!-- job-tools mounts its own Import/Check-fit/Fill DOM (with matching ids)
       into this host at module load, with hideSaveAnswers. -->
  <div id="job-tools-host"></div>
  <button id="btn-open-panel">Open the panel →</button>
  <p id="answers-notice" hidden></p>
  <div id="auto-save-notice" hidden>
    <p id="auto-save-notice-text"></p>
    <button id="auto-save-notice-dismiss"></button>
  </div>
  <p id="import-msg"></p>
  <div id="unpair-group" hidden>
    <button id="btn-unpair"></button>
  </div>
  <button id="btn-help" aria-expanded="false"></button>
  <div id="menu" hidden>
    <button id="menu-help">Help center</button>
    <button id="menu-settings">Settings</button>
    <button id="menu-about">About</button>
  </div>
  <p id="help-popover" hidden></p>
  <div id="about-popover" hidden>
    <p id="about-version"></p>
  </div>
`;

/** Build the popup DOM, then import `popup.ts` so its module-load wiring runs. */
export async function bootPopup() {
  document.body.innerHTML = POPUP_DOM;
  return import('../popup');
}

export const byId = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

export const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

export type Phase = ConnectionStatus['phase'];

/** Deliver a live status push through the popup's real connection-status listener
 *  (the FIRST `onMessage` listener registered — `connectionStatus.start()` runs
 *  before `wire()`). Call while the suite is being collected, before any test
 *  clears the mock history. */
export function statusPusher(): (phase: Phase, hasToken?: boolean) => void {
  const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
    ((message: unknown) => void) | undefined;
  if (!listener) throw new Error('onMessage status listener not registered');
  return (phase, hasToken = true) =>
    listener({ ok: true, kind: 'status', status: { phase, port: null, hasToken } });
}
