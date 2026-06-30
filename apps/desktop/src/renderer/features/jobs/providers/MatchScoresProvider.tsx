import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';

import type { MatchScore } from '@ajh/shared';

import { useJobMatchScore } from '@/services';

interface MatchScoresContextValue {
  requested: Set<string>;
  resumeId: string | null;
  scoreJob: (jobId: string) => void;
  hasResume: boolean;
}

const MatchScoresContext = createContext<MatchScoresContextValue | null>(null);

/**
 * On-demand per-job match scoring provider.
 *
 * Replaces the old batch-all-on-mount model. Scores are fetched one at a time,
 * ONLY when the user opens a job (`scoreJob(jobId)` called by `JobDetailPane`).
 *
 * Reactive model: `scoreJob` adds the jobId to a `requested` Set in React state.
 * Each `RowMatchScore` row calls `useJobMatchScore(resumeId, jobId, requested.has(jobId))`
 * which is a `useQuery`-backed subscription — not a snapshot. The row re-renders
 * reactively when the score lands, without any polling or event wiring.
 *
 * Per-job cache key (`['match', resumeId, jobId]`) means repeated opens within
 * `TEN_MIN` are free — no re-embed on every click.
 */
export function MatchScoresProvider({
  resumeId,
  children,
}: {
  resumeId: string | null;
  children: ReactNode;
}) {
  const [requested, setRequested] = useState<Set<string>>(new Set());
  const hasResume = !!resumeId;

  // Reset the requested set whenever the active resume changes so previously-opened
  // rows don't fire stale scores against the new resume on mount.
  useEffect(() => {
    setRequested(new Set());
  }, [resumeId]);

  const scoreJob = useCallback((jobId: string) => {
    if (!jobId) return;
    setRequested((prev) => {
      if (prev.has(jobId)) return prev; // stable ref when already present
      return new Set([...prev, jobId]);
    });
  }, []);

  const value = useMemo<MatchScoresContextValue>(
    () => ({ requested, resumeId, scoreJob, hasResume }),
    [requested, resumeId, scoreJob, hasResume]
  );

  return <MatchScoresContext.Provider value={value}>{children}</MatchScoresContext.Provider>;
}

/**
 * Per-row hook: returns `{ score, hasResume }` backed by a reactive `useQuery`
 * subscription. The query is gated — it only enables when the user has opened
 * this job (`requested.has(jobId)`), so rows that have never been opened show
 * no badge rather than a loading placeholder.
 */
export function useRowMatchScore(jobId: string): {
  score: MatchScore | undefined;
  hasResume: boolean;
} {
  const ctx = useContext(MatchScoresContext);
  if (!ctx) throw new Error('useRowMatchScore must be used within MatchScoresProvider');
  const { requested, resumeId, hasResume } = ctx;
  const enabled = requested.has(jobId) && hasResume;
  const { data } = useJobMatchScore(resumeId, jobId, enabled);
  return { score: data, hasResume };
}

/**
 * Returns the scoring trigger — used by `JobDetailPane` to call `scoreJob` on open.
 */
export function useMatchScores(): { scoreJob: (jobId: string) => void; hasResume: boolean } {
  const ctx = useContext(MatchScoresContext);
  if (!ctx) throw new Error('useMatchScores must be used within MatchScoresProvider');
  return { scoreJob: ctx.scoreJob, hasResume: ctx.hasResume };
}
