import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys, QUERY_TIMES } from '../query-client';

/** The web-search backend settings for company research: `{ searxngUrl }`, the URL of a
 *  SearXNG-compatible search server (SearXNG, RustXNG), `null` when unset. */
export const useSearchSettings = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.ai.searchSettings,
    queryFn: () => api.ai.searchSettings(),
    staleTime: QUERY_TIMES.MEDIUM,
  });
};

/**
 * Set (or clear, with `null`) the SearXNG-compatible server URL.
 *
 * Resolves the FRESH settings, or an `{ error }` union on a server-side rejection (not
 * http/https, no host, a query or fragment). Narrowed and thrown here so a rejection reaches
 * `onError`, and the backend's answer is written straight into the cache.
 */
export const useSetSearchSettings = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (req: { searxngUrl: string | null }) => {
      const result = await api.ai.setSearchSettings(req);
      if ('error' in result) throw new Error(result.error);
      return result;
    },
    onSuccess: (fresh) => qc.setQueryData(keys.ai.searchSettings, fresh),
  });
};
