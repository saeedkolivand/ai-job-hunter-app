/**
 * Unit tests for the pure view-decision helpers exported from popup.ts.
 *
 * popup.ts runs side-effects at module load (DOM queries via byId, wire(),
 * refreshStatusWithTimeout()).  To keep tests light we import only the PURE
 * exported functions directly — they have zero DOM dependency and zero
 * browser-API calls, so no DOM scaffolding and no @wxt-dev/browser mock
 * are strictly required for the assertions here.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { browser } from '@wxt-dev/browser';

import type { ConnectionStatus } from '../lib/messages';
import { getAnswerToolsExpanded, setAnswerToolsExpanded } from '../lib/storage';

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
    // `query` resolves the tab id the shared answer state is keyed by (ADR-044)
    // — available without the `tabs` permission, which stays on the denylist.
    tabs: { create: vi.fn(), query: vi.fn(() => Promise.resolve([{ id: 7 }])) },
    storage: {
      session: { get: vi.fn(() => Promise.resolve({})), set: vi.fn(), remove: vi.fn() },
      onChanged: { addListener: vi.fn(), removeListener: vi.fn() },
    },
  },
}));

vi.mock('../lib/storage', () => ({
  looksLikeToken: vi.fn(() => false),
  // Default: collapsed, matching the fresh-install default (no stored value
  // yet) — individual tests override via mockResolvedValueOnce as needed.
  getAnswerToolsExpanded: vi.fn(() => Promise.resolve(false)),
  setAnswerToolsExpanded: vi.fn(() => Promise.resolve(undefined)),
}));

// Build the minimal DOM that popup.ts queries at module load (byId calls).
// Must happen before the dynamic import below so jsdom has the elements when
// the module-level `els` constant is initialised.
function buildPopupDom(): void {
  document.body.innerHTML = `
    <div id="view-import" hidden></div>
    <!-- connection-status.ts mounts the pill/retry (with matching ids) into
         this host, and the four non-connected views into
         #connection-views-host, at module load — see
         connection-status.test.ts for that component's own unit tests. -->
    <div id="connection-pill-host"></div>
    <div id="connection-views-host"></div>
    <button id="btn-mark-applied" hidden></button>
    <!-- job-tools mounts its own Import/Check-fit/Fill/Save-answers DOM
         (with matching ids) into this host at module load — see
         job-tools.test.ts for that component's own unit tests. -->
    <div id="job-tools-host"></div>
    <details id="answer-tools">
      <summary id="answer-tools-summary">Answer tools</summary>
      <div id="answer-tools-host"></div>
      <button id="btn-open-panel">Open AI Job Hunter answer tool</button>
    </details>
    <p id="applied-status" hidden></p>
    <p id="import-msg"></p>
    <div id="unpair-group" hidden>
      <button id="btn-unpair"></button>
    </div>
    <button id="btn-help"></button>
    <p id="help-popover" hidden></p>
  `;
}

buildPopupDom();

// Dynamic import AFTER DOM + mocks are in place. The module wires its DOM event
// listeners at load (wire()), so the behavioral tests below drive the controller
// by dispatching real clicks on the wired buttons and asserting DOM state.
const {
  resolveAppliedStatusLine,
  resolveImportButtonLabel,
  resolveShowMarkAppliedButton,
  resolveMarkAppliedResponse,
  bootstrapAnswerTools,
} = await import('./popup');

const sendMessageMock = vi.mocked(browser.runtime.sendMessage);
const getAnswerToolsExpandedMock = vi.mocked(getAnswerToolsExpanded);
const setAnswerToolsExpandedMock = vi.mocked(setAnswerToolsExpanded);
const byId = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

// `resolveStatusResponse` + the connection-status pill/retry/pairing/offline/
// outdated/searching behavior moved to `connection-status.ts` (ADR-046) — see
// `connection-status.test.ts` for those. `looksLikeToken` is still mocked
// above because `connection-status.ts` (which this file mounts for real, not
// mocked) imports it too.

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

  it('includes the year in the applied date when it differs from the current year', () => {
    // Relative to the current year so this never rots — June avoids any
    // UTC/local timezone day-boundary rollover into a different year.
    const priorYear = new Date().getFullYear() - 1;
    const appliedAt = Date.UTC(priorYear, 5, 12);
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: true, status: 'applied', appliedAt },
    };
    expect(resolveAppliedStatusLine(res)).toMatch(
      new RegExp(`^Already in your pipeline — applied .+\\b${priorYear}\\.$`)
    );
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

// ── resolveShowMarkAppliedButton ───────────────────────────────────────────────

describe('resolveShowMarkAppliedButton', () => {
  it('returns false for a non-appliedCheck response', () => {
    const res = { ok: true as const, kind: 'token' as const };
    expect(resolveShowMarkAppliedButton(res)).toBe(false);
  });

  it('returns false when ok is false', () => {
    const res = { ok: false as const, error: 'boom' };
    expect(resolveShowMarkAppliedButton(res)).toBe(false);
  });

  it('returns false when not found', () => {
    const res = { ok: true as const, kind: 'appliedCheck' as const, result: { found: false } };
    expect(resolveShowMarkAppliedButton(res)).toBe(false);
  });

  it('returns false when the result carries an error', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: false, error: 'malformed' },
    };
    expect(resolveShowMarkAppliedButton(res)).toBe(false);
  });

  it('returns true for a found + saved result', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: true, status: 'saved' },
    };
    expect(resolveShowMarkAppliedButton(res)).toBe(true);
  });

  it('returns false for a found result with no status (CAS precondition requires an explicit saved status)', () => {
    const res = { ok: true as const, kind: 'appliedCheck' as const, result: { found: true } };
    expect(resolveShowMarkAppliedButton(res)).toBe(false);
  });

  it('returns false for a found + already-applied result', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: true, status: 'applied' },
    };
    expect(resolveShowMarkAppliedButton(res)).toBe(false);
  });

  it('returns false for a found + mid-pipeline result', () => {
    const res = {
      ok: true as const,
      kind: 'appliedCheck' as const,
      result: { found: true, status: 'interviewing' },
    };
    expect(resolveShowMarkAppliedButton(res)).toBe(false);
  });
});

// ── resolveMarkAppliedResponse ─────────────────────────────────────────────────

describe('resolveMarkAppliedResponse', () => {
  it('surfaces a transport-level error (unlike the passive appliedCheck fold)', () => {
    const res = { ok: false as const, error: 'Desktop app not reachable.' };
    const { text, tone } = resolveMarkAppliedResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe('Desktop app not reachable.');
  });

  it('returns the unexpected-response error when kind is not statusUpdate', () => {
    const res = { ok: true as const, kind: 'token' as const };
    const { text, tone } = resolveMarkAppliedResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe('Unexpected response — please retry.');
  });

  it('surfaces the desktop refusal text when result.ok is false', () => {
    const res = {
      ok: true as const,
      kind: 'statusUpdate' as const,
      result: { ok: false, error: "couldn't find a saved job for this page" },
    };
    const { text, tone } = resolveMarkAppliedResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe("couldn't find a saved job for this page");
  });

  it('falls back to a generic refusal message when result.ok is false with no error text', () => {
    const res = { ok: true as const, kind: 'statusUpdate' as const, result: { ok: false } };
    const { text, tone } = resolveMarkAppliedResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe('Could not mark this job as applied.');
  });

  it('reports success when result.ok is true', () => {
    const res = {
      ok: true as const,
      kind: 'statusUpdate' as const,
      result: { ok: true, applicationId: 'app-1', status: 'applied' },
    };
    const { text, tone } = resolveMarkAppliedResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('Marked as applied.');
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

// The pill/retry/pairing/offline/outdated/searching behavior — savePairing,
// "get the app", header Retry visibility, offline-sticky, the outdated-desktop
// view — all moved to `connection-status.ts` (ADR-046); see
// `connection-status.test.ts` for those. What's left here is popup.ts's OWN
// contract with that module: `view-import` (and the "Unpair this device"
// group) toggle correctly off a real status push through the SAME module.

describe('view-import + unpair-group gating (via the real connection-status module)', () => {
  const statusListener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
    ((message: unknown) => void) | undefined;
  if (!statusListener) throw new Error('onMessage status listener not registered');
  const push = (phase: ConnectionStatus['phase'], hasToken = true) =>
    statusListener({ ok: true, kind: 'status', status: { phase, port: null, hasToken } });

  it('shows view-import only for connected, and hides it (resetting job/answer tools) otherwise', () => {
    const importView = byId<HTMLElement>('view-import');

    push('connected');
    expect(importView.hidden).toBe(false);

    push('app_not_running');
    expect(importView.hidden).toBe(true);
  });

  it('shows "Unpair this device" only while a pairing token is stored, independent of phase', () => {
    push('not_paired', false);
    expect(byId<HTMLElement>('unpair-group').hidden).toBe(true);

    push('connected', true);
    expect(byId<HTMLElement>('unpair-group').hidden).toBe(false);

    push('app_not_running', false);
    expect(byId<HTMLElement>('unpair-group').hidden).toBe(true);
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
    // Already applied — the mark-applied button has nothing left to do.
    expect(byId<HTMLButtonElement>('btn-mark-applied').hidden).toBe(true);
  });

  it('shows the mark-applied button for a found+saved result', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'saved' },
    });

    push('connected');
    await flush();

    expect(byId<HTMLButtonElement>('btn-mark-applied').hidden).toBe(false);
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
    // Entering `connected` fires the three fire-and-forget auto-checks —ppliedCheck
    // and fieldsProbe (see the sibling `fieldsProbe auto-check` describe block).
    expect(sendMessageMock).toHaveBeenCalledTimes(3);
    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'answerScan' });
    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'fieldsProbe' });

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
    const btnMarkApplied = byId<HTMLButtonElement>('btn-mark-applied');
    expect(status.hidden).toBe(false);
    expect(btnImport.textContent).toBe('Re-import / update');
    expect(btnMarkApplied.hidden).toBe(true); // job A is already applied

    // Desktop drops the connection — job A's stale line/label must not survive.
    push('app_not_running');
    expect(status.hidden).toBe(true);
    expect(status.textContent).toBe('');
    expect(btnImport.textContent).toBe('Import this job');
    expect(btnMarkApplied.hidden).toBe(true);

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
    expect(btnMarkApplied.hidden).toBe(true);
    await flush();
    // Job B's check resolves as found+saved — the button appears for it.
    expect(btnMarkApplied.hidden).toBe(false);
  });

  it('ignores a stale in-flight response that resolves after a newer check has already rendered', async () => {
    // Check A starts on entering `connected` for job A, but its response never
    // resolves yet (simulates it still being in flight when a reconnect fires).
    let resolveA: ((res: unknown) => void) | undefined;
    const pendingA = new Promise((resolve) => {
      resolveA = resolve;
    });
    sendMessageMock.mockReturnValueOnce(pendingA);
    push('connected');

    // Disconnect → reconnect: a fresh, edge-triggered check B starts for job B
    // and resolves before A does.
    push('app_not_running');
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'saved', title: 'Job B' },
    });
    push('connected');
    await flush();

    const status = byId<HTMLParagraphElement>('applied-status');
    const btnImport = byId<HTMLButtonElement>('btn-import');
    expect(status.textContent).toBe('“Job B” is saved in your pipeline.');
    expect(btnImport.textContent).toBe('Re-import / update');

    // Check A finally resolves late (found:false for job A) — it must NOT
    // overwrite the already-rendered job B result.
    resolveA?.({ ok: true, kind: 'appliedCheck', result: { found: false } });
    await flush();

    expect(status.textContent).toBe('“Job B” is saved in your pipeline.');
    expect(btnImport.textContent).toBe('Re-import / update');
  });
});

// ── fieldsProbe auto-check (fire-and-forget on entering `connected`) ──────────
// Gates the Form group (#group-form) + the Answer-tools disclosure
// (#answer-tools) on "does this page have fillable form fields?". Runs
// ALONGSIDE the appliedCheck auto-check above on the SAME transition — the
// first queued sendMessage response answers appliedCheck (code calls it
// first), the second answers fieldsProbe.

describe('fieldsProbe auto-check (Form group + Answer-tools gating)', () => {
  const statusListener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
    ((message: unknown) => void) | undefined;
  if (!statusListener) throw new Error('onMessage status listener not registered');
  const push = (phase: ConnectionStatus['phase']) =>
    statusListener({ ok: true, kind: 'status', status: { phase, port: null, hasToken: true } });

  const flush = () => new Promise((r) => setTimeout(r, 0));

  const NEUTRAL_APPLIED_CHECK = {
    ok: true as const,
    kind: 'appliedCheck' as const,
    result: { found: false },
  };

  beforeEach(() => {
    sendMessageMock.mockReset();
    // Force a genuine transition for the next push('connected') below.
    push('searching');
  });

  it('shows the Form group + Answer-tools disclosure when the probe finds fillable fields', async () => {
    sendMessageMock.mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK).mockResolvedValueOnce({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: true,
      hasAnswerFields: true,
    });

    push('connected');
    await flush();

    expect(byId<HTMLElement>('group-form').hidden).toBe(false);
    expect(byId<HTMLDetailsElement>('answer-tools').hidden).toBe(false);
  });

  it('hides the Form group + Answer-tools disclosure when the probe finds no fillable fields at all', async () => {
    sendMessageMock.mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK).mockResolvedValueOnce({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: false,
      hasAnswerFields: false,
    });

    push('connected');
    await flush();

    expect(byId<HTMLElement>('group-form').hidden).toBe(true);
    expect(byId<HTMLDetailsElement>('answer-tools').hidden).toBe(true);
  });

  it('shows the Form group but hides Answer tools for an IDENTITY-ONLY form (name/email/phone) — the union vs. narrower-signal split', async () => {
    sendMessageMock.mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK).mockResolvedValueOnce({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: true, // hasAutofillableFields matched name/email/phone
      hasAnswerFields: false, // hasAnswerCapturableFields excludes identity fields
    });

    push('connected');
    await flush();

    expect(byId<HTMLElement>('group-form').hidden).toBe(false);
    expect(byId<HTMLDetailsElement>('answer-tools').hidden).toBe(true);
  });

  it('fails OPEN (shows both groups) when the probe request rejects', async () => {
    sendMessageMock
      .mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK)
      .mockRejectedValueOnce(new Error('message channel closed'));

    push('connected');
    await flush();

    expect(byId<HTMLElement>('group-form').hidden).toBe(false);
    expect(byId<HTMLDetailsElement>('answer-tools').hidden).toBe(false);
  });

  it('re-shows both groups on a fresh page after a previous page hid them (no stale hide across a reconnect)', async () => {
    sendMessageMock.mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK).mockResolvedValueOnce({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: false,
      hasAnswerFields: false,
    });
    push('connected');
    await flush();
    expect(byId<HTMLElement>('group-form').hidden).toBe(true);

    // Disconnect (leaving `connected` resets to the fail-open default) then
    // reconnect for a fresh page whose own probe hasn't resolved yet.
    push('app_not_running');
    expect(byId<HTMLElement>('group-form').hidden).toBe(false);

    sendMessageMock.mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK).mockResolvedValueOnce({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: true,
      hasAnswerFields: true,
    });
    push('connected');
    await flush();
    expect(byId<HTMLElement>('group-form').hidden).toBe(false);
  });

  it('a stale hasFields:false response that resolves AFTER leaving connected must not hide the groups again (generation invalidated on disconnect)', async () => {
    // The probe starts on entering `connected` but its response never
    // resolves yet (simulates it still being in flight when the user
    // disconnects before it settles).
    let resolveProbe: ((res: unknown) => void) | undefined;
    const pendingProbe = new Promise((resolve) => {
      resolveProbe = resolve;
    });
    sendMessageMock.mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK).mockReturnValueOnce(pendingProbe);
    push('connected');
    await flush();
    expect(byId<HTMLElement>('group-form').hidden).toBe(false);

    // Leave `connected` BEFORE the probe resolves — this must invalidate it.
    push('app_not_running');
    expect(byId<HTMLElement>('group-form').hidden).toBe(false);
    expect(byId<HTMLDetailsElement>('answer-tools').hidden).toBe(false);

    // The stale probe finally resolves as "no fields" — must be a no-op now.
    resolveProbe?.({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: false,
      hasAnswerFields: false,
    });
    await flush();

    expect(byId<HTMLElement>('group-form').hidden).toBe(false);
    expect(byId<HTMLDetailsElement>('answer-tools').hidden).toBe(false);
  });
});

// ── doMarkApplied (#btn-mark-applied) ─────────────────────────────────────────

describe('doMarkApplied (#btn-mark-applied)', () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));

  beforeEach(() => {
    sendMessageMock.mockReset();
    byId<HTMLButtonElement>('btn-mark-applied').hidden = false;
    byId<HTMLButtonElement>('btn-mark-applied').disabled = false;
    byId<HTMLParagraphElement>('import-msg').textContent = '';
  });

  it('shows "Marking as applied…" then re-fires the auto-check on success, hiding the button', async () => {
    sendMessageMock
      .mockResolvedValueOnce({
        ok: true,
        kind: 'statusUpdate',
        result: { ok: true, applicationId: 'app-1', status: 'applied' },
      })
      // The success-path re-fire of runAppliedAutoCheck sends a SECOND
      // request — the same generation-guarded path every other render goes
      // through, never a hand-rolled DOM update.
      .mockResolvedValueOnce({
        ok: true,
        kind: 'appliedCheck',
        result: { found: true, status: 'applied' },
      });

    const btn = byId<HTMLButtonElement>('btn-mark-applied');
    btn.click();
    expect(btn.disabled).toBe(true);
    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe('Marking as applied…');

    await flush();
    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe('Marked as applied.');
    expect(sendMessageMock).toHaveBeenNthCalledWith(1, { kind: 'statusUpdate' });
    expect(sendMessageMock).toHaveBeenNthCalledWith(2, { kind: 'appliedCheck' });
    // The re-fired auto-check's found+applied result hides the button.
    expect(btn.hidden).toBe(true);
  });

  it('surfaces the desktop refusal text and re-enables the button (errors ARE shown, unlike the passive check)', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'statusUpdate',
      result: { ok: false, error: "couldn't find a saved job for this page" },
    });

    const btn = byId<HTMLButtonElement>('btn-mark-applied');
    btn.click();
    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      "couldn't find a saved job for this page"
    );
    expect(btn.disabled).toBe(false);
    // No auto-check re-fire on failure — only one request went out.
    expect(sendMessageMock).toHaveBeenCalledTimes(1);
  });

  it('shows a retry message and re-enables the button when sendMessage rejects', async () => {
    sendMessageMock.mockRejectedValueOnce(new Error('message channel closed'));

    const btn = byId<HTMLButtonElement>('btn-mark-applied');
    btn.click();
    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      'Could not mark this job as applied. Please retry.'
    );
    expect(btn.disabled).toBe(false);
  });
});

// ── Answer-tools expand/collapse (#answer-tools <details>) ──────────────────
// Collapsed by default; the expand/collapse state persists via
// chrome.storage.local (a UI boolean, not PII/job data); a buffered/still-
// running stream always wins over a persisted "collapsed" preference.

describe('answer-tools persistence (toggle → storage)', () => {
  beforeEach(() => {
    setAnswerToolsExpandedMock.mockClear();
  });

  it('persists the CURRENT open state when the disclosure is toggled', () => {
    const details = byId<HTMLDetailsElement>('answer-tools');
    details.open = true;
    details.dispatchEvent(new Event('toggle'));

    expect(setAnswerToolsExpandedMock).toHaveBeenCalledWith(true);
  });

  it('persists collapsed the same way', () => {
    const details = byId<HTMLDetailsElement>('answer-tools');
    details.open = false;
    details.dispatchEvent(new Event('toggle'));

    expect(setAnswerToolsExpandedMock).toHaveBeenCalledWith(false);
  });
});

describe('bootstrapAnswerTools (applies the persisted preference, then subscribes)', () => {
  beforeEach(() => {
    sendMessageMock.mockReset();
    getAnswerToolsExpandedMock.mockReset();
    byId<HTMLDetailsElement>('answer-tools').open = false;
  });

  it('defaults to collapsed when no preference has been stored', async () => {
    getAnswerToolsExpandedMock.mockResolvedValueOnce(false);
    byId<HTMLDetailsElement>('answer-tools').open = true; // start non-default to prove it applies

    await bootstrapAnswerTools();

    expect(byId<HTMLDetailsElement>('answer-tools').open).toBe(false);
  });

  it('applies a persisted "expanded" preference', async () => {
    getAnswerToolsExpandedMock.mockResolvedValueOnce(true);

    await bootstrapAnswerTools();

    expect(byId<HTMLDetailsElement>('answer-tools').open).toBe(true);
  });

  it('subscribes the Answer-tools section to the shared state instead of querying for it', async () => {
    // The old popup-open reattach asked the background "what is buffered?".
    // The stream now lives in the shared per-tab state, so the popup must
    // SUBSCRIBE rather than ask — a query would go stale the moment the panel
    // (or the next chunk) changed it, which is the drift ADR-044 decision 1
    // exists to prevent.
    getAnswerToolsExpandedMock.mockResolvedValueOnce(false);
    const addListener = vi.mocked(browser.storage.onChanged.addListener);
    addListener.mockClear();

    await bootstrapAnswerTools();

    expect(addListener).toHaveBeenCalled();
    expect(sendMessageMock).not.toHaveBeenCalledWith({ kind: 'answerAssistProgress' });
  });

  it('renders the empty state rather than throwing when no tab id can be read', async () => {
    getAnswerToolsExpandedMock.mockResolvedValueOnce(false);
    vi.mocked(browser.tabs.query).mockResolvedValueOnce([]);

    await expect(bootstrapAnswerTools()).resolves.toBeUndefined();
    expect(byId<HTMLElement>('answer-tools-host').textContent).toContain('Nothing scanned yet');
  });
});

// ── openAnswerPanel (#btn-open-panel, ADR-044 decision 10a) ─────────────────
// Neither `sidePanel` nor `sidebarAction` is on the shared browser mock (most
// tests need neither), so each test here adds only the ONE the browser under
// test would expose, and removes it afterwards — proving the click handler
// picks the right API rather than assuming Chrome.

describe('openAnswerPanel (#btn-open-panel)', () => {
  type MutableBrowser = typeof browser & {
    sidePanel?: { open: (o: { tabId: number }) => Promise<void> };
    sidebarAction?: { open: () => Promise<void> };
  };
  const mutableBrowser = browser as MutableBrowser;

  afterEach(() => {
    delete mutableBrowser.sidePanel;
    delete mutableBrowser.sidebarAction;
  });

  it('calls chrome.sidePanel.open with the active tab id, synchronously from the click', async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    mutableBrowser.sidePanel = { open };
    await bootstrapAnswerTools(); // (re)resolves activeTabId from the tabs.query mock (id 7)

    byId<HTMLButtonElement>('btn-open-panel').click();

    expect(open).toHaveBeenCalledWith({ tabId: 7 });
  });

  it('renders the label shared with the context-menu entry of the same name', () => {
    expect(byId<HTMLButtonElement>('btn-open-panel').textContent?.trim()).toBe(
      'Open AI Job Hunter answer tool'
    );
  });

  it('falls back to browser.sidebarAction.open() when there is no sidePanel API (Firefox)', async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    mutableBrowser.sidebarAction = { open };
    await bootstrapAnswerTools();

    byId<HTMLButtonElement>('btn-open-panel').click();

    expect(open).toHaveBeenCalledTimes(1);
  });

  it('surfaces a message rather than throwing when neither API is available', async () => {
    await bootstrapAnswerTools();
    byId<HTMLElement>('import-msg').textContent = '';

    byId<HTMLButtonElement>('btn-open-panel').click();

    expect(byId<HTMLElement>('import-msg').textContent).toContain('no side panel');
  });

  it('reports the unresolved tab, not a false "no side panel", when sidePanel exists but activeTabId has not resolved yet (regression)', async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    mutableBrowser.sidePanel = { open };
    // No tab from `tabs.query` this time — `activeTabId` stays `null`.
    vi.mocked(browser.tabs.query).mockResolvedValueOnce([]);
    await bootstrapAnswerTools();
    byId<HTMLElement>('import-msg').textContent = '';

    byId<HTMLButtonElement>('btn-open-panel').click();

    // Chrome DOES have a side panel here — the true cause is the unresolved
    // tab id, and the message must say so instead of the Firefox-shaped "this
    // browser has no side panel" line, which is false on this browser.
    expect(open).not.toHaveBeenCalled();
    expect(byId<HTMLElement>('import-msg').textContent).not.toContain('no side panel');
    expect(byId<HTMLElement>('import-msg').textContent).toContain('this tab');
  });
});

// ── unpair (#btn-unpair, now reachable via the "?" help popover) ────────────
// Moved off the import view's standing row in popup.html — the click handler
// itself is unchanged/unaffected by that relocation.

describe('unpair (#btn-unpair, #unpair-group hasToken-gated)', () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));
  const statusListener = vi.mocked(browser.runtime.onMessage.addListener).mock.calls[0]?.[0] as
    ((message: unknown) => void) | undefined;
  if (!statusListener) throw new Error('onMessage status listener not registered');
  const push = (hasToken: boolean, phase: ConnectionStatus['phase'] = 'not_paired') =>
    statusListener({ ok: true, kind: 'status', status: { phase, port: null, hasToken } });

  it('clears the token and returns to the pairing view', async () => {
    sendMessageMock.mockReset();
    sendMessageMock
      .mockResolvedValueOnce({ ok: true, kind: 'token' }) // clearToken
      .mockResolvedValueOnce({
        ok: true,
        kind: 'status',
        status: { phase: 'not_paired', port: 1, hasToken: false },
      });
    byId<HTMLElement>('view-pair').hidden = true;

    byId<HTMLButtonElement>('btn-unpair').click();
    await flush();

    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'clearToken' });
    expect(byId<HTMLElement>('view-pair').hidden).toBe(false);
  });

  it('shows the "Unpair this device" group only while a pairing token is stored', () => {
    push(false);
    expect(byId<HTMLElement>('unpair-group').hidden).toBe(true);

    push(true, 'connected');
    expect(byId<HTMLElement>('unpair-group').hidden).toBe(false);

    push(false, 'app_not_running');
    expect(byId<HTMLElement>('unpair-group').hidden).toBe(true);
  });
});
