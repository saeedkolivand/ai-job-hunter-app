import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys, QUERY_TIMES } from '../query-client';

/** Every explicitly-set per-stage override, keyed by pipeline stage name.
 *
 *  An ABSENT key means "run this stage on the active provider" — not "an
 *  override that happens to equal the active provider". Nothing is written on
 *  the user's behalf, so a suggestion stays a suggestion until it is applied
 *  (writing today's default back as a row would pin that stage to today's model
 *  forever). */
export const useStageOverrides = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.ai.stageOverrides,
    queryFn: () => api.ai.stageOverrides(),
    staleTime: QUERY_TIMES.MEDIUM,
  });
};

/**
 * Point ONE stage at a provider + model.
 *
 * Both writers resolve the FRESH map, or an `{ error }` union on a server-side
 * rejection (unknown stage, unknown/unconfigured provider, a cross-family
 * model, a rejected base URL, a window outside 512–131072). Narrowed and
 * thrown here so a rejection reaches `onError`, and the returned map is written
 * straight into the cache — the backend's answer, never an optimistic merge.
 */
export const useSetStageOverride = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (req: {
      stage: string;
      provider: string;
      model?: string;
      baseUrl?: string;
      contextWindow?: number;
    }) => {
      const result = await api.ai.setStageOverride(req);
      if ('error' in result) throw new Error(result.error as string);
      return result;
    },
    onSuccess: (fresh) => qc.setQueryData(keys.ai.stageOverrides, fresh),
  });
};

/** Return ONE stage to the active provider. A stage with no override is a
 *  no-op server-side, not an error — clear without reading first. */
export const useClearStageOverride = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (stage: string) => {
      const result = await api.ai.clearStageOverride({ stage });
      if ('error' in result) throw new Error(result.error as string);
      return result;
    },
    onSuccess: (fresh) => qc.setQueryData(keys.ai.stageOverrides, fresh),
  });
};
