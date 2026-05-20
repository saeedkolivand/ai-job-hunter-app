import { useQuery } from '@tanstack/react-query';

import type { HybridSearchRequest } from '@ajh/shared';

import { keys } from './query-client';

export const useSearch = (req: HybridSearchRequest | null) =>
  useQuery({
    queryKey: keys.search.results(req?.query ?? ''),
    queryFn: () => window.api.search.hybrid(req as HybridSearchRequest),
    enabled: !!req?.query,
    staleTime: 60_000,
  });
