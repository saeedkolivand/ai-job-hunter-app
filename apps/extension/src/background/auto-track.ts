/**
 * Auto-track (Task #22, Layer A): after a successful page-touching gesture, arm
 * the submit watcher on that page; when it reports a submit, mark the matched
 * application applied (and optionally save the answers).
 */

import { browser } from '@wxt-dev/browser';

import type { ExtensionAnswersSaveResult } from '@ajh/shared';

import type { CapturedAnswer } from '../lib/answers-capture';
import { setAutoSaveNotice } from '../lib/auto-save-notice';
import { handleSubmitDetected, maybeArmSubmitWatch } from '../lib/auto-track';
import { broadcastJobStatusChanged, getClient } from './bridge-client';
import { activeTabIn, injectAndRun } from './page';

/** Isolated-world global key under which `submit-watch.js` exposes its arm
 *  runner. MUST match `SUBMIT_WATCH_GLOBAL` in `lib/submit-watch.ts`. */
const SUBMIT_WATCH_GLOBAL = '__ajhArmSubmitWatch';

/**
 * Inject the submit watcher into the active tab and arm it with
 * `captureAnswers` — the desktop-enforced `saveAnswersOnSubmit` opt-in value,
 * resolved by `maybeArmSubmitWatch` BEFORE this call, passed as a plain
 * JSON-safe boolean. Called only after a successful gesture + only when the
 * auto-track opt-in is on; the watcher's own isolated-world flag makes a repeat
 * injection on the same page a no-op.
 */
async function injectSubmitWatch(captureAnswers: boolean, windowId?: number): Promise<void> {
  const tabId = (await activeTabIn(windowId))?.id;
  if (typeof tabId !== 'number') return;
  await injectAndRun(tabId, 'submit-watch.js', SUBMIT_WATCH_GLOBAL, [captureAnswers]);
}

/**
 * Nudge the user (action badge) that they submitted an application for a job the
 * app isn't tracking — clicking the action opens the popup, whose Import button
 * captures the page. Uses only the always-available `action` API (NO
 * `notifications` permission).
 */
function promptImport(): void {
  try {
    browser.action.setBadgeText({ text: '!' }).catch(() => {});
    browser.action.setBadgeBackgroundColor({ color: '#2563eb' }).catch(() => {});
  } catch {
    // action API unavailable — skip the nudge.
  }
}

/** Clear the untracked-submit nudge (called when the popup opens). */
export function clearImportPrompt(): void {
  try {
    browser.action.setBadgeText({ text: '' }).catch(() => {});
  } catch {
    // ignore — nothing to clear.
  }
}

/**
 * Read the desktop-enforced save-answers-on-submit opt-in — `true` only when
 * `settings.get` replies with `saveAnswersOnSubmit: true`. NEVER rejects: any
 * failure (not connected, a malformed reply) degrades to `false` (OFF, the safe
 * default). Rides `settings.get` rather than a dedicated wire verb — the switch
 * is reachable ONLY through `settings.get`/`settings.set`.
 */
async function saveAnswersOnSubmitEnabled(): Promise<boolean> {
  try {
    const res = await getClient().settingsGet();
    return res.ok && res.settings.saveAnswersOnSubmit === true;
  } catch {
    return false;
  }
}

/** Auto-track dependencies wired to the live bridge client. */
function submitFlowDeps() {
  return {
    autotrackEnabled: () => getClient().autotrackEnabled(),
    checkApplied: (url: string) => getClient().checkApplied(url),
    updateStatusAuto: (url: string) => getClient().updateStatus(url, true),
    promptImport,
    saveAnswersAuto: (url: string, answers: CapturedAnswer[]) =>
      getClient().saveAnswers(url, answers, true),
    notifyAutoSave: (result: Extract<ExtensionAnswersSaveResult, { ok: true }>) => {
      const count = result.saved;
      void setAutoSaveNotice(
        `Saved ${count} answer${count === 1 ? '' : 's'} from this submit${result.title ? ` (${result.title})` : ''} — change this in Settings → What the extension may do.`
      );
    },
    // The side panel's ONLY event-driven refresh: push the flipped application's
    // url so it re-reads just that job (never polls). Fires inside
    // `handleSubmitDetected` only on an `updateStatusAuto` ok:true.
    notifyJobStatusChanged: (url: string) => {
      void broadcastJobStatusChanged(url);
    },
  };
}

/** The injected watcher reported a form submit (fire-and-forget, no response). */
export function onSubmitDetected(url: string, answers?: CapturedAnswer[]): void {
  void handleSubmitDetected(url, submitFlowDeps(), answers);
}

/**
 * Arm the submit watcher on the page a gesture just touched (opt-in gated +
 * idempotent per page). Fire-and-forget: it never affects the popup's own
 * response. `windowId` rides the dep closure — never module state — because
 * every request is concurrent in a service worker and the ARM runs after the
 * request settles: it must target the tab the gesture happened on, not
 * whichever window the browser focused last (#1215).
 */
export function armSubmitWatch(windowId?: number): void {
  void maybeArmSubmitWatch({
    autotrackEnabled: () => getClient().autotrackEnabled(),
    injectSubmitWatch: (captureAnswers) => injectSubmitWatch(captureAnswers, windowId),
    saveAnswersOnSubmitEnabled,
  });
}
