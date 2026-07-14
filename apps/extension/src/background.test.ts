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
import { browser } from '@wxt-dev/browser';

import type { AutofillSummary } from './lib/autofill';
import type { PopupRequest, PopupResponse } from './lib/messages';
import { getToken } from './lib/storage';

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
}));

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    runtime: {
      onMessage: { addListener: vi.fn() },
      onStartup: { addListener: vi.fn() },
      onInstalled: { addListener: vi.fn() },
      onUpdateAvailable: { addListener: vi.fn() },
      sendMessage: vi.fn(),
      reload: vi.fn(),
    },
    tabs: { query: vi.fn() },
    scripting: { executeScript: vi.fn() },
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
await import('./background');

const tabsQueryMock = vi.mocked(browser.tabs.query);
const executeScriptMock = vi.mocked(browser.scripting.executeScript);
const getTokenMock = vi.mocked(getToken);

/** The `handleRequest`-wrapping callback background.ts registered at module load. */
const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
  ((message: unknown) => Promise<PopupResponse>) | undefined;

function send(req: PopupRequest): Promise<PopupResponse> {
  if (!listener) throw new Error('onMessage listener not registered');
  return listener(req);
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
