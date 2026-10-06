/**
 * Unit tests for the connection-status component's pairing + deep-link
 * controls: "Get the app" and the token form (`savePairing`).
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import { looksLikeToken } from '../../lib/storage';

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    tabs: { create: vi.fn() },
    runtime: { onMessage: { addListener: vi.fn() } },
  },
}));

vi.mock('../../lib/storage', () => ({
  looksLikeToken: vi.fn(() => false),
}));

import type { ConnectionStatusDeps } from '../connection-status';
import { byId, flush, mount, newSend, statusReply } from './test-support';

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

  beforeEach(() => {
    looksLikeTokenMock.mockReturnValue(true);
  });

  /** Mount against `send`, type a 64-char token (or `token`) and click Save & pair. */
  function pair(
    send: ReturnType<typeof newSend>,
    deps: Partial<ConnectionStatusDeps> = {},
    token = 'a'.repeat(64)
  ) {
    const { viewsHost } = mount({ send, ...deps });
    byId<HTMLInputElement>(viewsHost, 'token-input').value = token;
    const saveBtn = byId<HTMLButtonElement>(viewsHost, 'btn-save-token');
    saveBtn.click();
    return { viewsHost, saveBtn };
  }

  it('rejects an input that does not look like a token, without calling send', async () => {
    looksLikeTokenMock.mockReturnValue(false);
    const send = newSend();
    const { viewsHost } = pair(send, {}, 'nope');
    await flush();

    expect(send).not.toHaveBeenCalled();
    expect(byId(viewsHost, 'pair-msg').textContent).toMatch(/64-character/);
  });

  it('confirms with "✓ Authorized" then fires onPaired once the connected view settles', async () => {
    vi.useFakeTimers();
    try {
      const send = newSend()
        .mockResolvedValueOnce({ ok: true, kind: 'token' })
        .mockResolvedValueOnce(statusReply('connected'));
      const onPaired = vi.fn();
      const { saveBtn } = pair(send, { onPaired });
      expect(saveBtn.disabled).toBe(true);
      await vi.runAllTimersAsync();

      expect(saveBtn.textContent).toContain('Authorized');
      expect(onPaired).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it('restores the actionable label when the status refresh never reaches connected', async () => {
    vi.useFakeTimers();
    try {
      const send = newSend()
        .mockResolvedValueOnce({ ok: true, kind: 'token' })
        .mockResolvedValueOnce(statusReply('app_not_running'));
      const onPaired = vi.fn();
      const { saveBtn } = pair(send, { onPaired });
      await vi.runAllTimersAsync();

      expect(saveBtn.disabled).toBe(false);
      expect(saveBtn.textContent).toBe('Save & pair');
      expect(onPaired).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it('surfaces the desktop rejection error and restores the button', async () => {
    const { viewsHost, saveBtn } = pair(
      newSend().mockResolvedValueOnce({ ok: false, error: 'bad token' })
    );
    await flush();

    expect(saveBtn.disabled).toBe(false);
    expect(saveBtn.textContent).toBe('Save & pair');
    expect(byId(viewsHost, 'pair-msg').textContent).toBe('bad token');
  });

  it('restores the actionable button when the pairing request rejects', async () => {
    const { viewsHost, saveBtn } = pair(
      newSend().mockRejectedValueOnce(new Error('transport down'))
    );
    await flush();
    await flush();

    expect(saveBtn.disabled).toBe(false);
    expect(saveBtn.textContent).toBe('Save & pair');
    expect(byId(viewsHost, 'pair-msg').textContent).toMatch(/failed/i);
  });

  it('saves on Enter in the token input, not just a button click', async () => {
    const send = newSend().mockResolvedValueOnce({ ok: false, error: 'bad token' });
    const { viewsHost } = mount({ send });
    const input = byId<HTMLInputElement>(viewsHost, 'token-input');
    input.value = 'a'.repeat(64);

    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    await flush();

    expect(send).toHaveBeenCalledWith({ kind: 'setToken', token: 'a'.repeat(64) });
  });
});
