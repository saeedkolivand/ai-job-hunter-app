import { useQuery } from '@tanstack/react-query';

import type { HybridSearchRequest } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from './query-client';

export const useSearch = (req: HybridSearchRequest | null) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.search.results(req?.query ?? ''),
    queryFn: () => api.search.hybrid(req as HybridSearchRequest),
    enabled: !!req?.query,
    staleTime: 60_000,
  });
};
