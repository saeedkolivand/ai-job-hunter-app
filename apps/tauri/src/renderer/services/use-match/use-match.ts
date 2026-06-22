import { useMemo } from 'react';
import { keepPreviousData, useMutation, useQuery } from '@tanstack/react-query';

import type { MatchResumeRequest, MatchScore } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';
import { useSemanticScoring } from '@/store/preferences-store';

import { keys, QUERY_TIMES } from '../query-client';

/**
 * Score a resume against a job posting on demand. The result is expensive to
 * compute (embeds the job text), so this is a mutation rather than a query.
 */
export const useMatchResume = () => {
  const api = useAppClient();
  return useMutation({
    mutationFn: (req: MatchResumeRequest): Promise<MatchScore> => api.match.resume(req),
  });
};

/**
 * Score a resume against a job posting (legacy single-job hook). Results are
 * cached for 10 minutes so navigating away and back does not re-fire the embed.
 *
 * No longer used by the jobs default path — the Jobs page now batch-scores via
 * `useJobMatchScores` (MatchScoresProvider). Retained for any single-job caller.
 * Pass `enabled = false` to defer execution.
 */
export const useJobMatchScore = (resumeId: string | null, jobId: string, enabled = true) => {
  const api = useAppClient();
  const semanticScoring = useSemanticScoring();
  return useQuery({
    queryKey: ['match', resumeId, jobId, semanticScoring],
    queryFn: (): Promise<MatchScore> =>
      api.match.resume({
        resumeId: resumeId as string,
        jobId,
        semanticScoringEnabled: semanticScoring,
      }),
    enabled: enabled && !!resumeId && !!jobId,
    staleTime: QUERY_TIMES.TEN_MIN,
  });
};

/**
 * Score a resume against MANY job postings in ONE backend call, replacing N
 * serialised per-row `match_resume` round-trips. Cached for 10 minutes; the key
 * is order-independent (job ids are sorted) so the same set hits regardless of
 * ordering. `scoresById` maps each successful result by `jobId`, skipping
 * per-job `{ error }` elements.
 */
export const useJobMatchScores = (resumeId: string | null, jobIds: string[]) => {
  const api = useAppClient();
  const semanticScoring = useSemanticScoring();
  const ids = Array.isArray(jobIds) ? jobIds : [];
  const query = useQuery({
    queryKey: keys.match.batch(resumeId, ids, semanticScoring),
    queryFn: (): Promise<MatchScore[]> =>
      api.match.resumeBatch({
        resumeId: resumeId as string,
        jobIds: ids,
        semanticScoringEnabled: semanticScoring,
      }),
    enabled: !!resumeId && ids.length > 0,
    staleTime: QUERY_TIMES.TEN_MIN,
    placeholderData: keepPreviousData,
  });
  const scoresById = useMemo(() => {
    const map = new Map<string, MatchScore>();
    for (const s of query.data ?? []) {
      const r = s as MatchScore & { error?: string };
      if (!r.error && r.jobId) map.set(r.jobId, r);
    }
    return map;
  }, [query.data]);
  return { ...query, scoresById };
};
