/**
 * Unit tests for the pure view-decision helpers exported from popup.ts. They
 * have no DOM or browser-API dependency of their own, but importing the module
 * runs its load-time wiring, so the shared popup harness boots it first.
 */

import { describe, expect, it, vi } from 'vitest';

vi.mock('@wxt-dev/browser', async () => (await import('./browser-mock')).popupBrowserMock());
vi.mock('../../lib/storage', () => ({
  looksLikeToken: vi.fn(() => false),
}));

import { resolveImportButtonLabel } from '../../job-tools/responses';
import { bootPopup } from './test-support';

const { resolveShowMarkAppliedButton, resolveMarkAppliedResponse, resolveAnswersNoticeLine } =
  await bootPopup();

/** An `appliedCheck` reply carrying `result`. */
const applied = (result: Record<string, unknown>) =>
  ({ ok: true as const, kind: 'appliedCheck' as const, result }) as never;
const TOKEN = { ok: true as const, kind: 'token' as const };
const statusUpdate = (result: Record<string, unknown>) =>
  ({ ok: true as const, kind: 'statusUpdate' as const, result }) as never;

describe('resolveImportButtonLabel', () => {
  it.each([
    ['returns the default label when not found', applied({ found: false }), 'Import this job'],
    [
      'returns the default label when the result carries an error',
      applied({ found: true, error: 'malformed' }),
      'Import this job',
    ],
    ['returns the default label for a non-appliedCheck response', TOKEN, 'Import this job'],
    [
      'returns the relabeled action when found',
      applied({ found: true, status: 'saved' }),
      'Re-import / update',
    ],
  ])('%s', (_name, res, label) => {
    expect(resolveImportButtonLabel(res)).toBe(label);
  });
});

describe('resolveShowMarkAppliedButton', () => {
  it.each([
    ['returns false for a non-appliedCheck response', TOKEN, false],
    ['returns false when ok is false', { ok: false as const, error: 'boom' }, false],
    ['returns false when not found', applied({ found: false }), false],
    [
      'returns false when the result carries an error',
      applied({ found: false, error: 'malformed' }),
      false,
    ],
    ['returns true for a found + saved result', applied({ found: true, status: 'saved' }), true],
    [
      'returns false for a found result with no status (CAS precondition requires an explicit saved status)',
      applied({ found: true }),
      false,
    ],
    [
      'returns false for a found + already-applied result',
      applied({ found: true, status: 'applied' }),
      false,
    ],
    [
      'returns false for a found + mid-pipeline result',
      applied({ found: true, status: 'interviewing' }),
      false,
    ],
  ])('%s', (_name, res, show) => {
    expect(resolveShowMarkAppliedButton(res)).toBe(show);
  });
});

describe('resolveMarkAppliedResponse', () => {
  it.each([
    [
      'surfaces a transport-level error (unlike the passive appliedCheck fold)',
      { ok: false as const, error: 'Desktop app not reachable.' },
      'Desktop app not reachable.',
      'err',
    ],
    [
      'returns the unexpected-response error when kind is not statusUpdate',
      TOKEN,
      'Unexpected response — please retry.',
      'err',
    ],
    [
      'surfaces the desktop refusal text when result.ok is false',
      statusUpdate({ ok: false, error: "couldn't find a saved job for this page" }),
      "couldn't find a saved job for this page",
      'err',
    ],
    [
      'falls back to a generic refusal message when result.ok is false with no error text',
      statusUpdate({ ok: false }),
      'Could not mark this job as applied.',
      'err',
    ],
    [
      'reports success when result.ok is true',
      statusUpdate({ ok: true, applicationId: 'app-1', status: 'applied' }),
      'Marked as applied.',
      'ok',
    ],
  ])('%s', (_name, res, expectedText, expectedTone) => {
    const { text, tone } = resolveMarkAppliedResponse(res);
    expect(tone).toBe(expectedTone);
    expect(text).toBe(expectedText);
  });
});

// PR0 §2's passive notice, replacing the popup's own interactive Answer-tools rows.
describe('resolveAnswersNoticeLine', () => {
  /** A state whose rows carry exactly these statuses. */
  const stateWith = (statuses: ('empty' | 'drafted' | 'filled' | 'saved-available')[]) => ({
    tabId: 1,
    origin: 'https://jobs.example.com',
    scannedAt: 0,
    rows: statuses.map((status, i) => ({
      id: `r${i}`,
      question: `Q${i}`,
      field: null,
      status,
      versions: [],
      selected: -1,
    })),
    stream: null,
    pageChanged: false,
  });

  it('returns null when there is no state at all', () => {
    expect(resolveAnswersNoticeLine(null)).toBeNull();
  });

  it('returns null when every row is still empty', () => {
    expect(resolveAnswersNoticeLine(stateWith(['empty']))).toBeNull();
  });

  it('counts rows that are not empty, singular phrasing for one', () => {
    expect(resolveAnswersNoticeLine(stateWith(['drafted', 'empty']))).toBe(
      '1 answer ready on this page.'
    );
  });

  it('uses plural phrasing for more than one', () => {
    expect(resolveAnswersNoticeLine(stateWith(['filled', 'saved-available']))).toBe(
      '2 answers ready on this page.'
    );
  });
});
