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

/** Resolve a single posting (incl. full description) from its URL, on demand. */
export const useResolveJobUrl = (url: string, enabled = true) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.postings.resolve(url),
    queryFn: () => api.scrape.resolveUrl({ url }),
    enabled: enabled && !!url,
    staleTime: QUERY_TIMES.VERY_LONG,
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

export const usePersistJob = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { job: Record<string, unknown>; interactionType: string }) =>
      api.scrape.persistJob(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.postings.interactions() }),
  });
};
