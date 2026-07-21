import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { DiscoverySearchRequest, DiscoveryStarRequest } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from '../query-client';

/**
 * Discovery (ADR-030): the passively-harvested ATS company slugs that back the
 * ScrapeForm slug typeahead and the watched-company autopilot target.
 */

/**
 * Typeahead search over discovered/seeded company slugs. Debouncing is the
 * caller's job — pass an already-debounced `query`. Keyed per query so React
 * Query caches each distinct prefix.
 */
export const useCompanySearch = (req: DiscoverySearchRequest) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.discovery.search(req.query),
    queryFn: () => api.discovery.searchCompanies(req),
  });
};

/** The user's watched (starred) companies. */
export const useWatchedCompanies = () => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.discovery.watched,
    queryFn: () => api.discovery.watched(),
  });
};

/**
 * Star / unstar a company. The command RESOLVES (never rejects) an `{ error }`
 * union on failure, so we narrow it and THROW — otherwise React Query fires
 * `onSuccess` on a silent failure (the #756 lesson; mirrors `useMarkNotDuplicate`).
 * On success, invalidates the discovery queries so the typeahead + watched list
 * re-render with the new star state.
 */
export const useSetStarred = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (req: DiscoveryStarRequest) => {
      const result = await api.discovery.setStarred(req);
      if ('error' in result) throw new Error(result.error);
      return result;
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.discovery.all });
    },
  });
};
