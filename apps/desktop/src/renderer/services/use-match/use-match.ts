import { useMutation, useQuery } from '@tanstack/react-query';

import type { MatchResumeRequest, MatchScore, TrimSuggestions } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';
import { useSemanticScoring } from '@/store/preferences-store';

import { QUERY_TIMES } from '../query-client';

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
 * Reactive per-job score hook. Results are cached for 10 minutes so navigating
 * away and back does not re-fire the embed.
 *
 * Used by `useRowMatchScore` (via MatchScoresProvider) with `enabled = requested.has(jobId)`
 * so the query only fires once the user has opened the job. Pass `enabled = false` to defer.
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
 * Advisory trim ranking for the résumé currently on screen: which bullets carry
 * the least of this posting's vocabulary, weakest first.
 *
 * Keyed on the document text itself (not an id) because the AI-Generate flow
 * scores unsaved, hand-edited output. Scoring is embedding-free and local, so
 * re-running it on each committed edit is cheap — but the caller should pass
 * `enabled = false` until the preview actually overflows, so a normal-length
 * résumé never pays for it at all.
 */
export const useTrimSuggestions = (
  resumeText: string,
  jobText: string,
  locale: string | undefined,
  enabled = true
) => {
  const api = useAppClient();
  return useQuery({
    queryKey: ['trim-suggestions', resumeText, jobText, locale],
    queryFn: (): Promise<TrimSuggestions> =>
      api.match.trimSuggestions({ resumeText, jobText, locale }),
    // Truthy checks only — never call methods on an argument here. The
    // `exerciseServiceHooks` smoke test renders every exported hook with a
    // single junk argument, so a `.trim()` in the render path throws.
    enabled: enabled && !!resumeText && !!jobText,
    staleTime: QUERY_TIMES.TEN_MIN,
  });
};
