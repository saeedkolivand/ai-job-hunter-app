/**
 * MatchScoresProvider — batch-score distribution + derived-flag tests.
 *
 * The provider calls useJobMatchScores(resumeId, jobIds) ONCE and exposes a
 * per-row slice through useRowMatchScore. These tests stub useJobMatchScores
 * and assert:
 *  - getScore distributes the matching MatchScore by jobId (undefined when absent)
 *  - hasResume === !!resumeId
 *  - pending === isPending (gated by resume + jobIds) && the row has NO cached score
 *  - useRowMatchScore throws outside the provider
 */
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import type { MatchScore } from '@ajh/shared';

// ── useJobMatchScores stub ────────────────────────────────────────────────────
// Module-level ref so each test sets it BEFORE renderHook.

let stubbedQuery: { scoresById: Map<string, MatchScore>; isPending: boolean } = {
  scoresById: new Map(),
  isPending: false,
};

vi.mock('@/services', () => ({
  useJobMatchScores: () => stubbedQuery,
}));

import { MatchScoresProvider, useRowMatchScore } from './MatchScoresProvider';

// ── constants ─────────────────────────────────────────────────────────────────

const RESUME_ID = 'resume-xyz';
const JOB_A = 'job-a';
const JOB_B = 'job-b';

/**
 * Fixture with DISTINCT field values so a future swap of combined/ats/semantic
 * in the context value cannot pass silently. The ats and semantic values are
 * intentionally different from combined and from each other.
 */
function score(jobId: string, combined: number): MatchScore {
  return {
    resumeId: RESUME_ID,
    jobId,
    ats: combined - 10, // distinct: always 10 below combined
    semantic: combined + 5, // distinct: always 5 above combined
    combined,
    gaps: [],
    recommendations: [],
  };
}

// ── helper ─────────────────────────────────────────────────────────────────────

function renderRow(
  jobId: string,
  opts: {
    resumeId?: string | null;
    jobIds?: string[];
    scoresById?: Map<string, MatchScore>;
    isPending?: boolean;
  } = {}
) {
  stubbedQuery = {
    scoresById: opts.scoresById ?? new Map(),
    isPending: opts.isPending ?? false,
  };
  const resumeId = 'resumeId' in opts ? (opts.resumeId ?? null) : RESUME_ID;
  const jobIds = opts.jobIds ?? [JOB_A, JOB_B];
  const wrapper = ({ children }: { children: ReactNode }) => (
    <MatchScoresProvider resumeId={resumeId} jobIds={jobIds}>
      {children}
    </MatchScoresProvider>
  );
  return renderHook(() => useRowMatchScore(jobId), { wrapper });
}

// ── tests ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  stubbedQuery = { scoresById: new Map(), isPending: false };
});

describe('MatchScoresProvider — score distribution', () => {
  it('returns the matching score for the requested jobId', () => {
    const scores = new Map<string, MatchScore>([
      [JOB_A, score(JOB_A, 82)],
      [JOB_B, score(JOB_B, 30)],
    ]);
    const { result } = renderRow(JOB_A, { scoresById: scores });

    expect(result.current.score).toEqual(score(JOB_A, 82));
    // Pin that the consumer reads the `combined` field, not ats/semantic.
    // score(JOB_A, 82) has combined=82, ats=72, semantic=87 — all distinct.
    expect(result.current.score?.combined).toBe(82);
    expect(result.current.score?.ats).toBe(72);
    expect(result.current.score?.semantic).toBe(87);
  });

  it('returns undefined when the requested jobId has no score', () => {
    const scores = new Map<string, MatchScore>([[JOB_B, score(JOB_B, 30)]]);
    const { result } = renderRow(JOB_A, { scoresById: scores });

    expect(result.current.score).toBeUndefined();
  });
});

describe('MatchScoresProvider — hasResume', () => {
  it('is true when a resumeId is present', () => {
    const { result } = renderRow(JOB_A, { resumeId: RESUME_ID });
    expect(result.current.hasResume).toBe(true);
  });

  it('is false when resumeId is null', () => {
    const { result } = renderRow(JOB_A, { resumeId: null });
    expect(result.current.hasResume).toBe(false);
  });
});

describe('MatchScoresProvider — pending', () => {
  it('is true while the batch is in-flight with a resume and job ids', () => {
    const { result } = renderRow(JOB_A, { isPending: true });
    expect(result.current.pending).toBe(true);
  });

  // FIX 2 pin: a row WITHOUT a cached score during a pending batch → pending === true.
  // Exhaustive pair with the "cached score suppresses pending" case below.
  it('is true for a row WITHOUT a cached score while the batch is in-flight', () => {
    // scoresById has JOB_B but not JOB_A — JOB_A has no cached score.
    const scores = new Map<string, MatchScore>([[JOB_B, score(JOB_B, 55)]]);
    const { result } = renderRow(JOB_A, { scoresById: scores, isPending: true });
    expect(result.current.pending).toBe(true);
    expect(result.current.score).toBeUndefined();
  });

  it('is false for a row WITH a cached score even while the batch is in-flight', () => {
    const scores = new Map<string, MatchScore>([[JOB_A, score(JOB_A, 70)]]);
    const { result } = renderRow(JOB_A, { scoresById: scores, isPending: true });
    expect(result.current.pending).toBe(false);
    expect(result.current.score).toBeDefined();
  });

  it('is false when isPending but there is no resume', () => {
    const { result } = renderRow(JOB_A, { resumeId: null, isPending: true });
    expect(result.current.pending).toBe(false);
  });

  it('is false when isPending but jobIds is empty', () => {
    const { result } = renderRow(JOB_A, { jobIds: [], isPending: true });
    expect(result.current.pending).toBe(false);
  });

  it('is false when the batch has settled', () => {
    const { result } = renderRow(JOB_A, { isPending: false });
    expect(result.current.pending).toBe(false);
  });
});

describe('useRowMatchScore — guard', () => {
  it('throws when used outside MatchScoresProvider', () => {
    expect(() => renderHook(() => useRowMatchScore(JOB_A))).toThrow(
      'useRowMatchScore must be used within MatchScoresProvider'
    );
  });
});
