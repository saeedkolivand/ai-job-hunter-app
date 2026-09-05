/**
 * Unit tests for the connection-status component: `resolveStatusResponse`
 * (moved here from popup.test.ts unchanged in behavior) plus the mounted
 * component's pill/retry/pairing/offline/outdated/searching behavior — driven
 * with bare `<div>` hosts and a mocked `send`/`browser`, without going
 * through either popup.ts or sidepanel.ts.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import type { ConnectionStatus, PopupRequest, PopupResponse } from '../lib/messages';
import { looksLikeToken } from '../lib/storage';

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    tabs: { create: vi.fn() },
    runtime: { onMessage: { addListener: vi.fn() } },
  },
}));

vi.mock('../lib/storage', () => ({
  looksLikeToken: vi.fn(() => false),
}));

import {
  type ConnectionStatusDeps,
  mountConnectionStatus,
  resolveStatusResponse,
} from './connection-status';

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

function status(phase: ConnectionStatus['phase'], hasToken = true): ConnectionStatus {
  return { phase, port: null, hasToken };
}

function mount(deps: Partial<ConnectionStatusDeps> = {}) {
  const pillHost = document.createElement('div');
  const viewsHost = document.createElement('div');
  const send = deps.send ?? vi.fn<[PopupRequest], Promise<PopupResponse>>();
  const view = mountConnectionStatus(pillHost, viewsHost, { send, ...deps });
  return { pillHost, viewsHost, send: send as ReturnType<typeof vi.fn>, view };
}

const byId = <T extends HTMLElement>(host: HTMLElement, id: string) =>
  host.querySelector<T>(`#${id}`)!;

// ── resolveStatusResponse ────────────────────────────────────────────────

describe('resolveStatusResponse', () => {
  it('returns the status when response is ok with kind=status', () => {
    const s = { phase: 'connected' as const, port: 47615, hasToken: true };
    const res = { ok: true as const, kind: 'status' as const, status: s };
    expect(resolveStatusResponse(res, false)).toEqual(s);
  });

  it('returns an app_not_running offline fallback when ok=false', () => {
    const res = { ok: false as const, error: 'Service worker not responding.' };
    const result = resolveStatusResponse(res, true);
    expect(result.phase).toBe('app_not_running');
    expect(result.hasToken).toBe(true);
    expect(result.port).toBeNull();
  });

  it('returns an app_not_running offline fallback for an unexpected ok kind', () => {
    const res = { ok: true as const, kind: 'token' as const };
    const result = resolveStatusResponse(res, false);
    expect(result.phase).toBe('app_not_running');
    expect(result.hasToken).toBe(false);
  });
});

// ── DOM built by mountConnectionStatus ──────────────────────────────────────

describe('mountConnectionStatus DOM', () => {
  it('builds the retry button + pill into pillHost and the four views into viewsHost', () => {
    const { pillHost, viewsHost } = mount();
    expect(byId(pillHost, 'btn-retry')).toBeTruthy();
    expect(byId(pillHost, 'status-pill')).toBeTruthy();
    for (const id of ['view-pair', 'view-offline', 'view-outdated', 'view-searching']) {
      expect(byId(viewsHost, id)).toBeTruthy();
    }
  });

  it('retry starts hidden and the pill starts at the searching label', () => {
    const { pillHost } = mount();
    expect(byId<HTMLButtonElement>(pillHost, 'btn-retry').hidden).toBe(true);
    expect(byId(pillHost, 'status-pill').textContent).toBe('○ Connecting…');
  });
});

// ── start() — first fetch + live push ───────────────────────────────────────

describe('start()', () => {
  it('fetches getStatus and renders the result', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValueOnce({ ok: true, kind: 'status', status: status('connected') });
    const { pillHost, view } = mount({ send });

    view.start();
    await flush();

    expect(send).toHaveBeenCalledWith({ kind: 'getStatus' });
    expect(byId(pillHost, 'status-pill').textContent).toBe('● Connected');
  });

  it('registers a live-push listener that re-renders on a pushed status', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValue({ ok: true, kind: 'status', status: status('not_paired') });
    const { pillHost, view } = mount({ send });

    view.start();
    await flush();
    expect(byId(pillHost, 'status-pill').textContent).toBe('⚠ Not paired');

    const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls.at(-1)?.[0];
    if (!listener) throw new Error('onMessage listener not registered');
    listener({ ok: true, kind: 'status', status: status('connected') });

    expect(byId(pillHost, 'status-pill').textContent).toBe('● Connected');
  });

  it('ignores a pushed message of a different kind — no cross-talk with other broadcasts', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValue({ ok: true, kind: 'status', status: status('not_paired') });
    const { pillHost, view } = mount({ send });

    view.start();
    await flush();
    expect(byId(pillHost, 'status-pill').textContent).toBe('⚠ Not paired');

    const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls.at(-1)?.[0];
    if (!listener) throw new Error('onMessage listener not registered');
    // A different broadcast kind sharing the same `runtime.onMessage` surface
    // (e.g. an in-flight answer-assist stream chunk) must not be mistaken for
    // a status push.
    listener({
      ok: true,
      kind: 'answerAssistProgress',
      text: 'x',
      done: false,
      interrupted: false,
      rowId: '',
    });

    expect(byId(pillHost, 'status-pill').textContent).toBe('⚠ Not paired');
  });

  it('falls back to the offline/Retry view when send() REJECTS (not just times out) — MV3 worker asleep/crashed', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockRejectedValueOnce(
        new Error('Could not establish connection. Receiving end does not exist.')
      );
    const { pillHost, viewsHost, view } = mount({ send });

    view.start();
    await flush();

    expect(byId(pillHost, 'status-pill').textContent).toBe('✕ App not running');
    expect(byId(viewsHost, 'view-offline').hidden).toBe(false);
    expect(byId<HTMLButtonElement>(pillHost, 'btn-retry').hidden).toBe(false);
  });
});

// ── refresh() rejection ──────────────────────────────────────────────────────

describe('refresh() rejection', () => {
  it('falls back to the offline/Retry view instead of throwing when send() rejects', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockRejectedValueOnce(new Error('message channel closed'));
    const { pillHost, viewsHost, view } = mount({ send });

    await expect(view.refresh()).resolves.toBeUndefined();

    expect(byId(pillHost, 'status-pill').textContent).toBe('✕ App not running');
    expect(byId(viewsHost, 'view-offline').hidden).toBe(false);
  });
});

// ── header Retry visibility ─────────────────────────────────────────────────

describe('header Retry visibility', () => {
  it('is shown only for app_not_running and outdated', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValue({ ok: true, kind: 'status', status: status('searching') });
    const { pillHost, view } = mount({ send });
    view.start();
    await flush();
    const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls.at(-1)?.[0];
    if (!listener) throw new Error('listener not registered');
    const push = (phase: ConnectionStatus['phase']) =>
      listener({ ok: true, kind: 'status', status: status(phase) });
    const retry = byId<HTMLButtonElement>(pillHost, 'btn-retry');

    push('app_not_running');
    expect(retry.hidden).toBe(false);
    push('outdated');
    expect(retry.hidden).toBe(false);
    push('connected');
    expect(retry.hidden).toBe(true);
    push('searching');
    expect(retry.hidden).toBe(true);
    push('not_paired');
    expect(retry.hidden).toBe(true);
  });

  it('clicking retry sends reconnect then re-fetches status', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValueOnce({ ok: true, kind: 'status', status: status('app_not_running') })
      .mockResolvedValueOnce({ ok: true, kind: 'token' })
      .mockResolvedValueOnce({ ok: true, kind: 'status', status: status('searching') });
    const { pillHost, view } = mount({ send });
    view.start();
    await flush();

    byId<HTMLButtonElement>(pillHost, 'btn-retry').click();
    await flush();

    expect(send).toHaveBeenNthCalledWith(2, { kind: 'reconnect' });
    expect(send).toHaveBeenNthCalledWith(3, { kind: 'getStatus' });
  });
});

// ── offline-sticky — searching after app_not_running must not hide offline view ──

describe('offline-sticky', () => {
  async function pushDriver() {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValue({ ok: true, kind: 'status', status: status('searching', false) });
    const { pillHost, viewsHost, view } = mount({ send });
    view.start();
    await flush();
    const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls.at(-1)?.[0];
    if (!listener) throw new Error('listener not registered');
    const push = (phase: ConnectionStatus['phase']) =>
      listener({ ok: true, kind: 'status', status: status(phase, false) });
    return { pillHost, viewsHost, push };
  }

  it('keeps #view-offline visible and retains Retry when searching follows app_not_running', async () => {
    const { pillHost, viewsHost, push } = await pushDriver();
    push('connected'); // settle the sticky flag first
    push('app_not_running');
    expect(byId(viewsHost, 'view-offline').hidden).toBe(false);

    push('searching');
    expect(byId(viewsHost, 'view-offline').hidden).toBe(false);
    expect(byId(viewsHost, 'view-searching').hidden).toBe(true);
    expect(byId(pillHost, 'status-pill').textContent).toBe('○ Connecting…');
    expect(byId<HTMLButtonElement>(pillHost, 'btn-retry').hidden).toBe(false);
  });

  it('a genuine connected arrival after the offline+searching cycle switches the view away', async () => {
    const { viewsHost, push } = await pushDriver();
    push('connected');
    push('app_not_running');
    push('searching');
    push('connected');
    expect(byId(viewsHost, 'view-offline').hidden).toBe(true);
  });

  it('does not suppress the first searching spinner before offline has been shown', async () => {
    const { viewsHost, push } = await pushDriver();
    push('connected'); // hasShownOffline resets to false
    push('searching');
    expect(byId(viewsHost, 'view-searching').hidden).toBe(false);
    expect(byId(viewsHost, 'view-offline').hidden).toBe(true);
  });
});

// ── outdated-desktop view ────────────────────────────────────────────────

describe('outdated-desktop view', () => {
  it('shows the update view (NOT the pairing view) and the update pill', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValue({ ok: true, kind: 'status', status: status('searching') });
    const { pillHost, viewsHost, view } = mount({ send });
    view.start();
    await flush();
    const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls.at(-1)?.[0];
    if (!listener) throw new Error('listener not registered');

    listener({ ok: true, kind: 'status', status: status('outdated') });

    expect(byId(viewsHost, 'view-outdated').hidden).toBe(false);
    expect(byId(viewsHost, 'view-pair').hidden).toBe(true);
    expect(byId(pillHost, 'status-pill').textContent).toBe('⟳ Update the app');
    expect(byId<HTMLButtonElement>(pillHost, 'btn-retry').hidden).toBe(false);
  });

  it('clicking "Update the app" opens the same download page as "Get the app"', () => {
    const tabsCreateMock = vi.mocked(browser.tabs.create);
    tabsCreateMock.mockClear();
    const { viewsHost } = mount();

    byId<HTMLButtonElement>(viewsHost, 'btn-update-app').click();

    expect(tabsCreateMock).toHaveBeenCalledWith({ url: 'https://aijobhunter.app/download' });
  });
});

// ── bad_token / not_paired pairing message ──────────────────────────────────

describe('bad_token / not_paired pairing message', () => {
  it('shows a wrong-token message for bad_token and clears it for not_paired', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValue({ ok: true, kind: 'status', status: status('searching') });
    const { viewsHost, view } = mount({ send });
    view.start();
    await flush();
    const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls.at(-1)?.[0];
    if (!listener) throw new Error('listener not registered');

    listener({ ok: true, kind: 'status', status: status('bad_token') });
    expect(byId(viewsHost, 'pair-msg').textContent).toMatch(/didn't match/);
    expect(byId(viewsHost, 'view-pair').hidden).toBe(false);

    listener({ ok: true, kind: 'status', status: status('not_paired') });
    expect(byId(viewsHost, 'pair-msg').textContent).toBe('');
  });
});

// ── get the app (#btn-get-app) ───────────────────────────────────────────

describe('get the app (#btn-get-app)', () => {
  const tabsCreateMock = vi.mocked(browser.tabs.create);

  beforeEach(() => {
    tabsCreateMock.mockReset();
  });

  it('opens the public download page in a new tab when clicked', async () => {
    const { viewsHost } = mount();
    byId<HTMLButtonElement>(viewsHost, 'btn-get-app').click();
    await flush();

    expect(tabsCreateMock).toHaveBeenCalledTimes(1);
    expect(tabsCreateMock).toHaveBeenCalledWith({ url: 'https://aijobhunter.app/download' });
  });

  it('swallows a tabs.create rejection without propagating an unhandled error', async () => {
    tabsCreateMock.mockRejectedValueOnce(new Error('tabs unavailable'));
    const { viewsHost } = mount();

    byId<HTMLButtonElement>(viewsHost, 'btn-get-app').click();
    await flush();

    expect(tabsCreateMock).toHaveBeenCalledTimes(1);
  });
});

// ── savePairing (#btn-save-token) ────────────────────────────────────────

describe('savePairing (#btn-save-token)', () => {
  const looksLikeTokenMock = vi.mocked(looksLikeToken);
  const flushAll = () => new Promise((r) => setTimeout(r, 0));

  beforeEach(() => {
    looksLikeTokenMock.mockReturnValue(true);
  });

  it('rejects an input that does not look like a token, without calling send', async () => {
    looksLikeTokenMock.mockReturnValue(false);
    const send = vi.fn<[PopupRequest], Promise<PopupResponse>>();
    const { viewsHost } = mount({ send });
    byId<HTMLInputElement>(viewsHost, 'token-input').value = 'nope';

    byId<HTMLButtonElement>(viewsHost, 'btn-save-token').click();
    await flushAll();

    expect(send).not.toHaveBeenCalled();
    expect(byId(viewsHost, 'pair-msg').textContent).toMatch(/64-character/);
  });

  it('confirms with "✓ Authorized" then fires onPaired once the connected view settles', async () => {
    vi.useFakeTimers();
    try {
      const send = vi
        .fn<[PopupRequest], Promise<PopupResponse>>()
        .mockResolvedValueOnce({ ok: true, kind: 'token' })
        .mockResolvedValueOnce({ ok: true, kind: 'status', status: status('connected') });
      const onPaired = vi.fn();
      const { viewsHost } = mount({ send, onPaired });
      byId<HTMLInputElement>(viewsHost, 'token-input').value = 'a'.repeat(64);

      byId<HTMLButtonElement>(viewsHost, 'btn-save-token').click();
      expect(byId<HTMLButtonElement>(viewsHost, 'btn-save-token').disabled).toBe(true);
      await vi.runAllTimersAsync();

      expect(byId<HTMLButtonElement>(viewsHost, 'btn-save-token').textContent).toContain(
        'Authorized'
      );
      expect(onPaired).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it('restores the actionable label when the status refresh never reaches connected', async () => {
    vi.useFakeTimers();
    try {
      const send = vi
        .fn<[PopupRequest], Promise<PopupResponse>>()
        .mockResolvedValueOnce({ ok: true, kind: 'token' })
        .mockResolvedValueOnce({ ok: true, kind: 'status', status: status('app_not_running') });
      const onPaired = vi.fn();
      const { viewsHost } = mount({ send, onPaired });
      byId<HTMLInputElement>(viewsHost, 'token-input').value = 'a'.repeat(64);

      byId<HTMLButtonElement>(viewsHost, 'btn-save-token').click();
      await vi.runAllTimersAsync();

      const btn = byId<HTMLButtonElement>(viewsHost, 'btn-save-token');
      expect(btn.disabled).toBe(false);
      expect(btn.textContent).toBe('Save & pair');
      expect(onPaired).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it('surfaces the desktop rejection error and restores the button', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValueOnce({ ok: false, error: 'bad token' });
    const { viewsHost } = mount({ send });
    byId<HTMLInputElement>(viewsHost, 'token-input').value = 'a'.repeat(64);

    byId<HTMLButtonElement>(viewsHost, 'btn-save-token').click();
    await flushAll();

    const btn = byId<HTMLButtonElement>(viewsHost, 'btn-save-token');
    expect(btn.disabled).toBe(false);
    expect(btn.textContent).toBe('Save & pair');
    expect(byId(viewsHost, 'pair-msg').textContent).toBe('bad token');
  });

  it('restores the actionable button when the pairing request rejects', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockRejectedValueOnce(new Error('transport down'));
    const { viewsHost } = mount({ send });
    byId<HTMLInputElement>(viewsHost, 'token-input').value = 'a'.repeat(64);

    byId<HTMLButtonElement>(viewsHost, 'btn-save-token').click();
    await flushAll();
    await flushAll();

    const btn = byId<HTMLButtonElement>(viewsHost, 'btn-save-token');
    expect(btn.disabled).toBe(false);
    expect(btn.textContent).toBe('Save & pair');
    expect(byId(viewsHost, 'pair-msg').textContent).toMatch(/failed/i);
  });

  it('saves on Enter in the token input, not just a button click', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValueOnce({ ok: false, error: 'bad token' });
    const { viewsHost } = mount({ send });
    const input = byId<HTMLInputElement>(viewsHost, 'token-input');
    input.value = 'a'.repeat(64);

    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    await flushAll();

    expect(send).toHaveBeenCalledWith({ kind: 'setToken', token: 'a'.repeat(64) });
  });
});

// ── onStatus / onConnected callback semantics ───────────────────────────────

describe('onStatus / onConnected callback semantics', () => {
  it('calls onStatus on every render, including a repeated push of the same phase', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValue({ ok: true, kind: 'status', status: status('searching') });
    const onStatus = vi.fn();
    const { view } = mount({ send, onStatus });
    view.start();
    await flush();
    const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls.at(-1)?.[0];
    if (!listener) throw new Error('listener not registered');

    listener({ ok: true, kind: 'status', status: status('connected') });
    listener({ ok: true, kind: 'status', status: status('connected') });

    expect(onStatus).toHaveBeenCalledTimes(3); // start()'s fetch + the two pushes
  });

  it('calls onConnected exactly once per transition into connected, not on a repeated push', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValue({ ok: true, kind: 'status', status: status('searching') });
    const onConnected = vi.fn();
    const { view } = mount({ send, onConnected });
    view.start();
    await flush();
    const listener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls.at(-1)?.[0];
    if (!listener) throw new Error('listener not registered');

    listener({ ok: true, kind: 'status', status: status('searching') });
    listener({ ok: true, kind: 'status', status: status('connected') });
    listener({ ok: true, kind: 'status', status: status('connected') });
    expect(onConnected).toHaveBeenCalledTimes(1);

    listener({ ok: true, kind: 'status', status: status('app_not_running') });
    listener({ ok: true, kind: 'status', status: status('connected') });
    expect(onConnected).toHaveBeenCalledTimes(2);
  });
});

// ── refresh() / focusPairInputIfShown() (the popup's own "Unpair" seam) ────

describe('refresh() and focusPairInputIfShown()', () => {
  it('refresh() re-fetches and re-renders', async () => {
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValueOnce({ ok: true, kind: 'status', status: status('not_paired') });
    const { pillHost, view } = mount({ send });

    await view.refresh();

    expect(send).toHaveBeenCalledWith({ kind: 'getStatus' });
    expect(byId(pillHost, 'status-pill').textContent).toBe('⚠ Not paired');
  });

  it('focusPairInputIfShown() focuses the token input only while the pair view is visible', async () => {
    document.body.innerHTML = '';
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValueOnce({ ok: true, kind: 'status', status: status('not_paired') });
    const { viewsHost, view } = mount({ send });
    document.body.append(viewsHost); // focus() is a no-op unless attached

    await view.refresh();
    view.focusPairInputIfShown();

    expect(document.activeElement).toBe(byId(viewsHost, 'token-input'));
  });

  it('is a no-op when the pair view is not shown', async () => {
    document.body.innerHTML = '';
    const send = vi
      .fn<[PopupRequest], Promise<PopupResponse>>()
      .mockResolvedValueOnce({ ok: true, kind: 'status', status: status('connected') });
    const { viewsHost, view } = mount({ send });
    document.body.append(viewsHost);

    await view.refresh();
    view.focusPairInputIfShown();

    expect(document.activeElement).not.toBe(byId(viewsHost, 'token-input'));
  });
});
