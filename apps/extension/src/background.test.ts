/**
 * Unit tests for the background service worker's assisted-autofill orchestration
 * (apps/extension/src/background.ts).
 *
 * background.ts has no named exports — its popup-request dispatcher
 * (`handleRequest`) is only reachable via the `browser.runtime.onMessage`
 * listener it wires at module load. Mirrors the `popup.test.ts` /
 * `storage.test.ts` mocking style: mock `@wxt-dev/browser` + `./lib/storage` +
 * `./lib/bridge` BEFORE the dynamic import (so module-load side effects —
 * `onMessage.addListener`, the initial `ensureConnected()` probe — see mocked
 * dependencies), then grab the registered listener and drive it with typed
 * `PopupRequest` messages, asserting the `PopupResponse`.
 *
 * `vi.hoisted()` builds the shared mock `BridgeClient` instance BEFORE the
 * `vi.mock('./lib/bridge', ...)` factory runs (vi.mock is hoisted above
 * imports by vitest's transform) — same pattern as storage.test.ts's hoisted
 * in-memory store.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { type Browser, browser } from '@wxt-dev/browser';

import type { AnswerRow } from './lib/answer-state';
import type { AutofillSummary } from './lib/autofill';
import type { PopupRequest, PopupResponse } from './lib/messages';
import { getToken } from './lib/storage';
import { SUBMIT_DETECTED_MSG } from './lib/submit-watch';

/** The extension's own id (mocked) — used to build a trusted `sender` for the
 *  submit-watcher's fire-and-forget message (see `background.ts`'s
 *  belt-and-braces `sender.id === browser.runtime.id` check). */
const EXTENSION_ID = 'test-extension-id';

// ── hoisted mock BridgeClient (background.ts's getClient() lazily constructs
// ONE client and caches it for the worker's lifetime — every test drives this
// SAME mock instance, reset in beforeEach) ────────────────────────────────────
const mockClient = vi.hoisted(() => ({
  status: vi.fn(() => ({ phase: 'connected' as const, port: 47615 })),
  ensureConnected: vi.fn().mockResolvedValue(undefined),
  resetForNewToken: vi.fn(),
  importJob: vi.fn(),
  getProfile: vi.fn(),
  checkApplied: vi.fn(),
  updateStatus: vi.fn(),
  saveAnswers: vi.fn(),
  suggestAnswers: vi.fn(),
  matchLive: vi.fn(),
  answerAssist: vi.fn(),
  autotrackEnabled: vi.fn(),
}));

vi.mock('@wxt-dev/browser', () => ({
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
    tabs: {
      query: vi.fn(),
      // ADR-044: a navigation invalidates a tab's answer state for WRITING and
      // a closed tab drops it entirely, so background.ts subscribes to both at
      // module load. Registered here so that load does not throw.
      onUpdated: { addListener: vi.fn() },
      onRemoved: { addListener: vi.fn() },
    },
    scripting: { executeScript: vi.fn() },
    // The two context-menu entries (selection-only, and plain-page).
    // `removeAll` takes the callback `installContextMenu` passes it, so the
    // mock has to invoke it or `create` is never reached.
    contextMenus: {
      removeAll: vi.fn((cb?: () => void) => cb?.()),
      create: vi.fn(),
      onClicked: { addListener: vi.fn() },
    },
    // `openAnswerPanel`'s Chrome branch — asserted directly by the
    // page-context menu item's click test, since that item has no row to
    // read back the way the selection item's tests do.
    sidePanel: { open: vi.fn().mockResolvedValue(undefined) },
    // The shared answer state lives in `storage.session`; `onChanged` is what
    // both surfaces subscribe to. An in-memory area is enough here — the
    // background is the only writer.
    storage: {
      session: (() => {
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
      })(),
      onChanged: { addListener: vi.fn(), removeListener: vi.fn() },
    },
    action: {
      setBadgeText: vi.fn().mockResolvedValue(undefined),
      setBadgeBackgroundColor: vi.fn().mockResolvedValue(undefined),
    },
  },
}));

vi.mock('./lib/storage', () => ({
  getToken: vi.fn(),
  setToken: vi.fn(),
  clearToken: vi.fn(),
  looksLikeToken: vi.fn(() => true),
}));

vi.mock('./lib/bridge', () => ({
  // A regular `function` (not an arrow) so `new BridgeClient(...)` — as
  // background.ts's getClient() does — is constructible; an arrow-function
  // implementation throws "is not a constructor" under `new`.
  BridgeClient: vi.fn(function BridgeClientMock() {
    return mockClient;
  }),
}));

// Dynamic import AFTER the mocks are in place — background.ts registers its
// onMessage listener + kicks an initial ensureConnected() probe at module load.
const backgroundModule = await import('./background');

const tabsQueryMock = vi.mocked(browser.tabs.query);
const executeScriptMock = vi.mocked(browser.scripting.executeScript);
const getTokenMock = vi.mocked(getToken);
const setBadgeTextMock = vi.mocked(browser.action.setBadgeText);

/** The `handleRequest`-wrapping callback background.ts registered at module load. */
const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
  | ((
      message: unknown,
      sender: Browser.runtime.MessageSender,
      sendResponse: (response?: PopupResponse) => void
    ) => true | undefined)
  | undefined;

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
function send(req: PopupRequest): Promise<PopupResponse> {
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

/** Flush the fire-and-forget async work `handleRequest`/the raw listener kick
 *  off without awaiting (`void handleSubmitDetected(...)`, `void
 *  maybeArmSubmitWatch(...)`) — both settle within a couple of microtask/timer
 *  turns, mirrored from the existing streaming-race tests further down. */
function flush(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

const FAKE_TOKEN = 'a'.repeat(64);

beforeEach(() => {
  getTokenMock.mockReset();
  tabsQueryMock.mockReset();
  executeScriptMock.mockReset();
  mockClient.getProfile.mockReset();
  mockClient.importJob.mockReset();
  mockClient.checkApplied.mockReset();
  mockClient.updateStatus.mockReset();
  mockClient.saveAnswers.mockReset();
  mockClient.suggestAnswers.mockReset();
  mockClient.matchLive.mockReset();
  mockClient.answerAssist.mockReset();
  mockClient.autotrackEnabled.mockReset();
  setBadgeTextMock.mockClear();
});

// ── not-paired short-circuit ────────────────────────────────────────────────

describe('fill request — not-paired short-circuit', () => {
  it('surfaces "Not paired" and never reaches the profile fetch or executeScript when no token is stored', async () => {
    getTokenMock.mockResolvedValue(null);

    const res = await send({ kind: 'fill' });

    expect(res).toEqual({ ok: false, error: 'Not paired. Paste your pairing token first.' });
    expect(mockClient.getProfile).not.toHaveBeenCalled();
    expect(executeScriptMock).not.toHaveBeenCalled();
  });
});

// ── desktop refusal (resolve_profile: autofill opt-in OFF) ──────────────────

describe('fill request — desktop refusal', () => {
  it('surfaces the profile.result refusal payload (opt-in off) and never injects the filler', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.getProfile.mockResolvedValue({
      error:
        'Autofill is off. Turn it on in AI Job Hunter → Settings → Accounts → Browser extension.',
    });

    const res = await send({ kind: 'fill' });

    expect(res).toEqual({
      ok: false,
      error:
        'Autofill is off. Turn it on in AI Job Hunter → Settings → Accounts → Browser extension.',
    });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });

  it('surfaces a transport failure when getProfile REJECTS (desktop unreachable), never injects the filler', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.getProfile.mockRejectedValue(
      new Error('Desktop app not reachable. Is AI Job Hunter running?')
    );

    const res = await send({ kind: 'fill' });

    // handleRequest's outer try/catch converts a thrown Error to ok:false.
    expect(res).toEqual({
      ok: false,
      error: 'Desktop app not reachable. Is AI Job Hunter running?',
    });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });
});

// ── no active tab ────────────────────────────────────────────────────────────

describe('fill request — no active tab', () => {
  it('surfaces "No active tab to fill." when the tab query returns none', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.getProfile.mockResolvedValue({ email: 'saeed@example.com' });
    tabsQueryMock.mockResolvedValue([]);

    const res = await send({ kind: 'fill' });

    expect(res).toEqual({ ok: false, error: 'No active tab to fill.' });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });
});

// ── malformed injected result ────────────────────────────────────────────────

describe('fill request — malformed injected result', () => {
  it('surfaces "Could not fill the form on this page." when the injected func returns a non-summary', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.getProfile.mockResolvedValue({ email: 'saeed@example.com' });
    tabsQueryMock.mockResolvedValue([{ id: 7, url: 'https://example.com/apply' } as never]);
    executeScriptMock
      .mockResolvedValueOnce([] as never) // step 1: files:['fill.js'] injection — return value unused
      .mockResolvedValueOnce([{ result: null }] as never); // step 2: func call returns a non-summary

    const res = await send({ kind: 'fill' });

    expect(res).toEqual({ ok: false, error: 'Could not fill the form on this page.' });
    expect(executeScriptMock).toHaveBeenCalledTimes(2);
  });
});

// ── success path (sanity — proves the harness itself is wired correctly) ────

describe('fill request — success path', () => {
  it('returns the fill summary when the profile, tab, and injection all succeed', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.getProfile.mockResolvedValue({ email: 'saeed@example.com' });
    tabsQueryMock.mockResolvedValue([{ id: 7, url: 'https://example.com/apply' } as never]);
    const summary: AutofillSummary = {
      filled: [{ key: 'email', label: 'Email', count: 1 }],
      nameSplit: null,
      filledNothing: false,
    };
    executeScriptMock
      .mockResolvedValueOnce([] as never)
      .mockResolvedValueOnce([{ result: summary }] as never);

    const res = await send({ kind: 'fill' });

    expect(res).toEqual({ ok: true, kind: 'fill', summary });
  });

  it('forwards extraLinks from the profile.result reply into the injected fill fields', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.getProfile.mockResolvedValue({
      email: 'saeed@example.com',
      extraLinks: [{ label: 'Portfolio', url: 'https://saeed.dev' }],
    });
    tabsQueryMock.mockResolvedValue([{ id: 7, url: 'https://example.com/apply' } as never]);
    const summary: AutofillSummary = {
      filled: [{ key: 'extraLink:Portfolio', label: 'Portfolio', count: 1 }],
      nameSplit: null,
      filledNothing: false,
    };
    executeScriptMock
      .mockResolvedValueOnce([] as never)
      .mockResolvedValueOnce([{ result: summary }] as never);

    await send({ kind: 'fill' });

    const secondCallArgs = executeScriptMock.mock.calls[1]?.[0] as { args?: unknown[] };
    const [fields] = secondCallArgs.args as [{ extraLinks?: unknown }];
    expect(fields.extraLinks).toEqual([{ label: 'Portfolio', url: 'https://saeed.dev' }]);
  });
});

// ── appliedCheck request — always ok:true, every failure folds into found:false ─

describe('appliedCheck request', () => {
  it('returns the checkApplied result on success', async () => {
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    mockClient.checkApplied.mockResolvedValue({
      found: true,
      applicationId: 'app-1',
      status: 'applied',
      appliedAt: 1_718_000_000_000,
    });

    const res = await send({ kind: 'appliedCheck' });

    expect(res).toEqual({
      ok: true,
      kind: 'appliedCheck',
      result: {
        found: true,
        applicationId: 'app-1',
        status: 'applied',
        appliedAt: 1_718_000_000_000,
      },
    });
    expect(mockClient.checkApplied).toHaveBeenCalledWith('https://jobs.example.com/posting/9');
  });

  it('folds a checkApplied REJECTION (e.g. old-desktop unknown message type) into found:false, never ok:false', async () => {
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    mockClient.checkApplied.mockRejectedValue(
      new Error("The desktop app sent an error: unknown message type 'applied.check'")
    );

    const res = await send({ kind: 'appliedCheck' });

    expect(res).toEqual({ ok: true, kind: 'appliedCheck', result: { found: false } });
  });

  it('folds a missing active tab into found:false, never ok:false', async () => {
    tabsQueryMock.mockResolvedValue([]);

    const res = await send({ kind: 'appliedCheck' });

    expect(res).toEqual({ ok: true, kind: 'appliedCheck', result: { found: false } });
    expect(mockClient.checkApplied).not.toHaveBeenCalled();
  });
});

// ── fieldsProbe request — always ok:true, EVERY failure fails OPEN (unlike
// appliedCheck, which fails CLOSED into found:false) ────────────────────────

describe('fieldsProbe request', () => {
  it('returns the injected probe result on success (both signals true)', async () => {
    tabsQueryMock.mockResolvedValue([{ id: 7, url: 'https://jobs.example.com/apply' } as never]);
    executeScriptMock.mockResolvedValueOnce([
      { result: { hasFormFields: true, hasAnswerFields: true } },
    ] as never);

    const res = await send({ kind: 'fieldsProbe' });

    expect(res).toEqual({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: true,
      hasAnswerFields: true,
    });
    expect(executeScriptMock).toHaveBeenCalledWith({
      target: { tabId: 7 },
      files: ['probe-fields.js'],
    });
    // Never touches the token/bridge — this is a page-only, offline-safe read.
    expect(getTokenMock).not.toHaveBeenCalled();
  });

  it('passes through an identity-only-form result (hasFormFields true, hasAnswerFields false — the union split)', async () => {
    tabsQueryMock.mockResolvedValue([{ id: 7, url: 'https://jobs.example.com/apply' } as never]);
    executeScriptMock.mockResolvedValueOnce([
      { result: { hasFormFields: true, hasAnswerFields: false } },
    ] as never);

    const res = await send({ kind: 'fieldsProbe' });

    expect(res).toEqual({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: true,
      hasAnswerFields: false,
    });
  });

  it('returns the injected probe result on success (no fields at all)', async () => {
    tabsQueryMock.mockResolvedValue([{ id: 7, url: 'https://jobs.example.com/listing' } as never]);
    executeScriptMock.mockResolvedValueOnce([
      { result: { hasFormFields: false, hasAnswerFields: false } },
    ] as never);

    const res = await send({ kind: 'fieldsProbe' });

    expect(res).toEqual({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: false,
      hasAnswerFields: false,
    });
  });

  it('fails OPEN (both signals true) when there is no active tab', async () => {
    tabsQueryMock.mockResolvedValue([]);

    const res = await send({ kind: 'fieldsProbe' });

    expect(res).toEqual({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: true,
      hasAnswerFields: true,
    });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });

  it('fails OPEN (both signals true) when the injected result is malformed (missing/non-boolean fields)', async () => {
    tabsQueryMock.mockResolvedValue([{ id: 7, url: 'https://jobs.example.com/apply' } as never]);
    executeScriptMock.mockResolvedValueOnce([{ result: { hasFormFields: true } }] as never);

    const res = await send({ kind: 'fieldsProbe' });

    expect(res).toEqual({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: true,
      hasAnswerFields: true,
    });
  });

  it('fails OPEN (both signals true) when executeScript REJECTS (restricted page/scripting denied)', async () => {
    tabsQueryMock.mockResolvedValue([{ id: 7, url: 'https://jobs.example.com/apply' } as never]);
    executeScriptMock.mockRejectedValueOnce(new Error('Cannot access a chrome:// URL'));

    const res = await send({ kind: 'fieldsProbe' });

    expect(res).toEqual({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: true,
      hasAnswerFields: true,
    });
  });
});

// ── statusUpdate request — errors are NOT folded (unlike appliedCheck) ────────

describe('statusUpdate request', () => {
  it('returns the updateStatus success result', async () => {
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    mockClient.updateStatus.mockResolvedValue({
      ok: true,
      applicationId: 'app-1',
      status: 'applied',
    });

    const res = await send({ kind: 'statusUpdate' });

    expect(res).toEqual({
      ok: true,
      kind: 'statusUpdate',
      result: { ok: true, applicationId: 'app-1', status: 'applied' },
    });
    expect(mockClient.updateStatus).toHaveBeenCalledWith('https://jobs.example.com/posting/9');
  });

  it('passes a desktop-side refusal straight through as result (never folds it, unlike appliedCheck)', async () => {
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/none' } as never,
    ]);
    mockClient.updateStatus.mockResolvedValue({
      ok: false,
      error: "couldn't find a saved job for this page",
    });

    const res = await send({ kind: 'statusUpdate' });

    expect(res).toEqual({
      ok: true,
      kind: 'statusUpdate',
      result: { ok: false, error: "couldn't find a saved job for this page" },
    });
  });

  it('surfaces a transport-level rejection as ok:false at the OUTER level (UNLIKE appliedCheck, which folds every rejection)', async () => {
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    mockClient.updateStatus.mockRejectedValue(
      new Error('Desktop app not reachable. Is AI Job Hunter running?')
    );

    const res = await send({ kind: 'statusUpdate' });

    expect(res).toEqual({
      ok: false,
      error: 'Desktop app not reachable. Is AI Job Hunter running?',
    });
  });

  it('surfaces "Could not read the current tab URL." when there is no active tab, without calling updateStatus', async () => {
    tabsQueryMock.mockResolvedValue([]);

    const res = await send({ kind: 'statusUpdate' });

    expect(res).toEqual({ ok: false, error: 'Could not read the current tab URL.' });
    expect(mockClient.updateStatus).not.toHaveBeenCalled();
  });
});

// ── answersSave request — capture then send; errors are NOT folded ──────────

describe('answersSave request — not-paired short-circuit', () => {
  it('surfaces "Not paired" and never reaches the tab capture or saveAnswers when no token is stored', async () => {
    getTokenMock.mockResolvedValue(null);

    const res = await send({ kind: 'answersSave' });

    expect(res).toEqual({ ok: false, error: 'Not paired. Paste your pairing token first.' });
    expect(executeScriptMock).not.toHaveBeenCalled();
    expect(mockClient.saveAnswers).not.toHaveBeenCalled();
  });
});

describe('answersSave request', () => {
  it('injects capture.js, sends the captured answers, and returns the success result', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    const captured = [{ question: 'Why this role?', answer: 'Because I love it.' }];
    const filled = [{ question: 'Why this role?', index: 0, answer: 'Because I love it.' }];
    executeScriptMock.mockResolvedValueOnce([{ result: { answers: captured, filled } }] as never);
    mockClient.saveAnswers.mockResolvedValue({
      ok: true,
      applicationId: 'app-1',
      saved: 1,
      skipped: 0,
      title: 'Backend Engineer',
      company: 'Acme',
    });

    const res = await send({ kind: 'answersSave' });

    expect(executeScriptMock).toHaveBeenCalledWith(
      expect.objectContaining({ target: { tabId: 7 }, files: ['capture.js'] })
    );
    expect(mockClient.saveAnswers).toHaveBeenCalledWith(
      'https://jobs.example.com/posting/9',
      captured
    );
    expect(res).toEqual({
      ok: true,
      kind: 'answersSave',
      result: {
        ok: true,
        applicationId: 'app-1',
        saved: 1,
        skipped: 0,
        title: 'Backend Engineer',
        company: 'Acme',
      },
      filled,
    });
  });

  it('passes a desktop-side refusal straight through as result (never folds it, unlike appliedCheck)', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/none' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{ result: { answers: [], filled: [] } }] as never);
    mockClient.saveAnswers.mockResolvedValue({
      ok: false,
      error: "couldn't find a saved job for this page — import it first",
    });

    const res = await send({ kind: 'answersSave' });

    expect(res).toEqual({
      ok: true,
      kind: 'answersSave',
      result: { ok: false, error: "couldn't find a saved job for this page — import it first" },
      filled: [],
    });
  });

  it('surfaces "Could not read the answers on this page." when the injected script returns a non-array', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{ result: null }] as never);

    const res = await send({ kind: 'answersSave' });

    expect(res).toEqual({ ok: false, error: 'Could not read the answers on this page.' });
    expect(mockClient.saveAnswers).not.toHaveBeenCalled();
  });

  it('surfaces "Could not read the current tab URL." when there is no active tab, without calling saveAnswers', async () => {
    // activeTabUrl() runs BEFORE the capture injection (mirrors runStatusUpdate).
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([]);

    const res = await send({ kind: 'answersSave' });

    expect(res).toEqual({ ok: false, error: 'Could not read the current tab URL.' });
    expect(executeScriptMock).not.toHaveBeenCalled();
    expect(mockClient.saveAnswers).not.toHaveBeenCalled();
  });

  it('surfaces a transport-level rejection as ok:false (UNLIKE appliedCheck, which folds every rejection)', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{ result: { answers: [], filled: [] } }] as never);
    mockClient.saveAnswers.mockRejectedValue(
      new Error('Desktop app not reachable. Is AI Job Hunter running?')
    );

    const res = await send({ kind: 'answersSave' });

    expect(res).toEqual({
      ok: false,
      error: 'Desktop app not reachable. Is AI Job Hunter running?',
    });
  });
});

// ── answersSuggest request — scan then send; errors are NOT folded ─────────

describe('answersSuggest request — not-paired short-circuit', () => {
  it('surfaces "Not paired" and never reaches the tab scan or suggestAnswers when no token is stored', async () => {
    getTokenMock.mockResolvedValue(null);

    const res = await send({ kind: 'answersSuggest' });

    expect(res).toEqual({ ok: false, error: 'Not paired. Paste your pairing token first.' });
    expect(executeScriptMock).not.toHaveBeenCalled();
    expect(mockClient.suggestAnswers).not.toHaveBeenCalled();
  });
});

describe('answersSuggest request', () => {
  it('injects capture-questions.js, sends deduped labels, and returns the success result + scanned list', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    const scanned = [
      { question: 'Why this role?', index: 0 },
      { question: 'Why this role?', index: 0 }, // duplicate label text — deduped before send
    ];
    executeScriptMock.mockResolvedValueOnce([{ result: scanned }] as never);
    mockClient.suggestAnswers.mockResolvedValue({
      ok: true,
      suggestions: [
        { question: 'Why this role?', answer: 'Because I love it.', score: 0.8, salary: false },
      ],
    });

    const res = await send({ kind: 'answersSuggest' });

    expect(executeScriptMock).toHaveBeenCalledWith(
      expect.objectContaining({ target: { tabId: 7 }, files: ['capture-questions.js'] })
    );
    expect(mockClient.suggestAnswers).toHaveBeenCalledWith(['Why this role?']);
    expect(res).toEqual({
      ok: true,
      kind: 'answersSuggest',
      result: {
        ok: true,
        suggestions: [
          { question: 'Why this role?', answer: 'Because I love it.', score: 0.8, salary: false },
        ],
      },
      scanned,
    });
  });

  it('passes a desktop-side refusal straight through as result (never folds it, unlike appliedCheck)', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{ result: [] }] as never);
    mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'Autofill is off.' });

    const res = await send({ kind: 'answersSuggest' });

    expect(res).toEqual({
      ok: true,
      kind: 'answersSuggest',
      result: { ok: false, error: 'Autofill is off.' },
      scanned: [],
    });
  });

  it('surfaces "Could not read the questions on this page." when the injected script returns a non-array', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{ result: null }] as never);

    const res = await send({ kind: 'answersSuggest' });

    expect(res).toEqual({ ok: false, error: 'Could not read the questions on this page.' });
    expect(mockClient.suggestAnswers).not.toHaveBeenCalled();
  });

  it('surfaces a transport-level rejection as ok:false', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{ result: [] }] as never);
    mockClient.suggestAnswers.mockRejectedValue(
      new Error('Desktop app not reachable. Is AI Job Hunter running?')
    );

    const res = await send({ kind: 'answersSuggest' });

    expect(res).toEqual({
      ok: false,
      error: 'Desktop app not reachable. Is AI Job Hunter running?',
    });
  });
});

// ── matchLive request — capture then send; errors are NOT folded ───────────

describe('matchLive request — not-paired short-circuit', () => {
  it('surfaces "Not paired" and never reaches the tab capture or matchLive when no token is stored', async () => {
    getTokenMock.mockResolvedValue(null);

    const res = await send({ kind: 'matchLive' });

    expect(res).toEqual({ ok: false, error: 'Not paired. Paste your pairing token first.' });
    expect(executeScriptMock).not.toHaveBeenCalled();
    expect(mockClient.matchLive).not.toHaveBeenCalled();
  });
});

describe('matchLive request', () => {
  it('captures content.js, sends { url, html }, and returns the success result', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{ result: '<html>job</html>' }] as never);
    mockClient.matchLive.mockResolvedValue({
      ok: true,
      combined: 72,
      ats: 60,
      gaps: ['kubernetes'],
      resumeName: 'My Resume',
      scoreSource: 'keyword',
    });

    const res = await send({ kind: 'matchLive' });

    expect(executeScriptMock).toHaveBeenCalledWith(
      expect.objectContaining({ target: { tabId: 7 }, files: ['content.js'] })
    );
    expect(mockClient.matchLive).toHaveBeenCalledWith({
      url: 'https://jobs.example.com/posting/9',
      html: '<html>job</html>',
    });
    expect(res).toEqual({
      ok: true,
      kind: 'matchLive',
      result: {
        ok: true,
        combined: 72,
        ats: 60,
        gaps: ['kubernetes'],
        resumeName: 'My Resume',
        scoreSource: 'keyword',
      },
    });
  });

  it('passes a desktop-side refusal straight through as result (never folds it, unlike appliedCheck)', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{ result: '<html>job</html>' }] as never);
    mockClient.matchLive.mockResolvedValue({
      ok: false,
      error: 'Add a resume in AI Job Hunter first, then try Check fit again.',
    });

    const res = await send({ kind: 'matchLive' });

    expect(res).toEqual({
      ok: true,
      kind: 'matchLive',
      result: {
        ok: false,
        error: 'Add a resume in AI Job Hunter first, then try Check fit again.',
      },
    });
  });

  it('surfaces a fixed capture-failure message when the page DOM could not be captured — no URL-mode fallback', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    // Non-string / empty result → captureActiveTabHtml throws.
    executeScriptMock.mockResolvedValueOnce([{ result: null }] as never);

    const res = await send({ kind: 'matchLive' });

    expect(res).toEqual({
      ok: false,
      error: 'Could not read this page. Reload the job page and try again.',
    });
    expect(mockClient.matchLive).not.toHaveBeenCalled();
  });

  it('surfaces a transport-level rejection as ok:false', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{ result: '<html>job</html>' }] as never);
    mockClient.matchLive.mockRejectedValue(
      new Error('Desktop app not reachable. Is AI Job Hunter running?')
    );

    const res = await send({ kind: 'matchLive' });

    expect(res).toEqual({
      ok: false,
      error: 'Desktop app not reachable. Is AI Job Hunter running?',
    });
  });
});

// ── answerAssist request — first billable-AI verb; errors NOT folded ───────

describe('answerAssist request — not-paired short-circuit', () => {
  it('surfaces "Not paired" and never reaches the tab lookup or answerAssist when no token is stored', async () => {
    getTokenMock.mockResolvedValue(null);

    const res = await send({ kind: 'answerAssist', question: 'Why this role?', searchWeb: false });

    expect(res).toEqual({ ok: false, error: 'Not paired. Paste your pairing token first.' });
    expect(tabsQueryMock).not.toHaveBeenCalled();
    expect(mockClient.answerAssist).not.toHaveBeenCalled();
  });
});

describe('answerAssist request', () => {
  it('sends { question, url, searchWeb } and returns the success result', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    mockClient.answerAssist.mockResolvedValue({
      ok: true,
      question: 'Why this role?',
      draft: 'Because…',
      sourced: { brief: true },
    });

    const res = await send({ kind: 'answerAssist', question: 'Why this role?', searchWeb: true });

    expect(mockClient.answerAssist).toHaveBeenCalledWith(
      {
        question: 'Why this role?',
        searchWeb: true,
        url: 'https://jobs.example.com/posting/9',
      },
      expect.any(Function)
    );
    expect(res).toEqual({
      ok: true,
      kind: 'answerAssist',
      result: {
        ok: true,
        question: 'Why this role?',
        draft: 'Because…',
        sourced: { brief: true },
      },
    });
  });

  it('still sends the request without a url when the active tab url cannot be read (generic grounding)', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([{ id: 7, url: '' } as never]);
    mockClient.answerAssist.mockResolvedValue({
      ok: true,
      question: 'Why this role?',
      draft: 'Because…',
      sourced: {},
    });

    await send({ kind: 'answerAssist', question: 'Why this role?', searchWeb: false });

    expect(mockClient.answerAssist).toHaveBeenCalledWith(
      { question: 'Why this role?', searchWeb: false },
      expect.any(Function)
    );
  });

  it('forwards mode/existingAnswer/preset/instruction for a rewrite request (PR 11)', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    mockClient.answerAssist.mockResolvedValue({
      ok: true,
      question: 'Why this role?',
      draft: 'A shorter answer.',
      sourced: {},
    });

    await send({
      kind: 'answerAssist',
      question: 'Why this role?',
      searchWeb: false,
      mode: 'rewrite',
      existingAnswer: 'Because I really love it and want to work here.',
      preset: 'shorten',
    });

    expect(mockClient.answerAssist).toHaveBeenCalledWith(
      {
        question: 'Why this role?',
        searchWeb: false,
        url: 'https://jobs.example.com/posting/9',
        mode: 'rewrite',
        existingAnswer: 'Because I really love it and want to work here.',
        preset: 'shorten',
      },
      expect.any(Function)
    );
  });

  it('forwards a free-text instruction instead of a preset', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    mockClient.answerAssist.mockResolvedValue({
      ok: true,
      question: 'Why this role?',
      draft: 'A more confident answer.',
      sourced: {},
    });

    await send({
      kind: 'answerAssist',
      question: 'Why this role?',
      searchWeb: false,
      mode: 'rewrite',
      existingAnswer: 'Because I like it.',
      instruction: 'Make this sound more confident.',
    });

    expect(mockClient.answerAssist).toHaveBeenCalledWith(
      expect.objectContaining({
        mode: 'rewrite',
        instruction: 'Make this sound more confident.',
      }),
      expect.any(Function)
    );
  });

  it('passes a desktop-side refusal straight through as result (never folds it, unlike appliedCheck)', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    mockClient.answerAssist.mockResolvedValue({
      ok: false,
      error: 'AI answer drafting is off.',
    });

    const res = await send({ kind: 'answerAssist', question: 'Why this role?', searchWeb: false });

    expect(res).toEqual({
      ok: true,
      kind: 'answerAssist',
      result: { ok: false, error: 'AI answer drafting is off.' },
    });
  });

  it('surfaces a transport-level rejection as ok:false', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    mockClient.answerAssist.mockRejectedValue(
      new Error('Desktop app not reachable. Is AI Job Hunter running?')
    );

    const res = await send({ kind: 'answerAssist', question: 'Why this role?', searchWeb: false });

    expect(res).toEqual({
      ok: false,
      error: 'Desktop app not reachable. Is AI Job Hunter running?',
    });
  });
});

// ── answerAssist streaming buffer — background OWNS the accumulation so a
// popup that closes mid-stream and reopens can reattach ────────────────────

describe('answerAssist streaming buffer', () => {
  it('accumulates onChunk deltas, broadcasts progress, and answerAssistProgress reflects the final done state', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    const sendMessageMock = vi.mocked(browser.runtime.sendMessage);
    sendMessageMock.mockClear();
    mockClient.answerAssist.mockImplementation(async (_payload, onChunk?: (d: string) => void) => {
      onChunk?.('Because I ');
      onChunk?.('am drawn to it.');
      return {
        ok: true,
        question: 'Why this role?',
        draft: 'Because I am drawn to it.',
        sourced: {},
      };
    });

    await send({ kind: 'answerAssist', question: 'Why this role?', searchWeb: false });

    // At least one live progress push happened per chunk (best-effort, so we
    // only assert the FINAL broadcast carried the fully-accumulated text).
    const pushes = sendMessageMock.mock.calls
      .map((call) => call[0] as PopupResponse)
      .filter((m) => m.ok && m.kind === 'answerAssistProgress');
    expect(pushes.length).toBeGreaterThan(0);
    expect(pushes.at(-1)).toEqual({
      ok: true,
      kind: 'answerAssistProgress',
      text: 'Because I am drawn to it.',
      done: true,
      interrupted: false,
      rowId: '',
    });

    const progress = await send({ kind: 'answerAssistProgress' });
    expect(progress).toEqual({
      ok: true,
      kind: 'answerAssistProgress',
      text: 'Because I am drawn to it.',
      done: true,
      interrupted: false,
      rowId: '',
    });
  });

  it('marks the buffer interrupted when the stream fails after some text already accumulated', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    mockClient.answerAssist.mockImplementation(async (_payload, onChunk?: (d: string) => void) => {
      onChunk?.('Because I ');
      throw new Error('Connection to the desktop app closed.');
    });

    await expect(
      send({ kind: 'answerAssist', question: 'Why this role?', searchWeb: false })
    ).resolves.toEqual({
      ok: false,
      error: 'Connection to the desktop app closed.',
    });

    const progress = await send({ kind: 'answerAssistProgress' });
    expect(progress).toEqual({
      ok: true,
      kind: 'answerAssistProgress',
      text: 'Because I ',
      done: true,
      interrupted: true,
      rowId: '',
    });
  });

  it('a fresh answerAssist call resets the buffer, even after a prior interrupted stream', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    mockClient.answerAssist.mockImplementationOnce(
      async (_payload, onChunk?: (d: string) => void) => {
        onChunk?.('stale partial text');
        throw new Error('boom');
      }
    );
    await send({ kind: 'answerAssist', question: 'Q1', searchWeb: false });
    const interrupted = await send({ kind: 'answerAssistProgress' });
    expect(interrupted).toMatchObject({ text: 'stale partial text', interrupted: true });

    // A NEW call must reset the buffer — the stale interrupted text/flag from
    // the prior request must never leak into this one, even before the first
    // chunk of the new stream arrives.
    mockClient.answerAssist.mockImplementationOnce(
      async (_payload, onChunk?: (d: string) => void) => {
        const midStream = await send({ kind: 'answerAssistProgress' });
        expect(midStream).toEqual({
          ok: true,
          kind: 'answerAssistProgress',
          text: '',
          done: false,
          interrupted: false,
          rowId: '',
        });
        onChunk?.('fresh answer');
        return { ok: true, question: 'Q2', draft: 'fresh answer', sourced: {} };
      }
    );
    await send({ kind: 'answerAssist', question: 'Q2', searchWeb: false });
  });

  it('caps assistBuffer growth at 4000 chars even across many chunks, so the interrupted path never shows unbounded text', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    const bigChunk = 'x'.repeat(3_000);
    mockClient.answerAssist.mockImplementation(async (_payload, onChunk?: (d: string) => void) => {
      onChunk?.(bigChunk); // 3,000
      onChunk?.(bigChunk); // 6,000 — over the 4,000 cap
      throw new Error('stream interrupted');
    });

    await send({ kind: 'answerAssist', question: 'Why this role?', searchWeb: false });

    const progress = (await send({ kind: 'answerAssistProgress' })) as {
      text: string;
      done: boolean;
      interrupted: boolean;
    };
    expect(progress.done).toBe(true);
    expect(progress.interrupted).toBe(true);
    expect(progress.text.length).toBe(4_000);
  });

  // Reachable in production: MV3 tears down the popup on close, and
  // `reattachAssistProgress` re-renders an in-flight stream without
  // re-disabling `btnAssist` — closing the popup mid-stream and reopening it
  // lets the user re-click "Help me answer…" while the first call is still
  // in flight. Without the `assistGeneration` single-flight guard, run A's
  // late chunk and terminal write clobber run B's buffer once A settles.
  it("a superseded run's late chunk and terminal write never corrupt a newer overlapping run's buffer", async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);

    let chunkA: ((d: string) => void) | undefined;
    let resolveA: ((value: unknown) => void) | undefined;
    const pendingA = new Promise((resolve) => {
      resolveA = resolve;
    });
    mockClient.answerAssist.mockImplementationOnce(
      async (_payload, onChunk?: (d: string) => void) => {
        chunkA = onChunk;
        return pendingA;
      }
    );

    // Start run A (mirrors a stream left running when the popup closed) but
    // don't await it yet — it stays in flight. Flush a macrotask (not just a
    // microtask) so A's OWN setup awaits (getToken + activeTabUrl) fully
    // resolve and it reaches the actual streaming call (registering chunkA)
    // BEFORE run B ever starts — otherwise B's synchronous generation bump
    // would supersede A during its own setup, which is a different case
    // (covered by the "superseded before its own reset" test below).
    const runA = send({ kind: 'answerAssist', question: 'Q1', searchWeb: false });
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Run B (mirrors the reopened popup's re-click) starts and fully
    // completes while A is still pending.
    mockClient.answerAssist.mockImplementationOnce(
      async (_payload, onChunk?: (d: string) => void) => {
        onChunk?.('B chunk');
        return { ok: true, question: 'Q2', draft: 'B chunk', sourced: {} };
      }
    );
    await send({ kind: 'answerAssist', question: 'Q2', searchWeb: false });

    // A's late chunk arrives after B already owns the buffer — must be dropped.
    chunkA?.('A late chunk');

    // A finally settles — its terminal write must not clobber B's buffer.
    resolveA?.({ ok: true, question: 'Q1', draft: 'A full answer', sourced: {} });
    await runA;

    const progress = await send({ kind: 'answerAssistProgress' });
    expect(progress).toEqual({
      ok: true,
      kind: 'answerAssistProgress',
      text: 'B chunk',
      done: true,
      interrupted: false,
      rowId: '',
    });
  });

  // Narrower variant of the race above: run A is held BEFORE it ever resets
  // the buffer (its own `getToken()` await still pending) while run B starts
  // AND fully completes a whole round trip (reset -> chunk -> terminal
  // done:true). When A's await finally resolves, A must recognize it has
  // been superseded and bail out WITHOUT resetting the buffer B just
  // finished and WITHOUT ever calling the billable streaming client a
  // second time.
  it('a run superseded before its own reset never resets the buffer or calls the streaming client', async () => {
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);

    let resolveTokenA: ((value: string) => void) | undefined;
    const pendingTokenA = new Promise<string>((resolve) => {
      resolveTokenA = resolve;
    });
    getTokenMock.mockReturnValueOnce(pendingTokenA); // run A's getToken()
    getTokenMock.mockResolvedValue(FAKE_TOKEN); // run B's (and any later) getToken()

    // Start run A but don't await it — its getToken() await stays pending.
    const runA = send({ kind: 'answerAssist', question: 'Q1', searchWeb: false });

    // Run B starts and fully completes while A is still stuck before its
    // own reset.
    mockClient.answerAssist.mockImplementationOnce(
      async (_payload, onChunk?: (d: string) => void) => {
        onChunk?.('B chunk');
        return { ok: true, question: 'Q2', draft: 'B chunk', sourced: {} };
      }
    );
    await send({ kind: 'answerAssist', question: 'Q2', searchWeb: false });

    const afterB = await send({ kind: 'answerAssistProgress' });
    expect(afterB).toEqual({
      ok: true,
      kind: 'answerAssistProgress',
      text: 'B chunk',
      done: true,
      interrupted: false,
      rowId: '',
    });

    // A's getToken() finally resolves — A must bail out as superseded before
    // resetting the buffer, and must never call the streaming client again.
    resolveTokenA?.(FAKE_TOKEN);
    const resA = await runA;

    expect(resA).toEqual({ ok: false, error: 'Superseded by a newer request.' });
    expect(mockClient.answerAssist).toHaveBeenCalledTimes(1); // only B's call
    expect(mockClient.answerAssist).toHaveBeenCalledWith(
      expect.objectContaining({ question: 'Q2' }),
      expect.any(Function)
    );

    const finalProgress = await send({ kind: 'answerAssistProgress' });
    expect(finalProgress).toEqual({
      ok: true,
      kind: 'answerAssistProgress',
      text: 'B chunk',
      done: true,
      interrupted: false,
      rowId: '',
    });
  });
});

// ── settleRowFromAssist — the "unchanged rewrite" no-op guard (Finding 4) ───

describe('settleRowFromAssist — a chip rewrite that comes back unchanged', () => {
  /** Scan one row, then draft it once so there is a version a rewrite can
   *  reshape (`runAnswerRowAssist` refuses a rewrite with nothing to reshape
   *  yet), returning its id. */
  async function scanAndDraft(tabId: number, draftText: string): Promise<string> {
    tabsQueryMock.mockResolvedValue([
      { id: tabId, url: `https://jobs.example.com/posting/${tabId}` } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([
      { result: { questions: [{ question: 'Why this role?', index: 0 }], filled: [] } },
    ] as never);
    mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'not paired' });
    const scanned = await send({ kind: 'answerScan' });
    if (!scanned.ok || scanned.kind !== 'answerState' || !scanned.state) {
      throw new Error('expected an answerState response');
    }
    const rowId = scanned.state.rows[0]?.id;
    if (!rowId) throw new Error('expected a row');

    mockClient.answerAssist.mockResolvedValueOnce({
      ok: true,
      question: 'Why this role?',
      draft: draftText,
      sourced: {},
    });
    await send({
      kind: 'answerAssist',
      question: 'Why this role?',
      searchWeb: false,
      mode: 'draft',
      rowId,
    });
    return rowId;
  }

  /** Read the row list back without disturbing selection — `version: 0` is a
   *  no-op re-select of the version the draft above already selected. */
  async function readRows(rowId: string): Promise<AnswerRow[]> {
    const res = await send({ kind: 'answerSelectVersion', rowId, version: 0 });
    if (!res.ok || res.kind !== 'answerState' || !res.state) {
      throw new Error('expected an answerState response');
    }
    return res.state.rows;
  }

  it('does not append a new version, and sets a neutral (non-error) row notice', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    const rowId = await scanAndDraft(310, 'Led the migration and shipped the payment service.');

    // "Shorten" comes back with only a trailing comma added — the measured
    // no-op shape (ADR-044 / the desktop's F3 twin).
    mockClient.answerAssist.mockResolvedValueOnce({
      ok: true,
      question: 'Why this role?',
      draft: 'Led the migration and shipped the payment service,',
      sourced: {},
    });
    await send({
      kind: 'answerAssist',
      question: 'Why this role?',
      searchWeb: false,
      mode: 'rewrite',
      preset: 'shorten',
      rowId,
    });

    const rows = await readRows(rowId);
    // Mutation guard: without the unchanged-rewrite check this grows to 2 and
    // `notice` stays undefined — REVERT `settleRowFromAssist`'s rewrite branch
    // and this assertion is what catches it.
    expect(rows[0]?.versions).toHaveLength(1);
    expect(rows[0]?.notice).toMatch(/came back the same/);
    expect(rows[0]?.error).toBeUndefined();
  });

  it('still appends a genuinely different rewrite, and clears any stale notice', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    const rowId = await scanAndDraft(311, 'Led the migration and shipped the payment service.');

    mockClient.answerAssist.mockResolvedValueOnce({
      ok: true,
      question: 'Why this role?',
      draft: 'Led migration; shipped payments.',
      sourced: {},
    });
    await send({
      kind: 'answerAssist',
      question: 'Why this role?',
      searchWeb: false,
      mode: 'rewrite',
      preset: 'shorten',
      rowId,
    });

    const rows = await readRows(rowId);
    expect(rows[0]?.versions).toHaveLength(2);
    expect(rows[0]?.notice).toBeUndefined();
  });
});

// ── updateAnswerState — the lost-update race between the terminal stream
// mirror (broadcastAssistProgress → mirrorAssistToState) and settling the row
// (settleRowFromAssist), both of which read-modify-write the SAME tab's
// storage.session record with no serialization before answer-state.ts's
// per-tab write queue (pr-reviewer CRITICAL 2). ───────────────────────────────

describe('the terminal stream-mirror write never clobbers the settled row (CRITICAL 2)', () => {
  it('a two-chunk draft ends with BOTH stream.done:true AND the drafted version appended, and the settled row renders with its controls enabled', async () => {
    const { readAnswerState } = await import('./lib/answer-state');
    const { mountAnswerTools } = await import('./answer-tools/answer-tools');

    const tabId = 900;
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: tabId, url: `https://jobs.example.com/posting/${tabId}` } as never,
    ]);

    // Scan the page so there is a row to draft.
    executeScriptMock.mockResolvedValueOnce([
      { result: { questions: [{ question: 'Why this role?', index: 0 }], filled: [] } },
    ] as never);
    mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'not paired' });
    const scanned = await send({ kind: 'answerScan' });
    if (!scanned.ok || scanned.kind !== 'answerState' || !scanned.state) {
      throw new Error('expected an answerState response');
    }
    const rowId = scanned.state.rows[0]?.id;
    if (!rowId) throw new Error('expected a row');

    // Gate `storage.session.get` deterministically on the TERMINAL stream
    // mirror's own read — recognized by its CONTENT, not by which call number
    // it happens to be: the stored record's `stream.text` already equals the
    // full accumulated draft, but `stream.done` is still `false` (the
    // terminal write that flips it to `true` hasn't landed yet). Any OTHER
    // read (the initial reset, an in-progress chunk, settle's own read after
    // `done` flips) fails this check, so an unrelated read added anywhere in
    // the flow can never shift which one gets gated. Capturing the stored
    // value at CALL TIME (via the real mock implementation) but resolving the
    // returned promise only once this test releases `gate` is what makes the
    // race deterministic: under the OLD unserialized `updateAnswerState`,
    // settle's read+mutate+write (ungated) runs to completion while this call
    // is still pending, and this call's STALE snapshot then clobbers settle's
    // write the moment it is released.
    const FULL_DRAFT = 'Because I like solving real problems.';
    const sessionGetMock = vi.mocked(browser.storage.session.get);
    const realGet = sessionGetMock.getMockImplementation();
    if (!realGet) throw new Error('expected the default storage.session.get mock');
    let gated = false;
    let releaseGate: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      releaseGate = resolve;
    });
    sessionGetMock.mockImplementation(((key: string) => {
      const read = realGet(key) as Promise<Record<string, unknown>>;
      return read.then((value) => {
        const state = value[key] as { stream?: { done?: boolean; text?: string } } | undefined;
        if (!gated && state?.stream?.done === false && state.stream.text === FULL_DRAFT) {
          gated = true;
          return gate.then(() => value);
        }
        return value;
      });
    }) as typeof realGet);

    mockClient.answerAssist.mockImplementationOnce(
      async (_payload, onChunk?: (d: string) => void) => {
        onChunk?.('Because I ');
        onChunk?.('like solving real problems.');
        return {
          ok: true,
          question: 'Why this role?',
          draft: 'Because I like solving real problems.',
          sourced: {},
        };
      }
    );

    const assistDone = send({
      kind: 'answerAssist',
      question: 'Why this role?',
      searchWeb: false,
      mode: 'draft',
      rowId,
    });

    // Give settle's own (ungated) read+mutate+write a chance to run to
    // completion (under the OLD code) while the terminal mirror's read is
    // still stuck on `gate`, then release it and let everything settle.
    await flush();
    releaseGate?.();
    await assistDone;
    await flush();
    sessionGetMock.mockImplementation(realGet);

    const settled = await readAnswerState(tabId);
    if (!settled) throw new Error('expected a settled answer state');
    // Mutation guard: revert `updateAnswerState`'s per-tab queue (call
    // `readAnswerState`/`writeAnswerState` directly again) and EITHER of
    // these goes red, depending on which write lands last.
    expect(settled.stream?.done).toBe(true);
    expect(settled.rows.find((r) => r.id === rowId)?.versions).toHaveLength(1);

    // Close the loop into the UI (the same shared component both surfaces
    // mount): a lost `stream.done` combined with Finding 5's streaming gate
    // (`streaming = state.stream?.rowId === row.id && !state.stream.done`)
    // permanently disables every control on this row, on a FRESH mount, with
    // no escape on a single-question form.
    const host = document.createElement('div');
    document.body.append(host);
    const view = mountAnswerTools(host, { send, copy: vi.fn(async () => true) });
    view.render(settled);
    host.querySelector<HTMLButtonElement>('.arow__head')?.click();
    const regenerate = [...host.querySelectorAll<HTMLButtonElement>('.btn')].find(
      (b) => b.textContent === 'Regenerate'
    );
    expect(regenerate).not.toBeUndefined();
    expect(regenerate?.disabled).toBe(false);
  });
});

describe('a late activeTabId() must not re-arm a superseded run (regression)', () => {
  it('a run whose activeTabId() resolves AFTER a newer run already reset the buffer neither overwrites it nor fires its own billable request', async () => {
    const tabId = 851;
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    const tabInfo = [{ id: tabId, url: 'https://jobs.example.com/posting/851' } as never];
    tabsQueryMock.mockResolvedValue(tabInfo);

    executeScriptMock.mockResolvedValueOnce([
      {
        result: {
          questions: [
            { question: 'Why this role?', index: 0 },
            { question: 'What motivates you?', index: 0 },
          ],
          filled: [],
        },
      },
    ] as never);
    mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'not paired' });
    const scanned = await send({ kind: 'answerScan' });
    if (!scanned.ok || scanned.kind !== 'answerState' || !scanned.state) {
      throw new Error('expected an answerState response');
    }
    const rowA = scanned.state.rows[0]?.id;
    const rowB = scanned.state.rows[1]?.id;
    if (!rowA || !rowB) throw new Error('expected two rows');

    // A request naming a `rowId` resolves through `runAnswerRowAssist` first,
    // which makes its OWN `activeTabId()` call before ever reaching
    // `runAnswerAssist` — so run A's three `tabs.query` calls, in order, are
    // `runAnswerRowAssist`'s `activeTabId()`, `runAnswerAssist`'s
    // `activeTabUrl()`, then `runAnswerAssist`'s OWN `activeTabId()` (the
    // bug-relevant one). Gate that THIRD call; run B starts and finishes
    // entirely afterward, landing on counts 4-6, never gated.
    let queryCalls = 0;
    let releaseGate: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      releaseGate = resolve;
    });
    tabsQueryMock.mockImplementation(() => {
      queryCalls += 1;
      return queryCalls === 3 ? gate.then(() => tabInfo) : Promise.resolve(tabInfo);
    });

    mockClient.answerAssist.mockImplementation(async (_payload, onChunk?: (d: string) => void) => {
      onChunk?.('drafted');
      return { ok: true, question: 'x', draft: 'drafted', sourced: {} };
    });

    // Run A: gets past the gen check with `assistTabId` still unresolved.
    const runA = send({
      kind: 'answerAssist',
      question: 'Why this role?',
      searchWeb: false,
      mode: 'draft',
      rowId: rowA,
    });
    await flush();

    // Run B starts (bumps `assistGeneration`) and runs to completion while
    // run A is still suspended on the gate above.
    const runB = await send({
      kind: 'answerAssist',
      question: 'What motivates you?',
      searchWeb: false,
      mode: 'draft',
      rowId: rowB,
    });
    expect(runB.ok).toBe(true);

    // Release run A's gate. Under the bug, A would resume believing it is
    // still current, clobber the buffer B just wrote, and fire its OWN
    // billable request — the exact thing the generation guard exists to stop.
    releaseGate?.();
    const resultA = await runA;
    expect(resultA).toEqual({ ok: false, error: 'Superseded by a newer request.' });
    expect(mockClient.answerAssist).toHaveBeenCalledTimes(1);

    const { readAnswerState } = await import('./lib/answer-state');
    const settled = await readAnswerState(tabId);
    expect(settled?.stream?.rowId).toBe(rowB);
    expect(settled?.rows.find((r) => r.id === rowB)?.versions).toHaveLength(1);
    expect(settled?.rows.find((r) => r.id === rowA)?.versions).toHaveLength(0);

    tabsQueryMock.mockResolvedValue(tabInfo);
  });
});

// ── answerFill request — per-row fill, NEVER a different field ─────────────

describe('answerFill request — not-paired short-circuit', () => {
  it('surfaces "Not paired" and never reaches executeScript when no token is stored', async () => {
    getTokenMock.mockResolvedValue(null);

    const res = await send({
      kind: 'answerFill',
      question: 'Why this role?',
      index: 0,
      count: 1,
      answer: 'Because I love it.',
    });

    expect(res).toEqual({ ok: false, error: 'Not paired. Paste your pairing token first.' });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });
});

describe('answerFill request', () => {
  it('injects answer-fill.js then invokes it with the correlation + answer, returning the outcome', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{}] as never); // files-only registration step
    executeScriptMock.mockResolvedValueOnce([{ result: { filled: true } }] as never);

    const res = await send({
      kind: 'answerFill',
      question: 'Why this role?',
      index: 0,
      count: 1,
      answer: 'Because I love it.',
    });

    expect(executeScriptMock).toHaveBeenNthCalledWith(1, {
      target: { tabId: 7 },
      files: ['answer-fill.js'],
    });
    expect(executeScriptMock).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        target: { tabId: 7 },
        args: ['Why this role?', 0, 1, 'Because I love it.', '__ajhRunAnswerFill'],
      })
    );
    expect(res).toEqual({ ok: true, kind: 'answerFill', result: { filled: true } });
  });

  it('surfaces the fail-safe not-found result straight through — never a different field', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{}] as never);
    executeScriptMock.mockResolvedValueOnce([
      {
        result: { filled: false, error: 'Could not find this field — the page may have changed.' },
      },
    ] as never);

    const res = await send({
      kind: 'answerFill',
      question: 'Why this role?',
      index: 0,
      count: 1,
      answer: 'Because I love it.',
    });

    expect(res).toEqual({
      ok: true,
      kind: 'answerFill',
      result: { filled: false, error: 'Could not find this field — the page may have changed.' },
    });
  });

  it('surfaces "Could not fill this field." when the injected script returns a malformed result', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{}] as never);
    executeScriptMock.mockResolvedValueOnce([{ result: null }] as never);

    const res = await send({
      kind: 'answerFill',
      question: 'Why this role?',
      index: 0,
      count: 1,
      answer: 'Because I love it.',
    });

    expect(res).toEqual({ ok: false, error: 'Could not fill this field.' });
  });
});

// ── answerReplace request — rewrite Accept/Restore, NEVER a different field ─

describe('answerReplace request — not-paired short-circuit', () => {
  it('surfaces "Not paired" and never reaches executeScript when no token is stored', async () => {
    getTokenMock.mockResolvedValue(null);

    const res = await send({
      kind: 'answerReplace',
      question: 'Why this role?',
      index: 0,
      count: 1,
      text: 'A rewritten answer.',
      expectedValue: 'Because I like it.',
    });

    expect(res).toEqual({ ok: false, error: 'Not paired. Paste your pairing token first.' });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });
});

describe('answerReplace request', () => {
  it('injects answer-replace.js then invokes it with the correlation + text + expectedValue, returning the outcome', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{}] as never); // files-only registration step
    executeScriptMock.mockResolvedValueOnce([{ result: { filled: true } }] as never);

    const res = await send({
      kind: 'answerReplace',
      question: 'Why this role?',
      index: 0,
      count: 1,
      text: 'A rewritten answer.',
      expectedValue: 'Because I like it.',
    });

    expect(executeScriptMock).toHaveBeenNthCalledWith(1, {
      target: { tabId: 7 },
      files: ['answer-replace.js'],
    });
    expect(executeScriptMock).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        target: { tabId: 7 },
        args: [
          'Why this role?',
          0,
          1,
          'A rewritten answer.',
          'Because I like it.',
          '__ajhRunAnswerReplace',
        ],
      })
    );
    expect(res).toEqual({ ok: true, kind: 'answerReplace', result: { filled: true } });
  });

  it('surfaces the fail-safe not-found result straight through — never a different field', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{}] as never);
    executeScriptMock.mockResolvedValueOnce([
      {
        result: { filled: false, error: 'Could not find this field — the page may have changed.' },
      },
    ] as never);

    const res = await send({
      kind: 'answerReplace',
      question: 'Why this role?',
      index: 0,
      count: 1,
      text: 'A rewritten answer.',
      expectedValue: 'Because I like it.',
    });

    expect(res).toEqual({
      ok: true,
      kind: 'answerReplace',
      result: { filled: false, error: 'Could not find this field — the page may have changed.' },
    });
  });

  it('surfaces the changed-since-pick refusal straight through — never overwrites a manual edit', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{}] as never);
    executeScriptMock.mockResolvedValueOnce([
      {
        result: {
          filled: false,
          error: 'This field changed since you picked it — re-pick it to rewrite.',
        },
      },
    ] as never);

    const res = await send({
      kind: 'answerReplace',
      question: 'Why this role?',
      index: 0,
      count: 1,
      text: 'A rewritten answer.',
      expectedValue: 'Because I like it.',
    });

    expect(res).toEqual({
      ok: true,
      kind: 'answerReplace',
      result: {
        filled: false,
        error: 'This field changed since you picked it — re-pick it to rewrite.',
      },
    });
  });

  it('surfaces "Could not replace this field." when the injected script returns a malformed result', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 7, url: 'https://jobs.example.com/posting/9' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{}] as never);
    executeScriptMock.mockResolvedValueOnce([{ result: null }] as never);

    const res = await send({
      kind: 'answerReplace',
      question: 'Why this role?',
      index: 0,
      count: 1,
      text: 'A rewritten answer.',
      expectedValue: 'Because I like it.',
    });

    expect(res).toEqual({ ok: false, error: 'Could not replace this field.' });
  });
});

// ── ADR-044: the shared per-(tab, origin) answer state, driven entirely
// through the popup-request dispatcher — scan, free-text add, version select,
// Accept/Restore, and the context-menu entry that opens the panel. Each test
// picks its OWN tabId (the mocked `storage.session` area is a module-level
// store, never reset between tests) so no test can read another's state. ──

describe('answerScan request (ADR-044)', () => {
  it('injects capture-rows.js, captures the origin at gesture time, and writes the built state', async () => {
    tabsQueryMock.mockResolvedValue([
      { id: 200, url: 'https://jobs.example.com/posting/1' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([
      {
        result: {
          questions: [{ question: 'Why this role?', index: 0 }],
          filled: [{ question: 'Company name', index: 0, answer: 'Acme' }],
        },
      },
    ] as never);
    mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'not paired' });

    const res = await send({ kind: 'answerScan' });

    expect(executeScriptMock).toHaveBeenCalledWith({
      target: { tabId: 200 },
      files: ['capture-rows.js'],
    });
    expect(res.ok).toBe(true);
    if (res.ok && res.kind === 'answerState') {
      expect(res.state?.tabId).toBe(200);
      expect(res.state?.origin).toBe('https://jobs.example.com');
      expect(res.state?.pageChanged).toBe(false);
      expect(res.state?.rows.map((r) => r.question)).toEqual(['Why this role?', 'Company name']);
    } else {
      throw new Error('expected an answerState response');
    }
  });

  it('surfaces "Could not read the questions on this page." when the injected script returns a non-scan value', async () => {
    tabsQueryMock.mockResolvedValue([
      { id: 201, url: 'https://jobs.example.com/posting/2' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([{ result: null }] as never);

    const res = await send({ kind: 'answerScan' });

    expect(res).toEqual({ ok: false, error: 'Could not read the questions on this page.' });
  });
});

describe('answerAddRow request (ADR-044)', () => {
  it('creates a fresh state (unscanned page) carrying only the free-text row', async () => {
    tabsQueryMock.mockResolvedValue([
      { id: 202, url: 'https://jobs.example.com/posting/3' } as never,
    ]);

    const res = await send({ kind: 'answerAddRow', question: 'What is your visa status?' });

    expect(res.ok).toBe(true);
    if (res.ok && res.kind === 'answerState') {
      expect(res.state?.rows).toHaveLength(1);
      expect(res.state?.rows[0]).toMatchObject({
        question: 'What is your visa status?',
        field: null,
        status: 'empty',
      });
    } else {
      throw new Error('expected an answerState response');
    }
  });

  it('prepends onto an existing scan rather than replacing it, and reuses the row on a repeated question', async () => {
    tabsQueryMock.mockResolvedValue([
      { id: 203, url: 'https://jobs.example.com/posting/4' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([
      { result: { questions: [{ question: 'Why this role?', index: 0 }], filled: [] } },
    ] as never);
    mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'not paired' });
    await send({ kind: 'answerScan' });

    const first = await send({ kind: 'answerAddRow', question: 'A question the scan missed' });
    const second = await send({ kind: 'answerAddRow', question: 'A question the scan missed' });

    if (first.ok && first.kind === 'answerState' && second.ok && second.kind === 'answerState') {
      expect(first.state?.rows.map((r) => r.question)).toEqual([
        'A question the scan missed',
        'Why this role?',
      ]);
      // Same question added twice reuses the row rather than stacking a duplicate.
      expect(second.state?.rows.map((r) => r.question)).toEqual([
        'A question the scan missed',
        'Why this role?',
      ]);
    } else {
      throw new Error('expected two answerState responses');
    }
  });
});

describe('answerSelectVersion request (ADR-044)', () => {
  it('selects a version by index, and falls back to -1 (the page text) for an out-of-range index', async () => {
    const { updateAnswerState } = await import('./lib/answer-state');
    tabsQueryMock.mockResolvedValue([
      { id: 204, url: 'https://jobs.example.com/posting/5' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([
      { result: { questions: [{ question: 'Why this role?', index: 0 }], filled: [] } },
    ] as never);
    mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'not paired' });
    const scanned = await send({ kind: 'answerScan' });
    if (!scanned.ok || scanned.kind !== 'answerState' || !scanned.state) {
      throw new Error('expected an answerState response');
    }
    const rowId = scanned.state.rows[0]?.id;
    if (!rowId) throw new Error('expected a row');

    // Seed a version to select — a fresh row has none, so `version: 0` would
    // ALSO fall back to -1, and the test would never actually exercise the
    // in-range branch it claims to cover.
    await updateAnswerState(204, (state) => ({
      ...state,
      rows: state.rows.map((row) =>
        row.id === rowId
          ? { ...row, versions: [{ label: 'v1', text: 'A drafted answer.', kind: 'draft' }] }
          : row
      ),
    }));

    const inRange = await send({ kind: 'answerSelectVersion', rowId, version: 0 });
    if (inRange.ok && inRange.kind === 'answerState') {
      expect(inRange.state?.rows[0]?.selected).toBe(0);
    } else {
      throw new Error('expected an answerState response');
    }

    const outOfRange = await send({ kind: 'answerSelectVersion', rowId, version: 5 });
    if (outOfRange.ok && outOfRange.kind === 'answerState') {
      expect(outOfRange.state?.rows[0]?.selected).toBe(-1);
    } else {
      throw new Error('expected an answerState response');
    }
  });
});

describe('answerAccept / answerRestoreOriginal requests (ADR-044)', () => {
  it('writes the selected text into an EMPTY field via answer-fill.js and remembers it as currentText', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 205, url: 'https://jobs.example.com/posting/6' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([
      { result: { questions: [{ question: 'Why this role?', index: 0 }], filled: [] } },
    ] as never);
    mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'not paired' });
    const scanned = await send({ kind: 'answerScan' });
    if (!scanned.ok || scanned.kind !== 'answerState' || !scanned.state) {
      throw new Error('expected an answerState response');
    }
    const rowId = scanned.state.rows[0]?.id;
    if (!rowId) throw new Error('expected a row');

    executeScriptMock.mockResolvedValueOnce([{}] as never); // answer-fill.js registration
    executeScriptMock.mockResolvedValueOnce([{ result: { filled: true } }] as never);

    // A freshly-scanned row has no drafted version yet, so Restore (which
    // always has text — the frozen scan-time original, `''` for an empty
    // field) is what exercises `writeRowText`'s fail-safe write path here;
    // Accept goes through the identical function with a different source text.
    const res = await send({ kind: 'answerRestoreOriginal', rowId });

    // Call 1 was the scan's own capture-rows.js injection; 2 and 3 are this write.
    expect(executeScriptMock).toHaveBeenNthCalledWith(2, {
      target: { tabId: 205 },
      files: ['answer-fill.js'],
    });
    expect(executeScriptMock).toHaveBeenNthCalledWith(
      3,
      expect.objectContaining({
        target: { tabId: 205 },
        args: ['Why this role?', 0, 1, '', '__ajhRunAnswerFill'],
      })
    );
    expect(res).toEqual({ ok: true, kind: 'answerAccept', result: { filled: true } });
  });

  it('refuses to write once the page has changed, without touching the tab at all', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    tabsQueryMock.mockResolvedValue([
      { id: 206, url: 'https://jobs.example.com/posting/7' } as never,
    ]);
    executeScriptMock.mockResolvedValueOnce([
      { result: { questions: [{ question: 'Why this role?', index: 0 }], filled: [] } },
    ] as never);
    mockClient.suggestAnswers.mockResolvedValue({ ok: false, error: 'not paired' });
    const scanned = await send({ kind: 'answerScan' });
    if (!scanned.ok || scanned.kind !== 'answerState' || !scanned.state) {
      throw new Error('expected an answerState response');
    }
    const rowId = scanned.state.rows[0]?.id;
    if (!rowId) throw new Error('expected a row');

    // A navigation flips pageChanged — the onUpdated listener does this in the
    // real worker; call the registered callback directly the same way the
    // onMessage listener is driven above.
    const onUpdated = vi.mocked(browser.tabs.onUpdated.addListener).mock.calls[0]?.[0];
    onUpdated?.(206, { status: 'loading' } as never, {} as never);
    await flush();

    executeScriptMock.mockClear();
    const res = await send({ kind: 'answerRestoreOriginal', rowId });

    expect(res).toEqual({
      ok: false,
      error: 'This page changed. Click the toolbar icon to scan it, then try again.',
    });
    expect(executeScriptMock).not.toHaveBeenCalled();
  });
});

describe('the context-menu entries (ADR-044 decision 2)', () => {
  it('registers both entries: the selection-only one and the plain-page one', () => {
    const onInstalled = vi.mocked(browser.runtime.onInstalled.addListener).mock.calls[0]?.[0];
    vi.mocked(browser.contextMenus.create).mockClear();

    onInstalled?.({} as never);

    expect(browser.contextMenus.create).toHaveBeenCalledTimes(2);
    expect(browser.contextMenus.create).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'ajh-answer-selection',
        title: 'Answer this with AI Job Hunter',
        contexts: ['selection'],
      })
    );
    expect(browser.contextMenus.create).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'ajh-answer-open-panel',
        title: 'Open AI Job Hunter answer tool',
        contexts: ['page', 'editable'],
      })
    );
  });

  it('adds the trimmed selection as a free-text row on click, keyed to the clicked tab', async () => {
    const onClicked = vi.mocked(browser.contextMenus.onClicked.addListener).mock.calls[0]?.[0];
    if (!onClicked) throw new Error('context-menu click listener not registered');
    tabsQueryMock.mockResolvedValue([
      { id: 207, url: 'https://jobs.example.com/posting/8' } as never,
    ]);

    onClicked(
      {
        menuItemId: 'ajh-answer-selection',
        selectionText: '  Describe a challenge you solved.  ',
      } as never,
      { id: 207 } as never
    );
    await flush();

    // Read the resulting state back by asking for one more row.
    const res = await send({ kind: 'answerAddRow', question: 'a second question' });
    if (res.ok && res.kind === 'answerState') {
      expect(res.state?.rows.map((r) => r.question)).toContain('Describe a challenge you solved.');
    } else {
      throw new Error('expected an answerState response');
    }
  });

  it('adds the row to the CLICKED tab even when a fresh active-tab query would resolve to a DIFFERENT tab (regression)', async () => {
    const onClicked = vi.mocked(browser.contextMenus.onClicked.addListener).mock.calls[0]?.[0];
    if (!onClicked) throw new Error('context-menu click listener not registered');
    const { readAnswerState } = await import('./lib/answer-state');

    const clickedTabId = 208;
    const otherActiveTabId = 209;
    // Whichever tab a FRESH `{active:true,currentWindow:true}` query would
    // resolve to right now is a DIFFERENT tab than the one the context-menu
    // event actually fired on (e.g. focus moved to another window between
    // the gesture and this call) — the row must still land on the CLICKED
    // tab, not on whatever `activeTabId()` would independently resolve.
    tabsQueryMock.mockResolvedValue([
      { id: otherActiveTabId, url: 'https://jobs.example.com/posting/9' } as never,
    ]);

    onClicked(
      {
        menuItemId: 'ajh-answer-selection',
        selectionText: 'Tell me about a conflict you resolved.',
      } as never,
      { id: clickedTabId } as never
    );
    await flush();

    const clickedState = await readAnswerState(clickedTabId);
    expect(clickedState?.rows.map((r) => r.question)).toContain(
      'Tell me about a conflict you resolved.'
    );

    const otherState = await readAnswerState(otherActiveTabId);
    expect(otherState?.rows.map((r) => r.question) ?? []).not.toContain(
      'Tell me about a conflict you resolved.'
    );
  });

  it('ignores a click on a different menu id', async () => {
    const onClicked = vi.mocked(browser.contextMenus.onClicked.addListener).mock.calls[0]?.[0];
    if (!onClicked) throw new Error('context-menu click listener not registered');
    tabsQueryMock.mockClear();

    onClicked({ menuItemId: 'some-other-entry', selectionText: 'ignored' } as never, {} as never);
    await flush();

    expect(tabsQueryMock).not.toHaveBeenCalled();
  });

  it('opens the panel and adds nothing on a plain-page click (no selection to prefill)', async () => {
    const onClicked = vi.mocked(browser.contextMenus.onClicked.addListener).mock.calls[0]?.[0];
    if (!onClicked) throw new Error('context-menu click listener not registered');
    const { readAnswerState } = await import('./lib/answer-state');
    const sidePanelOpenMock = vi.mocked(browser.sidePanel.open);
    sidePanelOpenMock.mockClear();
    tabsQueryMock.mockClear();

    const clickedTabId = 210;
    onClicked({ menuItemId: 'ajh-answer-open-panel' } as never, { id: clickedTabId } as never);
    await flush();

    expect(sidePanelOpenMock).toHaveBeenCalledWith({ tabId: clickedTabId });
    // Distinguishes this entry from the selection one: nothing gets added as
    // a row, and it never even reads the tab to figure out where to write one.
    expect(tabsQueryMock).not.toHaveBeenCalled();
    // `readAnswerState` returns `null` (not `undefined`) for a tab it has
    // never written — asserting `toHaveLength(0)` on `state?.rows ?? []`
    // would pass just as well if the read silently returned nothing at all,
    // so it can't tell "wrote zero rows" apart from "never wrote anything".
    const state = await readAnswerState(clickedTabId);
    expect(state).toBeNull();
  });

  it('falls back to sidebarAction.open() on Firefox, which has no sidePanel API', async () => {
    const onClicked = vi.mocked(browser.contextMenus.onClicked.addListener).mock.calls[0]?.[0];
    if (!onClicked) throw new Error('context-menu click listener not registered');
    // `openAnswerPanel` (background.ts) tries the Chrome `sidePanel` branch
    // first and falls back to Firefox's `sidebarAction` only when it is
    // absent — simulate that by removing `sidePanel` from the shared mock
    // for the duration of this one test.
    const chromeSidePanel = (browser as { sidePanel?: unknown }).sidePanel;
    const sidebarOpenMock = vi.fn().mockResolvedValue(undefined);
    (browser as { sidePanel?: unknown }).sidePanel = undefined;
    (browser as { sidebarAction?: { open: typeof sidebarOpenMock } }).sidebarAction = {
      open: sidebarOpenMock,
    };
    try {
      onClicked({ menuItemId: 'ajh-answer-open-panel' } as never, { id: 212 } as never);
      await flush();
      expect(sidebarOpenMock).toHaveBeenCalledTimes(1);
    } finally {
      (browser as { sidePanel?: unknown }).sidePanel = chromeSidePanel;
      delete (browser as { sidebarAction?: unknown }).sidebarAction;
    }
  });
});

// ── Task #22 review closures: SUBMIT_DETECTED_MSG parity, submitDetected
// routing, submit-watch arming on a gesture, and the getStatus badge clear ──

describe('SUBMIT_DETECTED_MSG parity (Task #22 review closure)', () => {
  it('the background.ts local literal matches the imported lib/submit-watch.ts const — a future edit to one side cannot silently break routing', () => {
    expect(backgroundModule.SUBMIT_DETECTED_MSG).toBe(SUBMIT_DETECTED_MSG);
  });
});

describe('submitDetected message — not a popup request (Task #22 review closure)', () => {
  it('returns undefined (no popup response channel) and routes to handleSubmitDetected, which auto-marks a tracked saved job applied when the opt-in is ON', async () => {
    mockClient.autotrackEnabled.mockResolvedValue(true);
    mockClient.checkApplied.mockResolvedValue({ found: true, status: 'saved' });
    mockClient.updateStatus.mockResolvedValue({
      ok: true,
      applicationId: 'app-1',
      status: 'applied',
    });

    const result = listener?.(
      { kind: SUBMIT_DETECTED_MSG, url: 'https://jobs.example.com/posting/9' },
      { id: EXTENSION_ID } as Browser.runtime.MessageSender
    );
    expect(result).toBeUndefined();

    await flush();

    expect(mockClient.checkApplied).toHaveBeenCalledWith('https://jobs.example.com/posting/9');
    expect(mockClient.updateStatus).toHaveBeenCalledWith(
      'https://jobs.example.com/posting/9',
      true
    );
  });

  it('is ignored when the sender is not this extension (belt-and-braces MV3 hygiene)', async () => {
    mockClient.autotrackEnabled.mockResolvedValue(true);
    mockClient.checkApplied.mockResolvedValue({ found: true, status: 'saved' });

    const result = listener?.(
      { kind: SUBMIT_DETECTED_MSG, url: 'https://jobs.example.com/posting/9' },
      { id: 'some-other-extension-id' } as Browser.runtime.MessageSender
    );
    expect(result).toBeUndefined();

    await flush();

    expect(mockClient.checkApplied).not.toHaveBeenCalled();
    expect(mockClient.updateStatus).not.toHaveBeenCalled();
  });
});

describe('arming the submit watcher after a gesture request (Task #22 review closure)', () => {
  it('a successful GESTURE_KINDS request (e.g. fill) injects submit-watch.js when the opt-in is ON', async () => {
    getTokenMock.mockResolvedValue(FAKE_TOKEN);
    mockClient.getProfile.mockResolvedValue({ email: 'saeed@example.com' });
    mockClient.autotrackEnabled.mockResolvedValue(true);
    tabsQueryMock.mockResolvedValue([{ id: 7, url: 'https://example.com/apply' } as never]);
    const summary: AutofillSummary = {
      filled: [{ key: 'email', label: 'Email', count: 1 }],
      nameSplit: null,
      filledNothing: false,
    };
    executeScriptMock
      .mockResolvedValueOnce([] as never) // fill.js registration
      .mockResolvedValueOnce([{ result: summary }] as never); // fill.js call

    await send({ kind: 'fill' });
    await flush(); // the arm is fire-and-forget — flush it before asserting

    expect(mockClient.autotrackEnabled).toHaveBeenCalled();
    expect(executeScriptMock).toHaveBeenCalledWith({
      target: { tabId: 7 },
      files: ['submit-watch.js'],
    });
  });

  it('a non-gesture request (getStatus) never arms the watcher', async () => {
    mockClient.autotrackEnabled.mockResolvedValue(true);

    await send({ kind: 'getStatus' });
    await flush();

    expect(mockClient.autotrackEnabled).not.toHaveBeenCalled();
    expect(executeScriptMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ files: ['submit-watch.js'] })
    );
  });

  it('a fieldsProbe request never arms the watcher (a passive scan, not a user gesture)', async () => {
    mockClient.autotrackEnabled.mockResolvedValue(true);
    tabsQueryMock.mockResolvedValue([{ id: 7, url: 'https://example.com/apply' } as never]);
    executeScriptMock.mockResolvedValueOnce([
      { result: { hasFormFields: true, hasAnswerFields: true } },
    ] as never);

    await send({ kind: 'fieldsProbe' });
    await flush();

    expect(mockClient.autotrackEnabled).not.toHaveBeenCalled();
    expect(executeScriptMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ files: ['submit-watch.js'] })
    );
  });
});

describe('getStatus clears the import/badge prompt (Task #22 review closure)', () => {
  it('clears the action badge set by a prior untracked-submit nudge', async () => {
    await send({ kind: 'getStatus' });

    expect(setBadgeTextMock).toHaveBeenCalledWith({ text: '' });
  });
});
