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
    tabs: { query: vi.fn() },
    scripting: { executeScript: vi.fn() },
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
      sender: Browser.runtime.MessageSender
    ) => Promise<PopupResponse> | undefined)
  | undefined;

function send(req: PopupRequest): Promise<PopupResponse> {
  if (!listener) throw new Error('onMessage listener not registered');
  return listener(req, {
    id: EXTENSION_ID,
  } as Browser.runtime.MessageSender) as Promise<PopupResponse>;
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
    });

    const progress = await send({ kind: 'answerAssistProgress' });
    expect(progress).toEqual({
      ok: true,
      kind: 'answerAssistProgress',
      text: 'Because I am drawn to it.',
      done: true,
      interrupted: false,
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
    });
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
