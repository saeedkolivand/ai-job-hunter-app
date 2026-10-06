/**
 * Shared harness for the background service worker's tests (`*.test.ts` here).
 *
 * The entry (`../background.ts`) has no usable exports — its popup-request
 * dispatcher is only reachable through the `browser.runtime.onMessage` listener
 * it registers at module load. So this module mocks `@wxt-dev/browser` +
 * `../lib/storage` + `../lib/bridge` BEFORE the dynamic import (so module-load
 * side effects see mocked dependencies), grabs the registered listener, and
 * every test drives it with typed `PopupRequest` messages, asserting the
 * `PopupResponse`.
 *
 * Importing this module from a test file is what installs the mocks (vitest
 * hoists `vi.mock` to the top of this file, ahead of its own imports) and loads
 * the worker once per test file.
 */

import { type Mock, vi } from 'vitest';
import { type Browser, browser } from '@wxt-dev/browser';

import type { AnswerState } from '../lib/answer-state';
import type { AutofillSummary } from '../lib/autofill';
import type { PopupRequest, PopupResponse } from '../lib/messages';
import { getToken } from '../lib/storage';

// Re-exported so test files never import a mocked-dependency module themselves:
// anything that reaches `@wxt-dev/browser` BEFORE this module has registered its
// mocks would bind to the real one.
const mockedBrowser = browser;
export { mockedBrowser as browser };

/** The extension's own id (mocked) — the trusted `sender` for the submit-watcher's
 *  fire-and-forget message (see `message-listener.ts`'s sender check). */
export const EXTENSION_ID = 'test-extension-id';

/** Hoisted so the `vi.mock('../lib/bridge')` factory can hand out this SAME
 *  instance — `getClient()` lazily constructs ONE client for the worker's
 *  lifetime, and every test drives it (reset in {@link resetMocks}). */
type ClientMethod =
  | 'status'
  | 'ensureConnected'
  | 'resetForNewToken'
  | 'importJob'
  | 'getProfile'
  | 'checkApplied'
  | 'checkAppliedBatch'
  | 'updateStatus'
  | 'saveAnswers'
  | 'suggestAnswers'
  | 'matchLive'
  | 'answerAssist'
  | 'cancelCurrent'
  | 'autotrackEnabled'
  | 'agentQuery'
  | 'settingsGet'
  | 'settingsSet'
  | 'documentExport';

const hoistedClient = vi.hoisted((): Record<ClientMethod, Mock> => ({
  status: vi.fn(() => ({ phase: 'connected' as const, port: 47615, authenticated: true })),
  ensureConnected: vi.fn().mockResolvedValue(undefined),
  resetForNewToken: vi.fn(),
  importJob: vi.fn(),
  getProfile: vi.fn(),
  checkApplied: vi.fn(),
  checkAppliedBatch: vi.fn(),
  updateStatus: vi.fn(),
  saveAnswers: vi.fn(),
  suggestAnswers: vi.fn(),
  matchLive: vi.fn(),
  answerAssist: vi.fn(),
  cancelCurrent: vi.fn(),
  autotrackEnabled: vi.fn(),
  agentQuery: vi.fn(),
  settingsGet: vi.fn(),
  settingsSet: vi.fn(),
  documentExport: vi.fn(),
}));

export const mockClient = hoistedClient;

vi.mock('@wxt-dev/browser', () => {
  /** In-memory `storage` area: `session` holds the shared answer state, `local`
   *  holds `lib/appearance.ts`'s preferences. The background is the only writer. */
  const area = () => {
    const store: Record<string, unknown> = {};
    return {
      get: vi.fn((key: string) => Promise.resolve({ [key]: store[key] })),
      set: vi.fn((entries: Record<string, unknown>) => {
        Object.assign(store, entries);
        return Promise.resolve();
      }),
      remove: vi.fn((key: string) => {
        delete store[key];
        return Promise.resolve();
      }),
    };
  };
  return {
    browser: {
      runtime: {
        id: 'test-extension-id',
        onMessage: { addListener: vi.fn() },
        onStartup: { addListener: vi.fn() },
        onInstalled: { addListener: vi.fn() },
        onUpdateAvailable: { addListener: vi.fn() },
        sendMessage: vi.fn(),
        reload: vi.fn(),
      },
      // A navigation invalidates a tab's answer state for WRITING and a closed
      // tab drops it, so the entry subscribes to both at module load.
      tabs: {
        query: vi.fn(),
        onUpdated: { addListener: vi.fn() },
        onRemoved: { addListener: vi.fn() },
      },
      scripting: { executeScript: vi.fn() },
      // `removeAll` takes the callback `installContextMenu` passes it, so the
      // mock has to invoke it or `create` is never reached.
      contextMenus: {
        removeAll: vi.fn((cb?: () => void) => cb?.()),
        create: vi.fn(),
        onClicked: { addListener: vi.fn() },
      },
      sidePanel: { open: vi.fn().mockResolvedValue(undefined) },
      storage: {
        session: area(),
        local: area(),
        onChanged: { addListener: vi.fn(), removeListener: vi.fn() },
      },
      action: {
        setBadgeText: vi.fn().mockResolvedValue(undefined),
        setBadgeBackgroundColor: vi.fn().mockResolvedValue(undefined),
      },
    },
  };
});

vi.mock('../lib/storage', () => ({
  getToken: vi.fn(),
  setToken: vi.fn(),
  clearToken: vi.fn(),
  looksLikeToken: vi.fn(() => true),
}));

vi.mock('../lib/bridge', () => ({
  // A regular `function` (not an arrow) so `new BridgeClient(...)` — as
  // `getClient()` does — is constructible; an arrow implementation throws "is
  // not a constructor" under `new`.
  BridgeClient: vi.fn(function BridgeClientMock() {
    return hoistedClient;
  }),
}));

// Every listener under test is recorded on these mocks by the import BELOW,
// once, before any test body runs. Vitest 5 turned `clearMocks` on by default
// (a `vi.clearAllMocks()` before every test), which wipes exactly that history
// (https://vitest.dev/guide/migration#clearmocks-is-enabled-by-default). Opt
// out; the runner restores the config after the file, and per-test isolation
// stays explicit in {@link resetMocks}.
vi.setConfig({ clearMocks: false });

// Dynamic imports AFTER the mocks are in place — a static `import`/`export from`
// of anything that reaches `@wxt-dev/browser` would be evaluated first and bind
// the real one.
export const backgroundModule = await import('../background');
export const { readAnswerState, updateAnswerState, writeAnswerState } =
  await import('../lib/answer-state');
export const { setShowFitBadge, setStampResultsPages } = await import('../lib/appearance');

// Loosely typed on purpose: the real overloads (callback + promise) make the typed
// mocks reject every promise-style `mockResolvedValue`.
export const tabsQueryMock = browser.tabs.query as unknown as Mock;
export const executeScriptMock = browser.scripting.executeScript as unknown as Mock;
export const getTokenMock = vi.mocked(getToken);
export const setBadgeTextMock = vi.mocked(browser.action.setBadgeText);

/** The `onMessage` callback the entry registered at module load. */
export const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
  | ((
      message: unknown,
      sender: Browser.runtime.MessageSender,
      sendResponse: (response?: PopupResponse) => void
    ) => true | undefined)
  | undefined;

/** The `contextMenus.onClicked` callback (throws when the entry never registered it). */
export function contextMenuClick(): NonNullable<
  Parameters<typeof browser.contextMenus.onClicked.addListener>[0]
> {
  const onClicked = vi.mocked(browser.contextMenus.onClicked.addListener).mock.calls[0]?.[0];
  if (!onClicked) throw new Error('context-menu click listener not registered');
  return onClicked;
}

/**
 * Drive the registered listener the way the browser does: hand it a
 * `sendResponse` callback and resolve on the reply.
 *
 * The `kept === true` assertion is the point — Chromium keeps the message
 * channel open for an async reply ONLY for a literal `true` return. Anything
 * else (including a returned Promise, which is truthy but not `true`) closes it
 * immediately and `sendMessage` resolves `undefined`, which the popup reports as
 * "No response from the extension background."
 */
export function send(req: PopupRequest): Promise<PopupResponse> {
  if (!listener) throw new Error('onMessage listener not registered');
  return new Promise<PopupResponse>((resolve, reject) => {
    const kept = listener(
      req,
      { id: EXTENSION_ID } as Browser.runtime.MessageSender,
      (response?: PopupResponse) => {
        if (response) resolve(response);
        else reject(new Error('listener called sendResponse with no response'));
      }
    );
    if (kept !== true) {
      reject(
        new Error(`listener must return literal true to keep the channel open, got ${String(kept)}`)
      );
    }
  });
}

/** Flush the fire-and-forget async work a request or the raw listener kicks off
 *  without awaiting (`void handleSubmitDetected(...)`, `void armSubmitWatch(...)`). */
export function flush(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

export const FAKE_TOKEN = 'a'.repeat(64);
export const POSTING_URL = 'https://jobs.example.com/posting/9';
export const NOT_PAIRED = { ok: false, error: 'Not paired. Paste your pairing token first.' };
export const UNREADABLE_PAGE = {
  ok: false,
  error: "This page can't be read by the extension — there's nothing to work with here.",
};
export const RELOAD_HINT = {
  ok: false,
  error: 'Could not read this page. Reload it and try again.',
};
/** A fill.js completion value: one field filled. */
export const EMAIL_SUMMARY: AutofillSummary = {
  filled: [{ key: 'email', label: 'Email', count: 1 }],
  nameSplit: null,
  filledNothing: false,
};
export const DESKTOP_DOWN = 'Desktop app not reachable. Is AI Job Hunter running?';

/** The page urls Chrome/Firefox can never let the extension read (#1219):
 *  `[label, url]` rows for `it.each`. */
export const UNREADABLE_URLS: [string, string][] = [
  ['chrome://settings', 'chrome://settings'],
  ['chrome://extensions', 'chrome://extensions/'],
  [
    'chrome-extension://abcdefghijklmnop/content/index.html',
    'chrome-extension://abcdefghijklmnop/content/index.html',
  ],
  ['about:blank', 'about:blank'],
  ['about:newtab', 'about:newtab'],
  ['moz-extension://abcdefghijklmnop/page.html', 'moz-extension://abcdefghijklmnop/page.html'],
  ['the built-in PDF viewer', 'resource://pdf.js/web/viewer.html'],
  [
    'the Chrome Web Store',
    'https://chromewebstore.google.com/detail/some-extension/abcdefghijklmnop',
  ],
  ['a .pdf file', 'https://example.com/job-posting/attachment.pdf'],
];

/** Re-pin every mock's state — call from `beforeEach` in each test file. */
export async function resetMocks(): Promise<void> {
  getTokenMock.mockReset();
  tabsQueryMock.mockReset();
  executeScriptMock.mockReset();
  for (const [name, fn] of Object.entries(mockClient)) {
    // `status`/`ensureConnected`/`resetForNewToken`/`cancelCurrent` keep their
    // implementations (and call history) — only the request mocks are reset.
    if (!['status', 'ensureConnected', 'resetForNewToken', 'cancelCurrent'].includes(name)) {
      fn.mockReset();
    }
  }
  setBadgeTextMock.mockClear();
  // PR3 preferences default OFF — re-pin them before every test so an earlier
  // test's `setShowFitBadge(true)`/`setStampResultsPages(true)` never leaks into
  // the next one via the shared in-memory storage.local mock.
  await setShowFitBadge(false);
  await setStampResultsPages(false);
}

// ── arrange helpers ──────────────────────────────────────────────────────────

/** `tabs.query` resolves one active tab. */
export function activeTab(url: string = POSTING_URL, id = 7): void {
  tabsQueryMock.mockResolvedValue([{ id, url } as never]);
}

/** Queue the active tab `tabs.query` resolves on each successive call, as `[url, tabId]`. */
export function tabSequence(...tabs: [url: string, id?: number][]): void {
  for (const [url, id = 7] of tabs) tabsQueryMock.mockResolvedValueOnce([{ id, url } as never]);
}

/** Queue one `executeScript` completion value per injection, in call order — a
 *  `files:` registration step has no value worth asserting, pass `undefined`. */
export function scriptResults(...values: unknown[]): void {
  for (const result of values) executeScriptMock.mockResolvedValueOnce([{ result }] as never);
}

/** A paired browser on `url`'s tab — the common arrange of a gesture test. */
export function paired(url?: string, tabId?: number): void {
  getTokenMock.mockResolvedValue(FAKE_TOKEN);
  activeTab(url, tabId);
}

/** A successful `document.export` reply for a résumé. */
export function resumeExport(encoded = btoa('%PDF-1.4 fake')): Record<string, unknown> {
  return {
    ok: true,
    data: encoded,
    dataEncoding: 'base64',
    mimeType: 'application/pdf',
    filename: 'resume.pdf',
    byteLength: encoded.length,
    kind: 'resume',
    format: 'pdf',
    templateId: 'classic',
  };
}

/** A successful `match.live` reply. */
export function matchOk(extra: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    ok: true,
    combined: 82,
    ats: 60,
    gaps: [],
    resumeName: 'My Resume',
    scoreSource: 'keyword',
    ...extra,
  };
}

/** A successful `answer.assist` reply. */
export function assistOk(draft: string, question = 'Why this role?'): Record<string, unknown> {
  return { ok: true, question, draft, sourced: {} };
}

/** Unwrap an `answerState` response, or fail the test. */
export function stateOf(res: PopupResponse): AnswerState {
  if (!res.ok || res.kind !== 'answerState' || !res.state) {
    throw new Error('expected an answerState response');
  }
  return res.state;
}

/**
 * Scan a page whose collector reports `questions` on `tabId` (each test picks
 * its OWN tab id — the mocked `storage.session` is a module-level store, never
 * reset between tests) and return the new rows' ids.
 */
export async function scanRows(tabId: number, ...questions: string[]): Promise<string[]> {
  activeTab(`https://jobs.example.com/posting/${tabId}`, tabId);
  scriptResults({ questions: questions.map((question) => ({ question, index: 0 })), filled: [] });
  mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'not paired' });
  const { rows } = stateOf(await send({ kind: 'answerScan' }));
  const ids = rows.map((r) => r.id);
  if (ids.length < questions.length) throw new Error('expected a row per question');
  return ids;
}

/** The `executeScript` call whose injected func receives the fit-badge global. */
export function fitBadgeRenderCall(): { func: (...a: unknown[]) => void; args: unknown[] } {
  const call = executeScriptMock.mock.calls.find(
    (c) => (c[0] as { args?: unknown[] }).args?.[1] === '__ajhRenderFitBadge'
  );
  return call?.[0] as { func: (...a: unknown[]) => void; args: unknown[] };
}
