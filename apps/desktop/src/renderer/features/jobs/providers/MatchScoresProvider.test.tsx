/**
 * MatchScoresProvider — reactive on-demand scoring tests.
 *
 * Reactive model: `scoreJob(jobId)` adds to a `requested` Set in state.
 * `useRowMatchScore(jobId)` gates `useJobMatchScore(resumeId, jobId, enabled)`
 * where `enabled = requested.has(jobId) && hasResume`.
 *
 * Tests stub `@/services` with `useJobMatchScore` so no IPC or QueryClient needed.
 *
 * Exposed surface:
 *   - hasResume           → !!resumeId
 *   - useRowMatchScore    → { score, hasResume }; query only fires after scoreJob
 *   - useMatchScores      → { scoreJob, hasResume }
 */
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { MatchScore } from '@ajh/shared';

// ── useJobMatchScore stub ─────────────────────────────────────────────────────
// Controls what the reactive query returns per (jobId, enabled) call.

const scoreCache = new Map<string, MatchScore>();
let lastEnabled = false;

const mockUseJobMatchScore = vi.fn((_resumeId: string | null, _jobId: string, enabled = true) => {
  lastEnabled = enabled;
  return { data: enabled ? scoreCache.get(_jobId) : undefined };
});

vi.mock('@/services', () => ({
  useJobMatchScore: (...args: Parameters<typeof mockUseJobMatchScore>) =>
    mockUseJobMatchScore(...args),
}));

import { MatchScoresProvider, useMatchScores, useRowMatchScore } from './MatchScoresProvider';

// ── constants ─────────────────────────────────────────────────────────────────

const RESUME_ID = 'resume-xyz';
const JOB_A = 'job-a';
const JOB_B = 'job-b';

function score(jobId: string, combined: number): MatchScore {
  return {
    resumeId: RESUME_ID,
    jobId,
    ats: combined - 10,
    semantic: combined + 5,
    combined,
    gaps: [],
    recommendations: [],
  };
}

// ── helpers ───────────────────────────────────────────────────────────────────

function wrapper(resumeId: string | null) {
  return ({ children }: { children: ReactNode }) => (
    <MatchScoresProvider resumeId={resumeId}>{children}</MatchScoresProvider>
  );
}

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  scoreCache.clear();
  mockUseJobMatchScore.mockClear();
  lastEnabled = false;
});

// ── hasResume ─────────────────────────────────────────────────────────────────

describe('MatchScoresProvider — hasResume', () => {
  it('is true when resumeId is present', () => {
    const { result } = renderHook(() => useRowMatchScore(JOB_A), {
      wrapper: wrapper(RESUME_ID),
    });
    expect(result.current.hasResume).toBe(true);
  });

  it('is false when resumeId is null', () => {
    const { result } = renderHook(() => useRowMatchScore(JOB_A), {
      wrapper: wrapper(null),
    });
    expect(result.current.hasResume).toBe(false);
  });
});

// ── reactive gate: score is undefined until scoreJob fires ────────────────────

describe('MatchScoresProvider — reactive gate', () => {
  it('score is undefined before scoreJob is called (query disabled)', () => {
    scoreCache.set(JOB_A, score(JOB_A, 82));

    const { result } = renderHook(() => useRowMatchScore(JOB_A), {
      wrapper: wrapper(RESUME_ID),
    });

    // useJobMatchScore is called with enabled=false (not yet requested)
    expect(result.current.score).toBeUndefined();
    expect(lastEnabled).toBe(false);
  });

  it('score is returned once scoreJob is called (query enabled)', async () => {
    scoreCache.set(JOB_A, score(JOB_A, 75));

    const { result } = renderHook(
      () => ({ row: useRowMatchScore(JOB_A), ctrl: useMatchScores() }),
      { wrapper: wrapper(RESUME_ID) }
    );

    // Before scoring: disabled
    expect(result.current.row.score).toBeUndefined();

    await act(async () => {
      result.current.ctrl.scoreJob(JOB_A);
    });

    // After scoring: enabled → stub returns cached score
    expect(result.current.row.score).toEqual(score(JOB_A, 75));
    expect(lastEnabled).toBe(true);
  });

  it('scoreJob is idempotent (same job scored twice does not re-enable the query)', async () => {
    scoreCache.set(JOB_A, score(JOB_A, 90));

    const { result } = renderHook(
      () => ({ row: useRowMatchScore(JOB_A), ctrl: useMatchScores() }),
      { wrapper: wrapper(RESUME_ID) }
    );

    // Score once — query enables.
    await act(async () => {
      result.current.ctrl.scoreJob(JOB_A);
    });
    const callsAfterFirst = mockUseJobMatchScore.mock.calls.length;

    // Score again — must not trigger an additional render/call with a new Set.
    await act(async () => {
      result.current.ctrl.scoreJob(JOB_A);
    });

    // The mock call count must not have grown: scoreJob returned the same Set
    // reference (stable ref guard), so no re-render occurred.
    expect(mockUseJobMatchScore.mock.calls.length).toBe(callsAfterFirst);
    // Score is still present.
    expect(result.current.row.score).toEqual(score(JOB_A, 90));
  });

  it('scores different jobs independently (no key collision)', async () => {
    scoreCache.set(JOB_A, score(JOB_A, 80));
    scoreCache.set(JOB_B, score(JOB_B, 60));

    const { result } = renderHook(
      () => ({
        a: useRowMatchScore(JOB_A),
        b: useRowMatchScore(JOB_B),
        ctrl: useMatchScores(),
      }),
      { wrapper: wrapper(RESUME_ID) }
    );

    await act(async () => {
      result.current.ctrl.scoreJob(JOB_A);
    });

    // JOB_A scored; JOB_B not yet requested → still undefined
    expect(result.current.a.score).toEqual(score(JOB_A, 80));
    expect(result.current.b.score).toBeUndefined();

    await act(async () => {
      result.current.ctrl.scoreJob(JOB_B);
    });

    expect(result.current.b.score).toEqual(score(JOB_B, 60));
  });

  it('no score returned when resumeId is null even after scoreJob', async () => {
    scoreCache.set(JOB_A, score(JOB_A, 70));

    const { result } = renderHook(
      () => ({ row: useRowMatchScore(JOB_A), ctrl: useMatchScores() }),
      { wrapper: wrapper(null) }
    );

    await act(async () => {
      result.current.ctrl.scoreJob(JOB_A);
    });

    // hasResume=false means enabled=false regardless of requested
    expect(result.current.row.score).toBeUndefined();
    expect(result.current.row.hasResume).toBe(false);
  });
});

// ── resumeId change resets requested set (fix #4) ────────────────────────────

describe('MatchScoresProvider — resumeId change resets requested', () => {
  it('clears requested set when resumeId prop changes', async () => {
    scoreCache.set(JOB_A, score(JOB_A, 80));

    const RESUME_B = 'resume-bbb';

    // Use a ref-based wrapper so we can change resumeId via rerender.
    let currentResumeId: string | null = RESUME_ID;
    const DynamicWrapper = ({ children }: { children: ReactNode }) => (
      <MatchScoresProvider resumeId={currentResumeId}>{children}</MatchScoresProvider>
    );

    const { result, rerender } = renderHook(
      () => ({ row: useRowMatchScore(JOB_A), ctrl: useMatchScores() }),
      { wrapper: DynamicWrapper }
    );

    // Score JOB_A under RESUME_ID — query becomes enabled.
    await act(async () => {
      result.current.ctrl.scoreJob(JOB_A);
    });
    expect(result.current.row.score).toEqual(score(JOB_A, 80));

    // Switch resumeId — the requested set must be cleared.
    await act(async () => {
      currentResumeId = RESUME_B;
      rerender();
    });

    // After the resumeId change, JOB_A should no longer be in requested,
    // so the query is disabled and score returns undefined.
    expect(result.current.row.score).toBeUndefined();
  });
});

// ── guards ────────────────────────────────────────────────────────────────────

describe('useRowMatchScore — guard', () => {
  it('throws when used outside MatchScoresProvider', () => {
    expect(() => renderHook(() => useRowMatchScore(JOB_A))).toThrow(
      'useRowMatchScore must be used within MatchScoresProvider'
    );
  });
});

describe('useMatchScores — guard', () => {
  it('throws when used outside MatchScoresProvider', () => {
    expect(() => renderHook(() => useMatchScores())).toThrow(
      'useMatchScores must be used within MatchScoresProvider'
    );
  });
});
