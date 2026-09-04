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
import { getAnswerToolsExpanded, looksLikeToken, setAnswerToolsExpanded } from '../lib/storage';

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
    <span id="status-pill"></span>
    <div id="view-import" hidden></div>
    <div id="view-pair" hidden></div>
    <div id="view-offline" hidden></div>
    <div id="view-searching"></div>
    <button id="btn-import"></button>
    <button id="btn-fill"></button>
    <button id="btn-mark-applied" hidden></button>
    <section id="group-form">
      <button id="btn-save-answers"></button>
    </section>
    <details id="answer-tools">
      <summary id="answer-tools-summary">Answer tools</summary>
      <div id="answer-tools-host"></div>
      <button id="btn-open-panel"></button>
    </details>
    <button id="btn-check-fit"></button>
    <div id="match-result" hidden></div>
    <p id="applied-status" hidden></p>
    <input id="chk-applied" type="checkbox" />
    <p id="import-msg"></p>
    <div id="unpair-group" hidden>
      <button id="btn-unpair"></button>
    </div>
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
  resolveShowMarkAppliedButton,
  resolveMarkAppliedResponse,
  resolveAnswersSaveResponse,
  resolveMatchLiveResponse,
  resolveFieldsProbeResponse,
  bootstrapAnswerTools,
} = await import('./popup');

const sendMessageMock = vi.mocked(browser.runtime.sendMessage);
const looksLikeTokenMock = vi.mocked(looksLikeToken);
const getAnswerToolsExpandedMock = vi.mocked(getAnswerToolsExpanded);
const setAnswerToolsExpandedMock = vi.mocked(setAnswerToolsExpanded);
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

  // ── matchScore percent-fit suffix (best-effort, omitted on failure) ─────────

  it('appends the percent-fit suffix to a plain success when matchScore is present', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: {
        applicationId: 'app-score',
        status: 'saved',
        title: 'Rust Engineer',
        matchScore: 71.6,
      },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe(
      'Imported “Rust Engineer”. Open AI Job Hunter → Applications to view it. — 72% fit.'
    );
  });

  it('leaves the success copy unchanged when matchScore is absent', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: { applicationId: 'app-noscore', status: 'saved', title: 'QA Engineer' },
    };
    const { text } = resolveImportResponse(res, false);
    expect(text).toBe('Imported “QA Engineer”. Open AI Job Hunter → Applications to view it.');
  });

  it('appends the percent-fit suffix to the already-tracked/status-unchanged line too', () => {
    const res = {
      ok: true as const,
      kind: 'import' as const,
      result: {
        applicationId: 'app-existing-score',
        status: 'applied',
        title: 'Backend Engineer',
        matchScore: 55,
      },
    };
    const { text, tone } = resolveImportResponse(res, false);
    expect(tone).toBe('ok');
    expect(text).toBe(
      '“Backend Engineer” is already tracked as Applied — status unchanged. ' +
        'Open AI Job Hunter → Applications to view it. — 55% fit.'
    );
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

// ── resolveFieldsProbeResponse ──────────────────────────────────────────────────

describe('resolveFieldsProbeResponse', () => {
  it('returns both signals when ok with kind=fieldsProbe (fields found)', () => {
    const res = {
      ok: true as const,
      kind: 'fieldsProbe' as const,
      hasFormFields: true,
      hasAnswerFields: true,
    };
    expect(resolveFieldsProbeResponse(res)).toEqual({
      showFormGroup: true,
      showAnswerTools: true,
    });
  });

  it('returns both signals when ok with kind=fieldsProbe (no fields at all)', () => {
    const res = {
      ok: true as const,
      kind: 'fieldsProbe' as const,
      hasFormFields: false,
      hasAnswerFields: false,
    };
    expect(resolveFieldsProbeResponse(res)).toEqual({
      showFormGroup: false,
      showAnswerTools: false,
    });
  });

  it('splits the two signals independently (identity-only form: Form group visible, Answer tools hidden)', () => {
    const res = {
      ok: true as const,
      kind: 'fieldsProbe' as const,
      hasFormFields: true,
      hasAnswerFields: false,
    };
    expect(resolveFieldsProbeResponse(res)).toEqual({
      showFormGroup: true,
      showAnswerTools: false,
    });
  });

  it('fails OPEN (both true) on a transport-level ok:false', () => {
    const res = { ok: false as const, error: 'message channel closed' };
    expect(resolveFieldsProbeResponse(res)).toEqual({
      showFormGroup: true,
      showAnswerTools: true,
    });
  });

  it('fails OPEN (both true) for an unexpected response kind', () => {
    const res = { ok: true as const, kind: 'token' as const };
    expect(resolveFieldsProbeResponse(res)).toEqual({
      showFormGroup: true,
      showAnswerTools: true,
    });
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

// ── auto-suggest on popup open (fire-and-forget, Task #30) ───────────────────
// Chained off the SAME fieldsProbe check above: appliedCheck, fieldsProbe,
// then (only when hasAnswerFields:true) autofill.check, then (only when
// enabled:true) answers.suggest — 4 sendMessage calls in that fixed order.

describe('resolveAnswersSaveResponse', () => {
  it('surfaces a transport-level error (unlike the passive appliedCheck fold)', () => {
    const res = { ok: false as const, error: 'Desktop app not reachable.' };
    const { text, tone } = resolveAnswersSaveResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe('Desktop app not reachable.');
  });

  it('returns the unexpected-response error when kind is not answersSave', () => {
    const res = { ok: true as const, kind: 'token' as const };
    const { text, tone } = resolveAnswersSaveResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe('Unexpected response — please retry.');
  });

  it('surfaces the desktop refusal text when result.ok is false', () => {
    const res = {
      ok: true as const,
      kind: 'answersSave' as const,
      result: { ok: false as const, error: "couldn't find a saved job for this page" },
    };
    const { text, tone } = resolveAnswersSaveResponse(res);
    expect(tone).toBe('err');
    expect(text).toBe("couldn't find a saved job for this page");
  });

  it('names the job with title @ company and the saved count on success', () => {
    const res = {
      ok: true as const,
      kind: 'answersSave' as const,
      result: {
        ok: true as const,
        applicationId: 'app-1',
        saved: 7,
        skipped: 2,
        title: 'Backend Engineer',
        company: 'Acme',
      },
    };
    const { text, tone } = resolveAnswersSaveResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('Saved 7 answers to Backend Engineer @ Acme — 2 already recorded.');
  });

  it('singularizes the count for exactly one saved answer', () => {
    const res = {
      ok: true as const,
      kind: 'answersSave' as const,
      result: { ok: true as const, applicationId: 'app-1', saved: 1, skipped: 0 },
    };
    const { text, tone } = resolveAnswersSaveResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('Saved 1 answer.');
  });

  it('falls back to a generic "no new answers" message when saved and skipped are both 0', () => {
    const res = {
      ok: true as const,
      kind: 'answersSave' as const,
      result: { ok: true as const, applicationId: 'app-1', saved: 0, skipped: 0 },
    };
    const { text, tone } = resolveAnswersSaveResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('No new answers to save from this page.');
  });

  it('shows a distinct "already recorded" message when saved is 0 but skipped is not', () => {
    const res = {
      ok: true as const,
      kind: 'answersSave' as const,
      result: { ok: true as const, applicationId: 'app-1', saved: 0, skipped: 3 },
    };
    const { text, tone } = resolveAnswersSaveResponse(res);
    expect(tone).toBe('ok');
    expect(text).toBe('All 3 answers were already recorded.');
  });

  it('singularizes the "already recorded" message for exactly one skipped answer', () => {
    const res = {
      ok: true as const,
      kind: 'answersSave' as const,
      result: { ok: true as const, applicationId: 'app-1', saved: 0, skipped: 1 },
    };
    const { text } = resolveAnswersSaveResponse(res);
    expect(text).toBe('All 1 answer was already recorded.');
  });

  it('names the job with only a title when company is absent', () => {
    const res = {
      ok: true as const,
      kind: 'answersSave' as const,
      result: {
        ok: true as const,
        applicationId: 'app-1',
        saved: 2,
        skipped: 0,
        title: 'QA Engineer',
      },
    };
    const { text } = resolveAnswersSaveResponse(res);
    expect(text).toBe('Saved 2 answers to QA Engineer.');
  });
});

// ── correlateSuggestions ────────────────────────────────────────────────────

describe('resolveMatchLiveResponse', () => {
  it('surfaces a transport-level error with null score fields', () => {
    const res = { ok: false as const, error: 'Desktop app not reachable.' };
    const view = resolveMatchLiveResponse(res);
    expect(view.tone).toBe('err');
    expect(view.text).toBe('Desktop app not reachable.');
    expect(view.score).toBeNull();
    expect(view.gaps).toEqual([]);
  });

  it('returns the unexpected-response error when kind is not matchLive', () => {
    const res = { ok: true as const, kind: 'token' as const };
    const view = resolveMatchLiveResponse(res);
    expect(view.tone).toBe('err');
    expect(view.text).toBe('Unexpected response — please retry.');
    expect(view.score).toBeNull();
  });

  it('surfaces the desktop refusal text when result.ok is false', () => {
    const res = {
      ok: true as const,
      kind: 'matchLive' as const,
      result: {
        ok: false as const,
        error: 'Add a resume in AI Job Hunter first, then try Check fit again.',
      },
    };
    const view = resolveMatchLiveResponse(res);
    expect(view.tone).toBe('err');
    expect(view.text).toBe('Add a resume in AI Job Hunter first, then try Check fit again.');
    expect(view.score).toBeNull();
  });

  it('renders the rounded score, source label, résumé name, and gaps on success', () => {
    const res = {
      ok: true as const,
      kind: 'matchLive' as const,
      result: {
        ok: true as const,
        combined: 71.6,
        ats: 60,
        gaps: ['kubernetes', 'terraform'],
        resumeName: 'My Resume',
        scoreSource: 'keyword' as const,
      },
    };
    const view = resolveMatchLiveResponse(res);
    expect(view.tone).toBe('ok');
    expect(view.score).toBe(72);
    expect(view.scoreLabel).toBe('keyword coverage');
    expect(view.resumeName).toBe('My Resume');
    expect(view.gaps).toEqual(['kubernetes', 'terraform']);
    expect(view.text).toBe('72% fit against “My Resume”.');
  });
});

// ── resolveAnswerAssistResponse ────────────────────────────────────────────────

describe('doSaveAnswers (#btn-save-answers)', () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));

  beforeEach(() => {
    sendMessageMock.mockReset();
    byId<HTMLButtonElement>('btn-save-answers').disabled = false;
    byId<HTMLParagraphElement>('import-msg').textContent = '';
  });

  it('shows "Saving your answers…" then the success confirmation, and re-enables the button', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'answersSave',
      result: {
        ok: true,
        applicationId: 'app-1',
        saved: 7,
        skipped: 0,
        title: 'Backend Engineer',
        company: 'Acme',
      },
      filled: [],
    });

    const btn = byId<HTMLButtonElement>('btn-save-answers');
    btn.click();
    expect(btn.disabled).toBe(true);
    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe('Saving your answers…');

    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      'Saved 7 answers to Backend Engineer @ Acme.'
    );
    expect(btn.disabled).toBe(false);
    expect(sendMessageMock).toHaveBeenCalledWith({ kind: 'answersSave' });
  });

  it('surfaces the desktop refusal text and re-enables the button (errors ARE shown)', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'answersSave',
      result: { ok: false, error: "couldn't find a saved job for this page — import it first" },
      filled: [],
    });

    const btn = byId<HTMLButtonElement>('btn-save-answers');
    btn.click();
    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      "couldn't find a saved job for this page — import it first"
    );
    expect(btn.disabled).toBe(false);
  });

  it('shows a retry message and re-enables the button when sendMessage rejects', async () => {
    sendMessageMock.mockRejectedValueOnce(new Error('message channel closed'));

    const btn = byId<HTMLButtonElement>('btn-save-answers');
    btn.click();
    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      'Could not save your answers. Please retry.'
    );
    expect(btn.disabled).toBe(false);
  });
});

// ── doSuggestAnswers (#btn-suggest-answers) — rendering, salary Copy-only rule,
// per-row Fill correlation incl. fail-safe ─────────────────────────────────

describe('doCheckFit (#btn-check-fit)', () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));

  beforeEach(() => {
    sendMessageMock.mockReset();
    byId<HTMLButtonElement>('btn-check-fit').disabled = false;
    byId<HTMLParagraphElement>('import-msg').textContent = '';
    byId<HTMLDivElement>('match-result').textContent = '';
    byId<HTMLDivElement>('match-result').hidden = true;
  });

  it('renders the score card (score / source+résumé / gap chips) on success', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'matchLive',
      result: {
        ok: true,
        combined: 72,
        ats: 60,
        gaps: ['kubernetes', 'terraform'],
        resumeName: 'My Resume',
        scoreSource: 'keyword',
      },
    });

    byId<HTMLButtonElement>('btn-check-fit').click();
    await flush();

    const card = byId<HTMLDivElement>('match-result');
    expect(card.hidden).toBe(false);
    expect(card.textContent).toContain('72% fit');
    expect(card.textContent).toContain('keyword coverage');
    expect(card.textContent).toContain('My Resume');
    expect(card.textContent).toContain('kubernetes');
    expect(card.textContent).toContain('terraform');
    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      '72% fit against “My Resume”.'
    );
  });

  it('surfaces the desktop refusal and hides the score card (no résumé saved yet)', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'matchLive',
      result: {
        ok: false,
        error: 'Add a resume in AI Job Hunter first, then try Check fit again.',
      },
    });

    byId<HTMLButtonElement>('btn-check-fit').click();
    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      'Add a resume in AI Job Hunter first, then try Check fit again.'
    );
    expect(byId<HTMLDivElement>('match-result').hidden).toBe(true);
  });

  it('re-enables the button and surfaces a retry message on a transport rejection', async () => {
    sendMessageMock.mockRejectedValueOnce(new Error('boom'));

    byId<HTMLButtonElement>('btn-check-fit').click();
    await flush();

    expect(byId<HTMLButtonElement>('btn-check-fit').disabled).toBe(false);
    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      'Could not check fit for this page. Please retry.'
    );
  });

  it('surfaces the per-connection throttle refusal and re-enables the button', async () => {
    sendMessageMock.mockResolvedValueOnce({
      ok: true,
      kind: 'matchLive',
      result: { ok: false, error: 'Too many requests — try again shortly.' },
    });

    byId<HTMLButtonElement>('btn-check-fit').click();
    await flush();

    expect(byId<HTMLParagraphElement>('import-msg').textContent).toBe(
      'Too many requests — try again shortly.'
    );
    expect(byId<HTMLDivElement>('match-result').hidden).toBe(true);
    expect(byId<HTMLButtonElement>('btn-check-fit').disabled).toBe(false);
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
