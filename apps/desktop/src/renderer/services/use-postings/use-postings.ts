import { useCallback } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { ScrapeBoardsRequest, ScrapeUrlRequest } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys, QUERY_TIMES } from '../query-client';

export const usePostings = () => {
  const api = useAppClient();
  return useQuery({ queryKey: keys.postings.all, queryFn: () => api.scrape.listPostings() });
};

export const useInteractions = (interactionType?: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.postings.interactions(interactionType),
    queryFn: () => api.scrape.listInteractions({ interactionType }),
  });
};

export const useScrapeBoards = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: ScrapeBoardsRequest) => api.scrape.boards(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.postings.all }),
  });
};

export const useInvalidatePostings = () => {
  const qc = useQueryClient();
  return useCallback(() => qc.invalidateQueries({ queryKey: keys.postings.all }), [qc]);
};

export const useScrapeUrl = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (req: ScrapeUrlRequest) => api.scrape.url(req) });
};

/** Resolve a single posting (incl. full description) from its URL, on demand.
 *  staleTime=INFINITE: a cached description is never re-fetched while it is
 *  still in the cache — no re-fetch on re-open, no flash of truncated text.
 *  gcTime=TEN_MIN: inactive entries are evicted after 10 min to bound memory
 *  as the user browses many jobs. Re-opening an evicted job uses the persisted
 *  backend description rather than re-fetching in the common case. */
export const useResolveJobUrl = (url: string, enabled = true) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.postings.resolve(url),
    queryFn: () => api.scrape.resolveUrl({ url }),
    enabled: enabled && !!url,
    staleTime: QUERY_TIMES.INFINITE,
    gcTime: QUERY_TIMES.TEN_MIN,
  });
};

/** Button-triggered variant of {@link useResolveJobUrl}: import a posting's full
 *  description from a pasted job URL (LinkedIn, Greenhouse, Lever, …). */
export const useImportJobUrl = () => {
  const api = useAppClient();
  return useMutation({ mutationFn: (url: string) => api.scrape.resolveUrl({ url }) });
};

export const useClearPostings = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => api.scrape.clearPostings(),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.postings.all }),
  });
};

/** Persist the full resolved description back to the backend cache.
 *  On success, invalidates the postings list so consumers re-render with the
 *  full description rather than the stale truncated snippet. */
export const useUpdatePostingDescription = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { id: string; description: string }) => api.scrape.updateDescription(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.postings.all }),
  });
};

export const usePersistJob = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { job: Record<string, unknown>; interactionType: string }) =>
      api.scrape.persistJob(req),
    // Invalidate both the postings list (so interaction badges update in PostingListItem)
    // and the interactions queries (typed views like 'viewed', 'opened', 'bookmarked').
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.postings.all });
      void qc.invalidateQueries({ queryKey: ['postings', 'interactions'] });
    },
  });
};
