/**
 * Active-tab resolution and script injection — the seams every gesture handler
 * goes through to touch a page.
 */

import { type Browser, browser } from '@wxt-dev/browser';

/**
 * The active tab of the window a request came from — the single seam every
 * "act on the current tab" lookup in this worker goes through.
 *
 * A service worker has NO window of its own, so `currentWindow: true` here
 * resolves to whichever window the browser focused last, not the window whose
 * popup or side panel sent the request. With a second window focused that is a
 * different tab entirely: a read failed with a confusing error, and Import
 * silently created an application from an unrelated page while reporting
 * success (#1215). Surfaces therefore send their own `windowId`
 * (`PopupRequest`), and this targets it.
 *
 * `undefined` keeps the old last-focused-window behaviour, which is the only
 * thing available when no window is known (an older surface build, or a flow
 * with no originating window at all). Anything acting on a resolved tab id
 * afterwards must keep using THAT id rather than re-querying.
 */
export async function activeTabIn(windowId?: number): Promise<Browser.tabs.Tab | undefined> {
  const query =
    typeof windowId === 'number'
      ? { active: true, windowId }
      : { active: true, currentWindow: true };
  const [tab] = await browser.tabs.query(query);
  return tab;
}

/** Resolve the active tab's URL. */
export async function activeTabUrl(windowId?: number): Promise<string> {
  const url = (await activeTabIn(windowId))?.url ?? '';
  if (!url) throw new Error('Could not read the current tab URL.');
  return url;
}

/** The active tab's id, or throw `message`. Available WITHOUT the `tabs`
 *  permission (only a tab's url/title are gated behind it), which is what lets
 *  the answer state be keyed per tab while `tabs` stays on the denylist. */
export async function requireTabId(windowId: number | undefined, message: string): Promise<number> {
  const tabId = (await activeTabIn(windowId))?.id;
  if (typeof tabId !== 'number') throw new Error(message);
  return tabId;
}

export const activeTabId = (windowId?: number): Promise<number> =>
  requireTabId(windowId, 'No active tab.');

/**
 * The active tab's ORIGIN, read at GESTURE TIME (ADR-044 decision 1 and the
 * design log's amendment 10d). This only works because the click that got us
 * here just granted `activeTab` for this tab, which is what makes its url
 * readable — it is NOT a `tabs`-permission lookup. Degrades to `''` rather
 * than throwing: an unreadable origin costs the state its "same page?" check,
 * never the scan.
 */
export async function activeTabOriginAtGesture(windowId?: number): Promise<string> {
  try {
    return new URL(await activeTabUrl(windowId)).origin;
  } catch {
    return '';
  }
}

/**
 * Re-verify, right before injection, that `tabId` is still the active tab AND
 * still on `origin` (PR review round 2). `runDocumentAttach` captures both
 * BEFORE the desktop export round trip, which can take long enough for the user
 * to switch tabs or navigate away — without this check the résumé would attach
 * to whatever page happens to be active once the export resolves, not the one
 * the user confirmed.
 */
export async function tabStillConfirmed(
  tabId: number,
  origin: string,
  windowId?: number
): Promise<boolean> {
  const tab = await activeTabIn(windowId);
  if (tab?.id !== tabId || !tab.url) return false;
  try {
    return new URL(tab.url).origin === origin;
  } catch {
    return false;
  }
}

/** Same defect class as {@link tabStillConfirmed}, at EXACT-url granularity:
 *  a tab switch, or a same-tab navigation to another posting on the same
 *  origin, must not paint one page's score onto another (PR review finding). */
export async function tabStillOnExactUrl(
  tabId: number,
  url: string,
  windowId?: number
): Promise<boolean> {
  const tab = await activeTabIn(windowId);
  return tab?.id === tabId && tab.url === url;
}

/**
 * A tab the extension can NEVER read, so "reload the job page" would be a lie.
 * SHARED by the match-live ("Check fit") and stamp-results gestures, so the
 * wording is gesture-neutral (#1219). Chrome redacts the url of restricted tabs
 * (chrome://, about:*, the built-in PDF viewer, the Web Store) to an EMPTY
 * string unless the extension holds `tabs` permission — which it deliberately
 * does not (least privilege; it relies on `activeTab`). So both an empty url
 * AND a readable-but-restricted one fold into this message.
 */
export const UNREADABLE_PAGE_MSG =
  "This page can't be read by the extension — there's nothing to work with here.";
/** Transient capture failure on a NORMAL url — the reload hint is truthful here. */
export const CAPTURE_FAILED_MSG = 'Could not read this page. Reload it and try again.';

/**
 * Is `url` a permanently-unreadable page kind (see {@link UNREADABLE_PAGE_MSG})?
 * Lowercased so scheme/host matching is case-insensitive without a `URL` parse
 * (some restricted urls — `about:blank` — parse fine, but a raw prefix check
 * keeps this robust for every scheme the browser hands out).
 */
function isPermanentlyUnreadablePage(rawUrl: string): boolean {
  const trimmed = rawUrl.trim().toLowerCase();
  if (
    trimmed.startsWith('chrome://') ||
    trimmed.startsWith('about:') ||
    trimmed.startsWith('chrome-extension://') ||
    trimmed.startsWith('moz-extension://') ||
    trimmed.startsWith('resource://pdf.js/')
  ) {
    return true;
  }
  try {
    const parsed = new URL(trimmed);
    if (parsed.hostname === 'chromewebstore.google.com') return true;
    if (parsed.pathname.toLowerCase().endsWith('.pdf')) return true;
  } catch {
    // Unparsable, scheme-less strings are NOT restricted here — the caller's
    // `url === ''` check and `captureTabHtml`'s own failure cover those.
  }
  return false;
}

/**
 * Resolve the active tab ONCE with its url, or `null` when it can never be
 * read: no tab, a redacted empty url, or a restricted kind. Everything a
 * multi-step gesture does afterwards must target THIS tab, not "whatever is
 * active" at each await point.
 */
export async function readableActiveTab(
  windowId?: number
): Promise<{ tabId: number; url: string } | null> {
  const tab = await activeTabIn(windowId);
  const tabId = tab?.id;
  const url = tab?.url ?? '';
  if (typeof tabId !== 'number' || url === '' || isPermanentlyUnreadablePage(url)) return null;
  return { tabId, url };
}

/** Register `file` on the page (a classic-script injection). */
export async function injectFile(tabId: number, file: string): Promise<void> {
  await browser.scripting.executeScript({ target: { tabId }, files: [file] });
}

/**
 * Call the page global `key` (installed by an earlier {@link injectFile}) with
 * `args` and return its completion value, `null` when the global is absent.
 * Two steps (file, then func) so PII — the contact profile, a résumé, an
 * answer — rides in transiently as an `executeScript` arg instead of through
 * any stored or registered surface. Only JSON-safe values may cross: Chrome
 * JSON-serializes `args`, so a `Uint8Array` would arrive as `{"0":…}`. The key
 * is appended LAST and read back from the tail, so the injected func is
 * self-contained (params + `globalThis` only).
 */
export async function callPageGlobal(
  tabId: number,
  key: string,
  args: unknown[]
): Promise<unknown> {
  const results = await browser.scripting.executeScript({
    target: { tabId },
    func: (...a: unknown[]): unknown => {
      const runner = (globalThis as Record<string, unknown>)[a[a.length - 1] as string] as
        ((...rest: unknown[]) => unknown) | undefined;
      return runner ? runner(...a.slice(0, -1)) : null;
    },
    args: [...args, key],
  });
  return results[0]?.result;
}

/** {@link injectFile} then {@link callPageGlobal}. */
export async function injectAndRun(
  tabId: number,
  file: string,
  key: string,
  args: unknown[]
): Promise<unknown> {
  await injectFile(tabId, file);
  return callPageGlobal(tabId, key, args);
}

/** Inject a data-collecting `file` and return its completion value, shape
 *  checked by `guard` (else throw `failMessage`). */
export async function readPage<T>(
  tabId: number,
  file: string,
  guard: (v: unknown) => v is T,
  failMessage: string
): Promise<T> {
  const results = await browser.scripting.executeScript({ target: { tabId }, files: [file] });
  const value = results[0]?.result;
  if (!guard(value)) throw new Error(failMessage);
  return value;
}

/** Inject the capture script and return the page's `outerHTML`. Requires
 *  `scripting` + `activeTab` (granted on the click). */
export async function captureTabHtml(tabId: number): Promise<string> {
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
