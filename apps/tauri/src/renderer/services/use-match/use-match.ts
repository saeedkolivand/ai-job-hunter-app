import { useMutation } from '@tanstack/react-query';

import type { MatchResumeRequest, MatchScore } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

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
