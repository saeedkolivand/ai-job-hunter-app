/**
 * Unit tests for the connection-status component's status handling:
 * `resolveStatusResponse` plus the mounted pill/retry/offline/outdated/
 * searching behavior and the onStatus/onConnected callbacks — driven with bare
 * `<div>` hosts and a mocked `send`/`browser`, without going through either
 * popup.ts or sidepanel.ts.
 */

import { describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    tabs: { create: vi.fn() },
    runtime: { onMessage: { addListener: vi.fn() } },
  },
}));

import { resolveStatusResponse } from '../connection-status';
import {
  byId,
  flush,
  mount,
  newSend,
  type Phase,
  pillText,
  retryHidden,
  started,
  statusReply,
} from './test-support';

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
    expect(retryHidden(pillHost)).toBe(true);
    expect(pillText(pillHost)).toBe('○ Connecting…');
  });
});

// ── start() — first fetch + live push ───────────────────────────────────────

describe('start()', () => {
  it('fetches getStatus and renders the result', async () => {
    const { pillHost, send } = await started('connected');

    expect(send).toHaveBeenCalledWith({ kind: 'getStatus' });
    expect(pillText(pillHost)).toBe('● Connected');
  });

  it('registers a live-push listener that re-renders on a pushed status', async () => {
    const { pillHost, push } = await started('not_paired');
    expect(pillText(pillHost)).toBe('⚠ Not paired');

    push('connected');

    expect(pillText(pillHost)).toBe('● Connected');
  });

  it('ignores a pushed message of a different kind — no cross-talk with other broadcasts', async () => {
    const { pillHost, listener } = await started('not_paired');
    expect(pillText(pillHost)).toBe('⚠ Not paired');

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

    expect(pillText(pillHost)).toBe('⚠ Not paired');
  });

  it('falls back to the offline/Retry view when send() REJECTS (not just times out) — MV3 worker asleep/crashed', async () => {
    const send = newSend().mockRejectedValueOnce(
      new Error('Could not establish connection. Receiving end does not exist.')
    );
    const { pillHost, viewsHost, view } = mount({ send });

    view.start();
    await flush();

    expect(pillText(pillHost)).toBe('✕ App not running');
    expect(byId(viewsHost, 'view-offline').hidden).toBe(false);
    expect(retryHidden(pillHost)).toBe(false);
  });
});

// ── refresh() rejection ──────────────────────────────────────────────────────

describe('refresh() rejection', () => {
  it('falls back to the offline/Retry view instead of throwing when send() rejects', async () => {
    const send = newSend().mockRejectedValueOnce(new Error('message channel closed'));
    const { pillHost, viewsHost, view } = mount({ send });

    await expect(view.refresh()).resolves.toBeUndefined();

    expect(pillText(pillHost)).toBe('✕ App not running');
    expect(byId(viewsHost, 'view-offline').hidden).toBe(false);
  });
});

// ── header Retry visibility ─────────────────────────────────────────────────

describe('header Retry visibility', () => {
  it('is shown only for app_not_running and outdated', async () => {
    const { pillHost, push } = await started();

    for (const [phase, hidden] of [
      ['app_not_running', false],
      ['outdated', false],
      ['connected', true],
      ['searching', true],
      ['not_paired', true],
    ] as const) {
      push(phase);
      expect(retryHidden(pillHost), phase).toBe(hidden);
    }
  });

  it('clicking retry sends reconnect then re-fetches status', async () => {
    const send = newSend()
      .mockResolvedValueOnce(statusReply('app_not_running'))
      .mockResolvedValueOnce({ ok: true, kind: 'token' })
      .mockResolvedValueOnce(statusReply('searching'));
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
  it('keeps #view-offline visible and retains Retry when searching follows app_not_running', async () => {
    const { pillHost, viewsHost, push } = await started('searching', { hasToken: false });
    push('connected'); // settle the sticky flag first
    push('app_not_running');
    expect(byId(viewsHost, 'view-offline').hidden).toBe(false);

    push('searching');
    expect(byId(viewsHost, 'view-offline').hidden).toBe(false);
    expect(byId(viewsHost, 'view-searching').hidden).toBe(true);
    expect(pillText(pillHost)).toBe('○ Connecting…');
    expect(retryHidden(pillHost)).toBe(false);
  });

  it('a genuine connected arrival after the offline+searching cycle switches the view away', async () => {
    const { viewsHost, push } = await started('searching', { hasToken: false });
    push('connected');
    push('app_not_running');
    push('searching');
    push('connected');
    expect(byId(viewsHost, 'view-offline').hidden).toBe(true);
  });

  it('does not suppress the first searching spinner before offline has been shown', async () => {
    const { viewsHost, push } = await started('searching', { hasToken: false });
    push('connected'); // hasShownOffline resets to false
    push('searching');
    expect(byId(viewsHost, 'view-searching').hidden).toBe(false);
    expect(byId(viewsHost, 'view-offline').hidden).toBe(true);
  });
});

// ── outdated-desktop view ────────────────────────────────────────────────

describe('outdated-desktop view', () => {
  it('shows the update view (NOT the pairing view) and the update pill', async () => {
    const { pillHost, viewsHost, push } = await started();

    push('outdated');

    expect(byId(viewsHost, 'view-outdated').hidden).toBe(false);
    expect(byId(viewsHost, 'view-pair').hidden).toBe(true);
    expect(pillText(pillHost)).toBe('⟳ Update the app');
    expect(retryHidden(pillHost)).toBe(false);
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
    const { viewsHost, push } = await started();

    push('bad_token');
    expect(byId(viewsHost, 'pair-msg').textContent).toMatch(/didn't match/);
    expect(byId(viewsHost, 'view-pair').hidden).toBe(false);

    push('not_paired');
    expect(byId(viewsHost, 'pair-msg').textContent).toBe('');
  });
});

// ── onStatus / onConnected callback semantics ───────────────────────────────

describe('onStatus / onConnected callback semantics', () => {
  it('calls onStatus on every render, including a repeated push of the same phase', async () => {
    const onStatus = vi.fn();
    const { push } = await started('searching', { onStatus });

    push('connected');
    push('connected');

    expect(onStatus).toHaveBeenCalledTimes(3); // start()'s fetch + the two pushes
  });

  it('calls onConnected exactly once per transition into connected, not on a repeated push', async () => {
    const onConnected = vi.fn();
    const { push } = await started('searching', { onConnected });

    push('searching');
    push('connected');
    push('connected');
    expect(onConnected).toHaveBeenCalledTimes(1);

    push('app_not_running');
    push('connected');
    expect(onConnected).toHaveBeenCalledTimes(2);
  });
});

// ── refresh() / focusPairInputIfShown() (the popup's own "Unpair" seam) ────

describe('refresh() and focusPairInputIfShown()', () => {
  /** Attach the views to the document (focus() is a no-op unless attached),
   *  answer one fetch with `phase`, refresh and try to focus the token input. */
  async function refreshAndFocus(phase: Phase) {
    document.body.innerHTML = '';
    const send = newSend().mockResolvedValueOnce(statusReply(phase));
    const { pillHost, viewsHost, view } = mount({ send });
    document.body.append(viewsHost);

    await view.refresh();
    view.focusPairInputIfShown();
    return { pillHost, viewsHost, send };
  }

  it('refresh() re-fetches and re-renders', async () => {
    const { pillHost, send } = await refreshAndFocus('not_paired');

    expect(send).toHaveBeenCalledWith({ kind: 'getStatus' });
    expect(pillText(pillHost)).toBe('⚠ Not paired');
  });

  it('focusPairInputIfShown() focuses the token input only while the pair view is visible', async () => {
    const { viewsHost } = await refreshAndFocus('not_paired');

    expect(document.activeElement).toBe(byId(viewsHost, 'token-input'));
  });

  it('is a no-op when the pair view is not shown', async () => {
    const { viewsHost } = await refreshAndFocus('connected');

    expect(document.activeElement).not.toBe(byId(viewsHost, 'token-input'));
  });
});
