/**
 * Track B1: cross-run reliability history.
 *
 * The whole point of the feature: a board's chip must say whether its zero is
 * "nothing matched today" or "this source has been down for a week".
 *
 * `now` is injected and every expectation is a literal string — including the
 * relative time — so a regression in the derivation can't move both sides of
 * an assertion at once.
 */

import { describe, expect, it, vi } from 'vitest';

import { chips, renderChips, texts } from './test-support';

vi.mock('@ajh/translations', async () => (await import('./test-mocks')).translationsMock());
vi.mock('@ajh/ui', async () => (await import('./test-mocks')).uiMock());

describe('BoardSummaryChips — board health history', () => {
  const NOW = 1_767_225_600_000; // 2026-01-01T00:00:00Z
  const DAY = 24 * 60 * 60 * 1000;

  it('adds an amber history chip beside a failing board, keeping its own error chip', () => {
    const all = renderChips(
      [
        {
          board: 'wwr',
          count: 0,
          error: 'HTTP 500',
          health: {
            status: 'failing',
            consecutiveFailures: 4,
            verifiedRuns: 9,
            failedRuns: 4,
            lastSuccessAt: NOW - 6 * DAY,
            failingSince: NOW - 5 * DAY,
          },
        },
      ],
      NOW
    );
    expect(all).toHaveLength(2);
    // This run's own failure is untouched.
    expect(all[0]?.getAttribute('data-color')).toBe('error');
    expect(all[0]?.textContent).toBe('label(wwr) · HTTP 500');
    // …and the standing history rides beside it.
    expect(all[1]?.getAttribute('data-color')).toBe('warning');
    expect(all[1]?.textContent).toBe(
      'label(wwr) · jobs.boardSummary.health.failingSince:4:6 days ago'
    );
  });

  it('reports a board that has NEVER worked distinctly from one that broke recently', () => {
    renderChips(
      [
        {
          board: 'linkedin',
          count: 0,
          error: 'blocked',
          health: {
            status: 'failing',
            consecutiveFailures: 9,
            verifiedRuns: 9,
            failedRuns: 9,
            failingSince: NOW - 30 * DAY,
          },
        },
      ],
      NOW
    );
    expect(texts()[1]).toBe(
      'label(linkedin) · jobs.boardSummary.health.neverWorkedSince:4 wk. ago'
    );
  });

  it('falls back to the failure count when a never-worked board has no streak start', () => {
    renderChips(
      [
        {
          board: 'linkedin',
          count: 0,
          error: 'blocked',
          health: { status: 'failing', consecutiveFailures: 9, verifiedRuns: 9, failedRuns: 9 },
        },
      ],
      NOW
    );
    expect(texts()[1]).toBe('label(linkedin) · jobs.boardSummary.health.neverWorked:9');
  });

  it('shows the history for a board that is SKIPPED this run but broken since before', () => {
    // The case a "skipped" chip alone hides completely: nothing was fetched
    // today, and the board has been failing since long before that.
    const all = renderChips(
      [
        {
          board: 'linkedin',
          count: 0,
          skipped: 'needs-login',
          health: {
            status: 'failing',
            consecutiveFailures: 3,
            verifiedRuns: 10,
            failedRuns: 3,
            lastSuccessAt: NOW - 5 * DAY,
            failingSince: NOW - 4 * DAY,
            lastError: 'HTTP 999 from C:\\Users\\me\\session',
          },
        },
      ],
      NOW
    );
    expect(all).toHaveLength(2);
    expect(all[0]?.getAttribute('data-color')).toBe('default');
    expect(all[0]?.textContent).toBe('label(linkedin) · jobs.boardSummary.skip.needsLogin');
    expect(all[1]?.textContent).toBe(
      'label(linkedin) · jobs.boardSummary.health.failingSince:3:5 days ago'
    );
    // The remembered "why" rides as a tooltip — sanitized, since it came out of
    // a store and crossed IPC — because this run's own chip has no error to show.
    const hint = all[1]?.querySelector('span[title]')?.getAttribute('title') ?? '';
    expect(hint).toBe('HTTP 999 from <path-redacted>');
  });

  it('leaves the history chip untitled when there is no remembered reason', () => {
    renderChips(
      [
        {
          board: 'xing',
          count: 0,
          error: 'boom',
          health: {
            status: 'failing',
            consecutiveFailures: 2,
            verifiedRuns: 5,
            failedRuns: 2,
            lastSuccessAt: NOW - 3 * DAY,
          },
        },
      ],
      NOW
    );
    expect(chips()[1]?.querySelector('span[title]')).toBeNull();
  });

  it('renders a stale board as "not checked since <when>"', () => {
    renderChips(
      [
        {
          board: 'xing',
          count: 0,
          skipped: 'needs-login',
          health: {
            status: 'stale',
            consecutiveFailures: 0,
            verifiedRuns: 4,
            failedRuns: 0,
            lastSuccessAt: NOW - 20 * DAY,
          },
        },
      ],
      NOW
    );
    expect(texts()[1]).toBe('label(xing) · jobs.boardSummary.health.stale:3 wk. ago');
  });

  it('says nothing for a healthy board, an unknown one, or an incoherent streak', () => {
    renderChips(
      [
        {
          board: 'a',
          count: 3,
          health: {
            status: 'healthy',
            consecutiveFailures: 0,
            verifiedRuns: 4,
            failedRuns: 0,
            lastSuccessAt: NOW,
          },
        },
        {
          board: 'b',
          count: 0,
          skipped: 'needs-keys',
          health: { status: 'unknown', consecutiveFailures: 0, verifiedRuns: 0, failedRuns: 0 },
        },
        // "failing" with nothing behind it — never render "down for 0 runs".
        {
          board: 'c',
          count: 0,
          error: 'boom',
          health: { status: 'failing', consecutiveFailures: 0, verifiedRuns: 3, failedRuns: 0 },
        },
      ],
      NOW
    );
    // Exactly one chip per board — no history chip anywhere.
    expect(chips()).toHaveLength(3);
    expect(texts().some((s) => s.includes('health.'))).toBe(false);
  });

  it('reports a flapping board that a consecutive-failure streak cannot see', () => {
    // The board SUCCEEDED this run and has an empty streak — a streak counter
    // alone would call it healthy — but it has failed half the runs that reached
    // it, which is the thing the user actually needs told.
    const all = renderChips(
      [
        {
          board: 'wwr',
          count: 3,
          health: {
            status: 'flaky',
            consecutiveFailures: 0,
            verifiedRuns: 12,
            failedRuns: 6,
            lastSuccessAt: NOW,
          },
        },
      ],
      NOW
    );
    expect(all).toHaveLength(2);
    expect(all[0]?.getAttribute('data-color')).toBe('success');
    expect(all[1]?.getAttribute('data-color')).toBe('warning');
    expect(all[1]?.textContent).toBe('label(wwr) · jobs.boardSummary.health.flaky:6/12');
  });

  it('says nothing for an incoherent flaky payload', () => {
    renderChips(
      [
        // 0 failures cannot make a board flaky…
        {
          board: 'a',
          count: 1,
          health: { status: 'flaky', consecutiveFailures: 0, verifiedRuns: 9, failedRuns: 0 },
        },
        // …nor can more failures than runs.
        {
          board: 'b',
          count: 1,
          health: { status: 'flaky', consecutiveFailures: 0, verifiedRuns: 2, failedRuns: 9 },
        },
      ],
      NOW
    );
    // Both boards succeeded and neither got a history chip, so the strip
    // collapses to the single "all ok" chip — a collapse that a stray health
    // chip would itself have prevented.
    expect(chips()).toHaveLength(1);
    expect(texts()[0]).toBe('jobs.boardSummary.allOk:2');
  });

  it('does not badge anything when the backend sends no health at all (pre-B1 records)', () => {
    expect(renderChips([{ board: 'wwr', count: 0, error: 'boom' }], NOW)).toHaveLength(1);
  });
});
