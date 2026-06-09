import { useMutation, useQuery } from '@tanstack/react-query';

import type { MatchResumeRequest, MatchScore } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';
import { useSemanticScoring } from '@/store/preferences-store';

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
 * Auto-score a resume against a job posting. Results are cached for 10 minutes
 * so navigating away and back does not re-fire the embedding call.
 *
 * Pass `enabled = false` to defer execution (used by ScoringSchedulerProvider
 * to serialise concurrent requests down to CONCURRENCY = 1).
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
    staleTime: 10 * 60 * 1000,
  });
};
