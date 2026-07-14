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
    tabs: { create: vi.fn() },
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
    <button id="btn-import"></button>
    <button id="btn-fill"></button>
    <p id="applied-status" hidden></p>
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
    <div id="view-outdated" hidden></div>
    <button id="btn-update-app"></button>
  `;
}

buildPopupDom();

// Dynamic import AFTER DOM + mocks are in place. The module wires its DOM event
// listeners at load (wire()), so the behavioral tests below drive the controller
// by dispatching real clicks on the wired buttons and asserting DOM state.
const {
  resolveStatusResponse,
  resolveImportResponse,
  resolveFillResponse,
  resolveAppliedStatusLine,
  resolveImportButtonLabel,
} = await import('./popup');

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
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('err');
    expect(text).toBe('Bridge unavailable.');
  });

  it('returns the unexpected-response error message when kind is not import', () => {
    // Dead-end state: background replied with a non-import kind to an import
    // request (e.g. a stale status push, message ordering issue).
    const res = { ok: true as const, kind: 'token' as const };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('err');
    expect(text).toBe('Unexpected response — please retry.');
  });

  it('returns the result error text when the import result carries an error', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { error: 'Desktop app rejected the job URL.' },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('err');
    expect(text).toBe('Desktop app rejected the job URL.');
  });

  it('names the imported job and points to where it landed when a title is present', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-123', status: 'saved', title: 'Senior Rust Engineer' },
    };
    const { text, tone } = resolveImportResponse(res, false);
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
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe('Imported. Open AI Job Hunter → Applications to view it.');
  });

  it('shows a partial message with title when partial=true', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-789', title: 'Frontend Engineer', partial: true },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe(
      "Imported “Frontend Engineer” — couldn't read the description. Open AI Job Hunter → Applications to paste it."
    );
  });

  it('shows a partial message without title when partial=true and no title', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-000', partial: true },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe(
      "Imported — couldn't read the description. Open AI Job Hunter → Applications to paste it."
    );
  });

  // ── status transparency (dedup-merge into a pre-existing non-saved row) ─────

  it('surfaces a "already tracked" transparency message when the matched row is already past saved and the checkbox was unticked', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-existing', status: 'applied', title: 'Backend Engineer' },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe(
      '“Backend Engineer” is already tracked as Applied — status unchanged. Open AI Job Hunter → Applications to view it.'
    );
  });

  it('surfaces the transparency message without a title when the desktop parsed none', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-existing', status: 'interviewing' },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe(
      'This job is already tracked as Interviewing — status unchanged. Open AI Job Hunter → Applications to view it.'
    );
  });

  it('does not show the transparency message when status is saved (unchanged behavior)', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-1', status: 'saved', title: 'QA Engineer' },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe('Imported “QA Engineer”. Open AI Job Hunter → Applications to view it.');
  });

  it('does not show the transparency message when the checkbox was ticked, even for a non-saved status', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-2', status: 'applied', title: 'DevOps Engineer' },
    };
    const { text, tone } = resolveImportResponse(res, true);
    expect(tone).toBe('ok');
    expect(text).toBe('Imported “DevOps Engineer”. Open AI Job Hunter → Applications to view it.');
  });

  it('prefers the partial message over the transparency message (partial stub → unchanged)', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: {
        applicationId: 'app-3',
        status: 'applied',
        title: 'Frontend Engineer',
        partial: true,
      },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe(
      "Imported “Frontend Engineer” — couldn't read the description. Open AI Job Hunter → Applications to paste it."
    );
  });
});

// ── resolveFillResponse (assisted autofill) ────────────────────────────────────

describe('resolveFillResponse', () => {
  it('surfaces the desktop refusal (autofill opted out) as an error', () => {
    const res = { ok: false as const, error: 'Autofill is off.' };
    const { text, tone } = resolveFillResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe('Autofill is off.');
  });

  it('returns the unexpected-response error when kind is not fill', () => {
    const res = { ok: true as const, kind: 'token' as const };
    const { text, tone } = resolveFillResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe('Unexpected response — please retry.');
  });

  it('reports the no-match case as a benign message, not an error', () => {
    const res = {
      ok: true as const,
      kind: 'fill' as const,
      summary: { filled: [], nameSplit: null, filledNothing: true },
    };
    const { text, tone } = resolveFillResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('No matchable fields found on this page.');
  });

  it('summarises the filled count and points the user at the page', () => {
    const res = {
      ok: true as const,
      kind: 'fill' as const,
      summary: {
        filled: [
          { key: 'email', label: 'Email', count: 2 },
          { key: 'phone', label: 'Phone', count: 1 },
        ],
        nameSplit: null,
        filledNothing: false,
      },
    };
    const { text, tone } = resolveFillResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('Filled 3 fields — review them on the page.');
  });

  it('flags the name-split guess in the confirmation', () => {
    const res = {
      ok: true as const,
      kind: 'fill' as const,
      summary: {
        filled: [{ key: 'firstName', label: 'First name', count: 1 }],
        nameSplit: { first: 'Saeed', last: 'Kolivand' },
        filledNothing: false,
      },
    };
    const { text, tone } = resolveFillResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('Filled 1 field — review them on the page (name split is a guess — verify).');
  });
});

// ── resolveAppliedStatusLine / resolveImportButtonLabel ───────────────────────

describe('resolveAppliedStatusLine', () => {
  it('returns null when the response is not an appliedCheck response', () => {
    const res = { ok: true as const, kind: 'token' as const };
    expect(resolveAppliedStatusLine(res)).toBeNull();
  });

  it('returns null when ok is false', () => {
    const res = { ok: false as const, error: 'boom' };
    expect(resolveAppliedStatusLine(res)).toBeNull();
  });

  it('returns null when the result carries an error (soft-fail)', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: false, error: 'malformed' },
    };
    expect(resolveAppliedStatusLine(res)).toBeNull();
  });

  it('returns null when not found', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: false },
    };
    expect(resolveAppliedStatusLine(res)).toBeNull();
  });

  it('reports "Saved in your pipeline" for a found saved status with no title', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: true, status: 'saved' },
    };
    expect(resolveAppliedStatusLine(res)).toBe('Saved in your pipeline.');
  });

  it('names the job when a title is present for a found saved status', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: true, status: 'saved', title: 'Senior Rust Engineer' },
    };
    expect(resolveAppliedStatusLine(res)).toBe('“Senior Rust Engineer” is saved in your pipeline.');
  });

  it('reports the applied date for a found non-saved status with appliedAt', () => {
    const appliedAt = Date.UTC(2026, 5, 12); // Jun 12, 2026 (UTC)
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: true, status: 'applied', appliedAt },
    };
    expect(resolveAppliedStatusLine(res)).toMatch(/^Already in your pipeline — applied .+\.$/);
  });

  it('names the job + date together when both a title and appliedAt are present', () => {
    const appliedAt = Date.UTC(2026, 5, 12);
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: true, status: 'interviewing', title: 'Backend Engineer', appliedAt },
    };
    expect(resolveAppliedStatusLine(res)).toMatch(
      /^“Backend Engineer” is already in your pipeline — applied .+\.$/
    );
  });

  it('falls back to a dateless message when a non-saved status carries no appliedAt', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: true, status: 'applied' },
    };
    expect(resolveAppliedStatusLine(res)).toBe('Already in your pipeline.');
  });
});

describe('resolveImportButtonLabel', () => {
  it('returns the default label when not found', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: false },
    };
    expect(resolveImportButtonLabel(res)).toBe('Import this job');
  });

  it('returns the default label when the result carries an error', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: true, error: 'malformed' },
    };
    expect(resolveImportButtonLabel(res)).toBe('Import this job');
  });

  it('returns the default label for a non-appliedCheck response', () => {
    const res = { ok: true as const, kind: 'token' as const };
    expect(resolveImportButtonLabel(res)).toBe('Import this job');
  });

  it('returns the relabeled action when found', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: true, status: 'saved' },
    };
    expect(resolveImportButtonLabel(res)).toBe('Re-import / update');
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

describe('doFill (#btn-fill)', () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));

  beforeEach(() => {
    sendMessageMock.mockReset();
    byId<HTMLButtonElement>('btn-fill').disabled = false;
    byId<HTMLParagraphElement>('import-msg').textContent = '';
  });

  it('shows "Filling…" then the success summary, and re-enables the button', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'fill',
      summary: {
        filled: [{ key: 'email', label: 'Email', count: 1 }],
        nameSplit: null,
        filledNothing: false,
      },
    });

    const btn = byId<HTMLButtonElement>('btn-fill');
    btn.click();
    // The click handler disables the button and sets "Filling…" synchronously,
    // before the (mocked) sendMessage promise resolves.
    expect(btn.disabled).toBe(true);
    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe('Filling…');

    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      'Filled 1 field — review them on the page.'
    );
    expect(btn.disabled).toBe(false);
  });

  it('shows the retry message and re-enables the button when sendMessage rejects', async () => {
    sendMessageMock.mockRejectedValueOnce(new Error('message channel closed'));

    const btn = byId<HTMLButtonElement>('btn-fill');
    btn.click();
    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      'Autofill failed. Please retry.'
    );
    expect(btn.disabled).toBe(false);
  });
});

describe('doImport (#btn-import)', () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));

  beforeEach(() => {
    sendMessageMock.mockReset();
    byId<HTMLButtonElement>('btn-import').disabled = false;
    byId<HTMLParagraphElement>('import-msg').textContent = '';
    byId<HTMLInputElement>('chk-applied').checked = false;
  });

  it('shows "Importing…" then the already-tracked transparency message, sends applied: false, and re-enables the button', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'import',
      result: { applicationId: 'app-existing', status: 'applied', title: 'Backend Engineer' },
    });

    const btn = byId<HTMLButtonElement>('btn-import');
    btn.click();
    // The click handler disables the button and sets "Importing…" synchronously,
    // before the (mocked) sendMessage promise resolves.
    expect(btn.disabled).toBe(true);
    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe('Importing…');

    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      '“Backend Engineer” is already tracked as Applied — status unchanged. Open AI Job Hunter → Applications to view it.'
    );
    expect(btn.disabled).toBe(false);
    // The checkbox was unticked — the outgoing request must carry applied: false.
    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'import', applied: false });
  });

  it('shows the plain "Imported" success message and re-enables the button', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'import',
      result: { applicationId: 'app-new', status: 'saved', title: 'Senior Rust Engineer' },
    });

    const btn = byId<HTMLButtonElement>('btn-import');
    btn.click();
    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      'Imported “Senior Rust Engineer”. Open AI Job Hunter → Applications to view it.'
    );
    expect(btn.disabled).toBe(false);
  });
});

describe('get the app (#btn-get-app)', () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));
  const tabsCreateMock = vi.mocked(browser.tabs.create);

  beforeEach(() => {
    tabsCreateMock.mockReset();
  });

  it('opens the public download page in a new tab when clicked', async () => {
    byId<HTMLButtonElement>('btn-get-app').click();
    await flush();

    expect(tabsCreateMock).toHaveBeenCalledTimes(1);
    expect(tabsCreateMock).toHaveBeenCalledWith({ url: 'https://aijobhunter.app/download' });
  });

  it('swallows a tabs.create rejection without propagating an unhandled error', async () => {
    tabsCreateMock.mockRejectedValueOnce(new Error('tabs unavailable'));

    byId<HTMLButtonElement>('btn-get-app').click();
    await flush();

    // getApp() wraps tabs.create in try/catch; the rejection is swallowed
    // inside getApp, so reaching this point without an unhandled rejection is
    // the assertion. The call still fired exactly once.
    expect(tabsCreateMock).toHaveBeenCalledTimes(1);
  });
});

describe('header Retry visibility', () => {
  // wire() registers a runtime message listener that calls render() on status pushes.
  // Grab it from the mocked addListener so we can drive render() with a phase.
  const statusListener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
    ((message: unknown) => void) | undefined;
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

describe('offline-sticky — searching after app_not_running must not hide offline view', () => {
  // Reuse the same onMessage listener registered during module load.
  const statusListener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
    ((message: unknown) => void) | undefined;
  if (!statusListener) throw new Error('onMessage status listener not registered');
  const push = (phase: ConnectionStatus['phase']) =>
    statusListener({ ok: true, kind: 'status', status: { phase, port: null, hasToken: false } });

  beforeEach(() => {
    // Reset the sticky flag by pushing a settled phase so each test starts clean.
    push('connected');
  });

  it('keeps #view-offline visible and retains the Retry button when searching follows app_not_running', () => {
    expect(statusListener).toBeTypeOf('function');

    const offlineView = byId<HTMLElement>('view-offline');
    const searchingView = byId<HTMLElement>('view-searching');
    const pill = byId<HTMLSpanElement>('status-pill');
    const retry = byId<HTMLButtonElement>('btn-retry');

    // Step 1: offline view shown.
    push('app_not_running');
    expect(offlineView.hidden).toBe(false);
    expect(searchingView.hidden).toBe(true);

    // Step 2: background reconnect attempt fires a transient `searching`.
    // The offline guidance must NOT disappear.
    push('searching');
    expect(offlineView.hidden).toBe(false);
    expect(searchingView.hidden).toBe(true);
    // Pill reflects the reconnect attempt.
    expect(pill.textContent).toBe('○ Connecting…');
    // Retry button stays available.
    expect(retry.hidden).toBe(false);
  });

  it('switches to the import view when connected arrives after an offline+searching cycle', () => {
    expect(statusListener).toBeTypeOf('function');

    const offlineView = byId<HTMLElement>('view-offline');
    const importView = byId<HTMLElement>('view-import');

    // Simulate the full cycle: offline → searching reconnect → actually connected.
    push('app_not_running');
    push('searching');
    expect(offlineView.hidden).toBe(false);

    push('connected');
    expect(importView.hidden).toBe(false);
    expect(offlineView.hidden).toBe(true);
  });

  it('does not suppress the first searching spinner before offline has been shown', () => {
    expect(statusListener).toBeTypeOf('function');

    // After beforeEach pushed `connected`, hasShownOffline is false.
    // A searching push (first popup open, bridge connecting) should show the spinner.
    const searchingView = byId<HTMLElement>('view-searching');
    const offlineView = byId<HTMLElement>('view-offline');

    push('searching');
    expect(searchingView.hidden).toBe(false);
    expect(offlineView.hidden).toBe(true);
  });
});

// ── outdated-desktop view (v2 handshake force cutover) ──────────────────────────

describe('outdated-desktop view', () => {
  const statusListener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
    ((message: unknown) => void) | undefined;
  if (!statusListener) throw new Error('onMessage status listener not registered');
  const push = (phase: ConnectionStatus['phase']) =>
    statusListener({ ok: true, kind: 'status', status: { phase, port: null, hasToken: true } });

  it('shows the dedicated update view (NOT the pairing view) and the update pill', () => {
    push('outdated');

    const outdatedView = byId<HTMLElement>('view-outdated');
    const pairView = byId<HTMLElement>('view-pair');
    const importView = byId<HTMLElement>('view-import');
    const pill = byId<HTMLSpanElement>('status-pill');
    const retry = byId<HTMLButtonElement>('btn-retry');

    expect(outdatedView.hidden).toBe(false);
    // Critical: an outdated desktop is NOT a token problem — never show pairing.
    expect(pairView.hidden).toBe(true);
    expect(importView.hidden).toBe(true);
    expect(pill.textContent).toBe('⟳ Update the app');
    // Retry is available so the user can re-probe after updating the app.
    expect(retry.hidden).toBe(false);
  });
});

// ── appliedCheck auto-check (fire-and-forget on entering `connected`) ──────────

describe('appliedCheck auto-check', () => {
  const statusListener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
    ((message: unknown) => void) | undefined;
  if (!statusListener) throw new Error('onMessage status listener not registered');
  const push = (phase: ConnectionStatus['phase']) =>
    statusListener({ ok: true, kind: 'status', status: { phase, port: null, hasToken: true } });

  const flush = () => new Promise((r) => setTimeout(r, 0));

  beforeEach(() => {
    sendMessageMock.mockReset();
    // Force a non-connected phase first so the next `push('connected')` below is
    // a genuine transition regardless of what an earlier test left behind — the
    // auto-check only fires on ENTERING `connected`, not on a repeated push.
    push('searching');
  });

  it('sends an appliedCheck request and renders the found+applied status line with the relabeled button', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'applied', appliedAt: Date.UTC(2026, 5, 12) },
    });

    push('connected');
    await flush();

    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'appliedCheck' });
    const status = byId<HTMLParagraphElement>('applied-status');
    expect(status.hidden).toBe(false);
    expect(status.textContent).toContain('Already in your pipeline');
    expect(byId<HTMLButtonElement>('btn-import').textContent).toBe('Re-import / update');
  });

  it('renders nothing and keeps the default button label when not found', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: false },
    });

    push('connected');
    await flush();

    const status = byId<HTMLParagraphElement>('applied-status');
    expect(status.hidden).toBe(true);
    expect(byId<HTMLButtonElement>('btn-import').textContent).toBe('Import this job');
  });

  it('soft-fails silently (no status line, default label, no thrown error) when the request rejects', async () => {
    sendMessageMock.mockRejectedValueOnce(new Error('message channel closed'));

    push('connected');
    await flush();

    const status = byId<HTMLParagraphElement>('applied-status');
    expect(status.hidden).toBe(true);
    expect(byId<HTMLButtonElement>('btn-import').textContent).toBe('Import this job');
  });

  it('does not re-fire the check on a repeated connected push with no intervening phase change', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'saved' },
    });
    push('connected');
    await flush();
    expect(sendMessageMock).toHaveBeenCalledTimes(1);

    sendMessageMock.mockClear();
    push('connected'); // same phase again — not a transition
    await flush();
    expect(sendMessageMock).not.toHaveBeenCalled();
  });

  it('clears the stale status line + button label on leaving connected, with no flash before the next check resolves', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'applied', appliedAt: Date.UTC(2026, 5, 12) },
    });

    push('connected');
    await flush();

    const status = byId<HTMLParagraphElement>('applied-status');
    const btnImport = byId<HTMLButtonElement>('btn-import');
    expect(status.hidden).toBe(false);
    expect(btnImport.textContent).toBe('Re-import / update');

    // Desktop drops the connection — job A's stale line/label must not survive.
    push('app_not_running');
    expect(status.hidden).toBe(true);
    expect(status.textContent).toBe('');
    expect(btnImport.textContent).toBe('Import this job');

    // Reconnect for job B — before its own check resolves, the pre-resolve
    // state must already be clean (no lingering job-A text while it's in flight).
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'saved' },
    });
    push('connected');
    expect(status.hidden).toBe(true);
    expect(status.textContent).toBe('');
    expect(btnImport.textContent).toBe('Import this job');
  });
});
