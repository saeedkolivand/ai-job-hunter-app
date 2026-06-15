import { createContext, type ReactNode, useContext, useMemo } from 'react';

import type { MatchScore } from '@ajh/shared';

import { useJobMatchScores } from '@/services';

interface MatchScoresContextValue {
  getScore: (jobId: string) => MatchScore | undefined;
  isPending: boolean; // raw batch in-flight flag (gated by resume + jobIds)
  hasResume: boolean;
}

const MatchScoresContext = createContext<MatchScoresContextValue | null>(null);

/**
 * Batch-scores every filtered posting in ONE `useJobMatchScores` call and
 * distributes the result per-row through context, replacing the old N
 * serialised per-row scoring round-trips. Each `RowMatchScore` reads its slice
 * via `useRowMatchScore`, so rows hold no scheduling or fetching logic.
 */
export function MatchScoresProvider({
  resumeId,
  jobIds,
  children,
}: {
  resumeId: string | null;
  jobIds: string[];
  children: ReactNode;
}) {
  const { scoresById, isPending: queryIsPending } = useJobMatchScores(resumeId, jobIds);

  const isPending = queryIsPending && !!resumeId && jobIds.length > 0;
  const hasResume = !!resumeId;

  const value = useMemo<MatchScoresContextValue>(
    () => ({
      getScore: (jobId: string) => scoresById.get(jobId),
      isPending,
      hasResume,
    }),
    [scoresById, isPending, hasResume]
  );

  return <MatchScoresContext.Provider value={value}>{children}</MatchScoresContext.Provider>;
}

export function useRowMatchScore(jobId: string): {
  score?: MatchScore;
  pending: boolean;
  hasResume: boolean;
} {
  const ctx = useContext(MatchScoresContext);
  if (!ctx) throw new Error('useRowMatchScore must be used within MatchScoresProvider');
  const { getScore, isPending, hasResume } = ctx;
  const score = getScore(jobId);
  return { score, pending: isPending && !score, hasResume };
}

/**
 * Reads the whole batch-score context — used by `JobsResults` to gate the list
 * (while `isPending`/scraping) and to re-sort by score on reveal. Rows should
 * keep using `useRowMatchScore` for their per-row slice.
 */
export function useMatchScores(): {
  getScore: (jobId: string) => MatchScore | undefined;
  isPending: boolean;
  hasResume: boolean;
} {
  const ctx = useContext(MatchScoresContext);
  if (!ctx) throw new Error('useMatchScores must be used within MatchScoresProvider');
  return ctx;
}
