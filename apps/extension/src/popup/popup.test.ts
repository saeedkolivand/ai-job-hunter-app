/**
 * Unit tests for the pure view-decision helpers exported from popup.ts, plus
 * the launcher's own wired-DOM behavior (PR0 §2: the page-context card, the
 * "?" menu, the notice line — the interactive Answer-tools UI itself moved to
 * the side panel's Answers tab, so its persistence/bootstrap tests moved with
 * it; only a passive count survives here, via {@link resolveAnswersNoticeLine}).
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

// vi.mock must come before the import that triggers the module side-effects.
// popup.ts imports @wxt-dev/browser; stub it out so the module-level
// side-effects (wire(), runtime listener registration) have a usable browser
// namespace. We also need a minimal DOM for the byId calls.

vi.mock('@wxt-dev/browser', () => ({
  browser: {
    runtime: {
      sendMessage: vi.fn(),
      onMessage: { addListener: vi.fn() },
      openOptionsPage: vi.fn(),
      getManifest: vi.fn(() => ({ version: '1.2.3' })),
    },
    // `query` resolves the tab id the shared answer state is keyed by (ADR-044)
    // — available without the `tabs` permission, which stays on the denylist.
    tabs: { create: vi.fn(), query: vi.fn(() => Promise.resolve([{ id: 7 }])) },
    storage: {
      session: { get: vi.fn(() => Promise.resolve({})), set: vi.fn(), remove: vi.fn() },
      // `lib/theme.ts`'s `bootTheme()` reads this at module load.
      local: { get: vi.fn(() => Promise.resolve({})), set: vi.fn(), remove: vi.fn() },
      onChanged: { addListener: vi.fn(), removeListener: vi.fn() },
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
    <div id="view-import" hidden></div>
    <!-- connection-status.ts mounts the pill/retry (with matching ids) into
         this host, and the four non-connected views into
         #connection-views-host, at module load — see
         connection-status.test.ts for that component's own unit tests. -->
    <div id="connection-pill-host"></div>
    <div id="connection-views-host"></div>
    <div id="job-card" hidden>
      <p id="job-card-title" hidden></p>
      <span id="applied-status" hidden></span>
      <button id="btn-mark-applied" hidden></button>
    </div>
    <!-- job-tools mounts its own Import/Check-fit/Fill DOM (with matching
         ids) into this host at module load, with hideSaveAnswers — see
         job-tools.test.ts for that component's own unit tests. -->
    <div id="job-tools-host"></div>
    <button id="btn-open-panel">Open the panel →</button>
    <p id="answers-notice" hidden></p>
    <p id="import-msg"></p>
    <div id="unpair-group" hidden>
      <button id="btn-unpair"></button>
    </div>
    <button id="btn-help" aria-expanded="false"></button>
    <div id="menu" hidden>
      <button id="menu-help">Help center</button>
      <button id="menu-settings">Settings</button>
      <button id="menu-about">About</button>
    </div>
    <p id="help-popover" hidden></p>
    <div id="about-popover" hidden>
      <p id="about-version"></p>
    </div>
  `;
}

buildPopupDom();

// Dynamic import AFTER DOM + mocks are in place. The module wires its DOM event
// listeners at load (wire()), so the behavioral tests below drive the controller
// by dispatching real clicks on the wired buttons and asserting DOM state.
const {
  resolveImportButtonLabel,
  resolveShowMarkAppliedButton,
  resolveMarkAppliedResponse,
  resolveAnswersNoticeLine,
  bootstrapNotice,
} = await import('./popup');

const sendMessageMock = vi.mocked(browser.runtime.sendMessage);
const byId = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

// `resolveStatusResponse` + the connection-status pill/retry/pairing/offline/
// outdated/searching behavior moved to `connection-status.ts` (ADR-046) — see
// `connection-status.test.ts` for those. `looksLikeToken` is still mocked
// above because `connection-status.ts` (which this file mounts for real, not
// mocked) imports it too.

// ── resolveImportButtonLabel ───────────────────────────────────────────────

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

// ── resolveAnswersNoticeLine (PR0 §2's passive notice, replacing the popup's
// own interactive Answer-tools rows) ────────────────────────────────────────

describe('resolveAnswersNoticeLine', () => {
  it('returns null when there is no state at all', () => {
    expect(resolveAnswersNoticeLine(null)).toBeNull();
  });

  it('returns null when every row is still empty', () => {
    const state = {
      tabId: 1,
      origin: 'https://jobs.example.com',
      scannedAt: 0,
      rows: [
        {
          id: 'a',
          question: 'Q',
          field: null,
          status: 'empty' as const,
          versions: [],
          selected: -1,
        },
      ],
      stream: null,
      pageChanged: false,
    };
    expect(resolveAnswersNoticeLine(state)).toBeNull();
  });

  it('counts rows that are not empty, singular phrasing for one', () => {
    const state = {
      tabId: 1,
      origin: 'https://jobs.example.com',
      scannedAt: 0,
      rows: [
        {
          id: 'a',
          question: 'Q1',
          field: null,
          status: 'drafted' as const,
          versions: [],
          selected: -1,
        },
        {
          id: 'b',
          question: 'Q2',
          field: null,
          status: 'empty' as const,
          versions: [],
          selected: -1,
        },
      ],
      stream: null,
      pageChanged: false,
    };
    expect(resolveAnswersNoticeLine(state)).toBe('1 answer ready on this page.');
  });

  it('uses plural phrasing for more than one', () => {
    const state = {
      tabId: 1,
      origin: 'https://jobs.example.com',
      scannedAt: 0,
      rows: [
        {
          id: 'a',
          question: 'Q1',
          field: null,
          status: 'filled' as const,
          versions: [],
          selected: -1,
        },
        {
          id: 'b',
          question: 'Q2',
          field: null,
          status: 'saved-available' as const,
          versions: [],
          selected: -1,
        },
      ],
      stream: null,
      pageChanged: false,
    };
    expect(resolveAnswersNoticeLine(state)).toBe('2 answers ready on this page.');
  });
});

// ── controller behavior (wired DOM) ───────────────────────────────────────────

describe('the "?" menu (#btn-help → Help center / Settings / About)', () => {
  beforeEach(() => {
    byId<HTMLElement>('menu').hidden = true;
    byId<HTMLElement>('help-popover').hidden = true;
    byId<HTMLElement>('about-popover').hidden = true;
    byId<HTMLButtonElement>('btn-help').setAttribute('aria-expanded', 'false');
  });

  it('opens the menu on click, closes it on a second click', () => {
    const btn = byId<HTMLButtonElement>('btn-help');

    btn.click();
    expect(byId<HTMLElement>('menu').hidden).toBe(false);
    expect(btn.getAttribute('aria-expanded')).toBe('true');

    btn.click();
    expect(byId<HTMLElement>('menu').hidden).toBe(true);
    expect(btn.getAttribute('aria-expanded')).toBe('false');
  });

  it('"Help center" swaps the menu for the existing help-popover content', () => {
    byId<HTMLButtonElement>('btn-help').click();
    byId<HTMLButtonElement>('menu-help').click();

    expect(byId<HTMLElement>('menu').hidden).toBe(true);
    expect(byId<HTMLElement>('help-popover').hidden).toBe(false);
  });

  it('"Settings" opens the options page and closes the menu', () => {
    byId<HTMLButtonElement>('btn-help').click();
    byId<HTMLButtonElement>('menu-settings').click();

    expect(browser.runtime.openOptionsPage).toHaveBeenCalled();
    expect(byId<HTMLElement>('menu').hidden).toBe(true);
  });

  it('"About" shows the version line from the manifest', () => {
    byId<HTMLButtonElement>('btn-help').click();
    byId<HTMLButtonElement>('menu-about').click();

    expect(byId<HTMLElement>('about-popover').hidden).toBe(false);
    expect(byId<HTMLElement>('about-version').textContent).toContain('1.2.3');
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

  it('shows view-import only for connected, and hides it (resetting job/job-card) otherwise', () => {
    const importView = byId<HTMLElement>('view-import');

    push('connected');
    expect(importView.hidden).toBe(false);

    push('app_not_running');
    expect(importView.hidden).toBe(true);
    expect(byId<HTMLElement>('job-card').hidden).toBe(true);
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

  it('sends an appliedCheck request and renders the job-card chip with the relabeled button', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'applied', appliedAt: Date.UTC(2026, 5, 12) },
    });

    push('connected');
    await flush();

    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'appliedCheck' });
    expect(byId<HTMLElement>('job-card').hidden).toBe(false);
    const status = byId<HTMLSpanElement>('applied-status');
    expect(status.hidden).toBe(false);
    expect(status.textContent).toMatch(/^Applied /);
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

    expect(byId<HTMLElement>('job-card').hidden).toBe(true);
    expect(byId<HTMLButtonElement>('btn-import').textContent).toBe('Import this job');
  });

  it('soft-fails silently (card stays hidden, default label, no thrown error) when the request rejects', async () => {
    sendMessageMock.mockRejectedValueOnce(new Error('message channel closed'));

    push('connected');
    await flush();

    expect(byId<HTMLElement>('job-card').hidden).toBe(true);
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
    // Entering `connected` fires three fire-and-forget auto-checks — appliedCheck,
    // fieldsProbe (Form group gating) and answerScan (the notice line's data).
    expect(sendMessageMock).toHaveBeenCalledTimes(3);
    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'answerScan' });
    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'fieldsProbe' });

    sendMessageMock.mockClear();
    push('connected'); // same phase again — not a transition
    await flush();
    expect(sendMessageMock).not.toHaveBeenCalled();
  });

  it('clears the stale card + button label on leaving connected, with no flash before the next check resolves', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'applied', appliedAt: Date.UTC(2026, 5, 12) },
    });

    push('connected');
    await flush();

    const jobCard = byId<HTMLElement>('job-card');
    const btnImport = byId<HTMLButtonElement>('btn-import');
    const btnMarkApplied = byId<HTMLButtonElement>('btn-mark-applied');
    expect(jobCard.hidden).toBe(false);
    expect(btnImport.textContent).toBe('Re-import / update');
    expect(btnMarkApplied.hidden).toBe(true); // job A is already applied

    // Desktop drops the connection — job A's stale card must not survive.
    push('app_not_running');
    expect(jobCard.hidden).toBe(true);
    expect(btnImport.textContent).toBe('Import this job');
    expect(btnMarkApplied.hidden).toBe(true);

    // Reconnect for job B — before its own check resolves, the pre-resolve
    // state must already be clean (no lingering job-A content while it's in flight).
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'appliedCheck',
      result: { found: true, status: 'saved' },
    });
    push('connected');
    expect(jobCard.hidden).toBe(true);
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

    const jobCardTitle = byId<HTMLParagraphElement>('job-card-title');
    const status = byId<HTMLSpanElement>('applied-status');
    const btnImport = byId<HTMLButtonElement>('btn-import');
    expect(jobCardTitle.textContent).toBe('Job B');
    expect(status.textContent).toBe('Saved');
    expect(btnImport.textContent).toBe('Re-import / update');

    // Check A finally resolves late (found:false for job A) — it must NOT
    // overwrite the already-rendered job B result.
    resolveA?.({ ok: true, kind: 'appliedCheck', result: { found: false } });
    await flush();

    expect(jobCardTitle.textContent).toBe('Job B');
    expect(status.textContent).toBe('Saved');
    expect(btnImport.textContent).toBe('Re-import / update');
  });
});

// ── fieldsProbe auto-check (fire-and-forget on entering `connected`) ──────────
// Gates the Form group (#group-form) on "does this page have fillable form
// fields?". Runs ALONGSIDE the appliedCheck auto-check above on the SAME
// transition — the first queued sendMessage response answers appliedCheck
// (code calls it first), the second answers fieldsProbe. Unlike before this
// redesign there is no Answer-tools disclosure in the popup to gate anymore
// (`onAnswerToolsVisibility` is intentionally omitted) — only #group-form.

describe('fieldsProbe auto-check (#group-form gating)', () => {
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

  it('shows the Form group when the probe finds fillable fields', async () => {
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

  it('hides the Form group when the probe finds no fillable fields at all', async () => {
    sendMessageMock.mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK).mockResolvedValueOnce({
      ok: true,
      kind: 'fieldsProbe',
      hasFormFields: false,
      hasAnswerFields: false,
    });

    push('connected');
    await flush();

    expect(byId<HTMLElement>('group-form').hidden).toBe(true);
  });

  it('fails OPEN (shows the Form group) when the probe request rejects', async () => {
    sendMessageMock
      .mockResolvedValueOnce(NEUTRAL_APPLIED_CHECK)
      .mockRejectedValueOnce(new Error('message channel closed'));

    push('connected');
    await flush();

    expect(byId<HTMLElement>('group-form').hidden).toBe(false);
  });

  it('re-shows the Form group on a fresh page after a previous page hid it (no stale hide across a reconnect)', async () => {
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

// ── bootstrapNotice (resolves the active tab, subscribes the notice line) ────
// Replaces the old bootstrapAnswerTools now that the interactive rows live
// only in the panel's Answers tab — this popup only reports a passive count.

describe('bootstrapNotice', () => {
  beforeEach(() => {
    sendMessageMock.mockReset();
    byId<HTMLElement>('answers-notice').hidden = true;
    byId<HTMLElement>('answers-notice').textContent = '';
  });

  it('subscribes the notice line to the shared answer state instead of querying for it', async () => {
    // The stream lives in the shared per-tab state (ADR-044 decision 1) — a
    // query would go stale the moment the panel changed it, which subscribing
    // avoids.
    const addListener = vi.mocked(browser.storage.onChanged.addListener);
    addListener.mockClear();

    await bootstrapNotice();

    expect(addListener).toHaveBeenCalled();
  });

  it('leaves the notice line hidden rather than throwing when no tab id can be read', async () => {
    vi.mocked(browser.tabs.query).mockResolvedValueOnce([]);

    await expect(bootstrapNotice()).resolves.toBeUndefined();
    expect(byId<HTMLElement>('answers-notice').hidden).toBe(true);
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
    await bootstrapNotice(); // (re)resolves activeTabId from the tabs.query mock (id 7)

    byId<HTMLButtonElement>('btn-open-panel').click();

    expect(open).toHaveBeenCalledWith({ tabId: 7 });
  });

  it('renders the launcher label (PR0 §2)', () => {
    expect(byId<HTMLButtonElement>('btn-open-panel').textContent?.trim()).toBe('Open the panel →');
  });

  it('falls back to browser.sidebarAction.open() when there is no sidePanel API (Firefox)', async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    mutableBrowser.sidebarAction = { open };
    await bootstrapNotice();

    byId<HTMLButtonElement>('btn-open-panel').click();

    expect(open).toHaveBeenCalledTimes(1);
  });

  it('surfaces a message rather than throwing when neither API is available', async () => {
    await bootstrapNotice();
    byId<HTMLElement>('import-msg').textContent = '';

    byId<HTMLButtonElement>('btn-open-panel').click();

    expect(byId<HTMLElement>('import-msg').textContent).toContain('no side panel');
  });

  it('reports the unresolved tab, not a false "no side panel", when sidePanel exists but activeTabId has not resolved yet (regression)', async () => {
    const open = vi.fn().mockResolvedValue(undefined);
    mutableBrowser.sidePanel = { open };
    // No tab from `tabs.query` this time — `activeTabId` stays `null`.
    vi.mocked(browser.tabs.query).mockResolvedValueOnce([]);
    await bootstrapNotice();
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
