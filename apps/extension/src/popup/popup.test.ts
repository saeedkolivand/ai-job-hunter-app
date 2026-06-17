/**
 * Unit tests for the pure view-decision helpers exported from popup.ts.
 *
 * popup.ts runs side-effects at module load (DOM queries via byId, wire(),
 * refreshStatusWithTimeout()).  To keep tests light we import only the PURE
 * exported functions directly — they have zero DOM dependency and zero
 * browser-API calls, so no DOM scaffolding and no @wxt-dev/browser mock
 * are strictly required for the assertions here.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import type { ConnectionStatus } from '../lib/messages';
import { looksLikeToken } from '../lib/storage';

// vi.mock must come before the import that triggers the module side-effects.
// popup.ts imports @wxt-dev/browser; stub it out so the module-level
// side-effects (wire(), runtime listener registration) have a usable browser
// namespace. We also need a minimal DOM for the byId calls.

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    runtime: {
      sendMessage: vi.fn(),
      onMessage: { addListener: vi.fn() },
    },
  },
}));

vi.mock('../lib/storage', () => ({
  looksLikeToken: vi.fn(() => false),
}));

// Build the minimal DOM that popup.ts queries at module load (byId calls).
// Must happen before the dynamic import below so jsdom has the elements when
// the module-level `els` constant is initialised.
function buildPopupDom(): void {
  document.body.innerHTML = `
    <span id="status-pill"></span>
    <div id="view-import" hidden></div>
    <div id="view-pair" hidden></div>
    <div id="view-offline" hidden></div>
    <div id="view-searching"></div>
    <button id="btn-url"></button>
    <button id="btn-scan"></button>
    <input id="chk-applied" type="checkbox" />
    <p id="import-msg"></p>
    <button id="btn-unpair"></button>
    <input id="token-input" type="text" />
    <p id="pair-msg"></p>
    <button id="btn-save-token"></button>
    <button id="btn-retry"></button>
    <button id="btn-open-settings"></button>
    <button id="btn-help"></button>
    <p id="help-popover" hidden></p>
    <button id="btn-get-app"></button>
  `;
}

buildPopupDom();

// Dynamic import AFTER DOM + mocks are in place. The module wires its DOM event
// listeners at load (wire()), so the behavioral tests below drive the controller
// by dispatching real clicks on the wired buttons and asserting DOM state.
const { resolveStatusResponse, resolveImportResponse } = await import('./popup');

const sendMessageMock = vi.mocked(browser.runtime.sendMessage);
const looksLikeTokenMock = vi.mocked(looksLikeToken);
const byId = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

// ── resolveStatusResponse ─────────────────────────────────────────────────────

describe('resolveStatusResponse', () => {
  it('returns the status when response is ok with kind=status', () => {
    const status = { phase: 'connected' as const, port: 47615, hasToken: true };
    const res = { ok: true as const, kind: 'status' as const, status };
    expect(resolveStatusResponse(res, false)).toEqual(status);
  });

  it('returns an app_not_running offline fallback when ok=false', () => {
    const res = { ok: false as const, error: 'Service worker not responding.' };
    const result = resolveStatusResponse(res, true);
    expect(result.phase).toBe('app_not_running');
    // Preserves the last-known token state.
    expect(result.hasToken).toBe(true);
    expect(result.port).toBeNull();
  });

  it('returns an app_not_running offline fallback for an unexpected ok kind', () => {
    // A `{ ok: true, kind: 'token' }` response is not a status reply.
    const res = { ok: true as const, kind: 'token' as const };
    const result = resolveStatusResponse(res, false);
    expect(result.phase).toBe('app_not_running');
    expect(result.hasToken).toBe(false);
  });
});

// ── resolveImportResponse ─────────────────────────────────────────────────────

describe('resolveImportResponse', () => {
  it('returns an error message when ok=false', () => {
    const res = { ok: false as const, error: 'Bridge unavailable.' };
    const { text, tone } = resolveImportResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe('Bridge unavailable.');
  });

  it('returns the unexpected-response error message when kind is not import', () => {
    // Dead-end state: background replied with a non-import kind to an import
    // request (e.g. a stale status push, message ordering issue).
    const res = { ok: true as const, kind: 'token' as const };
    const { text, tone } = resolveImportResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe('Unexpected response — please retry.');
  });

  it('returns the result error text when the import result carries an error', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { error: 'Desktop app rejected the job URL.' },
    };
    const { text, tone } = resolveImportResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe('Desktop app rejected the job URL.');
  });

  it('names the imported job and points to where it landed when a title is present', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-123', status: 'saved', title: 'Senior Rust Engineer' },
    };
    const { text, tone } = resolveImportResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe(
      'Imported “Senior Rust Engineer”. Open AI Job Hunter → Applications to view it.'
    );
  });

  it('falls back to a generic success + landing hint when no title is present', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-456' },
    };
    const { text, tone } = resolveImportResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('Imported. Open AI Job Hunter → Applications to view it.');
  });
});

// ── controller behavior (wired DOM) ───────────────────────────────────────────

describe('help toggle (#btn-help)', () => {
  it('toggles the popover open/closed and keeps aria-expanded in sync', () => {
    const btn = byId<HTMLButtonElement>('btn-help');
    const popover = byId<HTMLParagraphElement>('help-popover');
    popover.hidden = true;
    btn.setAttribute('aria-expanded', 'false');

    btn.click();
    expect(popover.hidden).toBe(false);
    expect(btn.getAttribute('aria-expanded')).toBe('true');

    btn.click();
    expect(popover.hidden).toBe(true);
    expect(btn.getAttribute('aria-expanded')).toBe('false');
  });
});

describe('savePairing (#btn-save-token)', () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));

  beforeEach(() => {
    sendMessageMock.mockReset();
    looksLikeTokenMock.mockReturnValue(true);
    const btn = byId<HTMLButtonElement>('btn-save-token');
    btn.disabled = false;
    btn.textContent = 'Save & pair';
    byId<HTMLInputElement>('token-input').value = 'a'.repeat(64);
    byId<HTMLElement>('view-import').hidden = true;
  });

  it('confirms with "✓ Authorized" then flips to the import view on success', async () => {
    vi.useFakeTimers();
    try {
      sendMessageMock.mockResolvedValueOnce({ ok: true, kind: 'token' }).mockResolvedValueOnce({
        ok: true,
        kind: 'status',
        status: { phase: 'connected', port: 1, hasToken: true },
      });

      byId<HTMLButtonElement>('btn-save-token').click();
      await vi.runAllTimersAsync();

      expect(byId<HTMLButtonElement>('btn-save-token').textContent).toContain('Authorized');
      expect(byId<HTMLElement>('view-import').hidden).toBe(false);
    } finally {
      // Restore real timers even if an assertion throws, so later tests don't
      // inherit fake timers and flake.
      vi.useRealTimers();
    }
  });

  it('resets the button when the status refresh does not reach the connected view', async () => {
    vi.useFakeTimers();
    try {
      sendMessageMock.mockResolvedValueOnce({ ok: true, kind: 'token' }).mockResolvedValueOnce({
        ok: true,
        kind: 'status',
        status: { phase: 'app_not_running', port: null, hasToken: true },
      });

      byId<HTMLButtonElement>('btn-save-token').click();
      await vi.runAllTimersAsync();

      const btn = byId<HTMLButtonElement>('btn-save-token');
      expect(btn.disabled).toBe(false);
      expect(btn.textContent).toBe('Save & pair');
      expect(byId<HTMLElement>('view-import').hidden).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });

  it('restores the actionable button when the pairing request rejects', async () => {
    sendMessageMock.mockRejectedValueOnce(new Error('transport down'));

    byId<HTMLButtonElement>('btn-save-token').click();
    await flush();
    await flush();

    const btn = byId<HTMLButtonElement>('btn-save-token');
    expect(btn.disabled).toBe(false);
    expect(btn.textContent).toBe('Save & pair');
    expect(byId<HTMLParagraphElement>('pair-msg').textContent).toMatch(/failed/i);
  });
});

describe('header Retry visibility', () => {
  // wire() registers a runtime message listener that calls render() on status pushes.
  // Grab it from the mocked addListener so we can drive render() with a phase.
  const statusListener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
    | ((message: unknown) => void)
    | undefined;
  const push = (phase: ConnectionStatus['phase']) =>
    statusListener?.({ ok: true, kind: 'status', status: { phase, port: null, hasToken: true } });

  it('is shown only in the app_not_running state', () => {
    expect(statusListener).toBeTypeOf('function');
    const retry = byId<HTMLButtonElement>('btn-retry');

    push('app_not_running');
    expect(retry.hidden).toBe(false);

    push('connected');
    expect(retry.hidden).toBe(true);

    push('searching');
    expect(retry.hidden).toBe(true);

    push('not_paired');
    expect(retry.hidden).toBe(true);
  });
});
