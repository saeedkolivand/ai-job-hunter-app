import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { ScrapeBoardRequest, ScrapeUrlRequest } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys } from './query-client';

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

export const useScrapeBoard = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: ScrapeBoardRequest) => api.scrape.board(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.postings.all }),
  });
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
    staleTime: 5 * 60_000,
  });
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
