/** The worker's `runtime.onMessage` listener. */

import { type Browser, browser } from '@wxt-dev/browser';

import type { PopupRequest, PopupResponse } from '../lib/messages';
import { onSubmitDetected } from './auto-track';
import { openAnswerPanel } from './context-menu';
import { handleRequest } from './dispatch';
import { isOpenPanelFromBadge, isSubmitDetected } from './guards';

/** True only for a sender that is one of THIS extension's own pages. */
function isExtensionPage(sender: Browser.runtime.MessageSender): boolean {
  return (
    sender.id === browser.runtime.id &&
    typeof sender.url === 'string' &&
    sender.url.startsWith(browser.runtime.getURL(''))
  );
}

export function onRuntimeMessage(
  message: unknown,
  sender: Browser.runtime.MessageSender,
  sendResponse: (response?: PopupResponse) => void
): true | undefined {
  // The injected submit-watcher posts a fire-and-forget `submitDetected` — it
  // is NOT a popup request and expects no response, so handle it out-of-band.
  if (isSubmitDetected(message)) {
    // Belt-and-braces MV3 hygiene: this extension declares no
    // `externally_connectable`, so no other extension/page can ever reach this
    // listener — but require the sender to be THIS extension anyway before
    // acting on it (defense-in-depth, costs nothing).
    if (sender.id === browser.runtime.id)
      onSubmitDetected(message.url, message.answers, sender.tab?.id);
    return undefined;
  }
  // The injected fit badge's "Open the panel" button — also fire-and-forget,
  // same sender-check discipline as `isSubmitDetected` above.
  if (isOpenPanelFromBadge(message)) {
    if (sender.id === browser.runtime.id && typeof sender.tab?.id === 'number') {
      openAnswerPanel(sender.tab.id);
    }
    return undefined;
  }
  // Every remaining message is a PopupRequest (token, import, settings, ...).
  // Only extension PAGES (popup, side panel, options) may send one: a content
  // script runs inside a hostile page's process and must never reach these. A
  // content script's `sender.url` is the PAGE url, an extension page's starts
  // with this extension's own origin (chrome-extension:// or moz-extension://).
  // Deliberately NOT keyed on `sender.tab` — an options page opened in a tab has
  // one. Ignored without a response (the sender sees "no response").
  if (!isExtensionPage(sender)) return undefined;
  // Reply via `sendResponse` + a LITERAL `true`, never by returning a Promise.
  // `@wxt-dev/browser` is a thin `browser ?? chrome` pass-through (not
  // `webextension-polyfill`), and Chromium's `chrome.runtime.onMessage` has
  // never supported a Promise return value: it keeps the channel open only for
  // a literal `true`. A returned Promise is truthy but not `true`, so the
  // channel closed immediately, `sendMessage` resolved `undefined`, and every
  // request/response action came back as "No response from the extension
  // background." — while the one-way background→popup pushes kept working, so
  // the pill could still read "Connected". `sendResponse` + `return true` is the
  // shape BOTH engines accept.
  void handleRequest(message as PopupRequest).then(sendResponse, (err: unknown) => {
    // A rejection here would otherwise leave the port open until it times out,
    // which the popup surfaces as the same "No response" message.
    sendResponse({ ok: false, error: err instanceof Error ? err.message : String(err) });
  });
  return true;
}
