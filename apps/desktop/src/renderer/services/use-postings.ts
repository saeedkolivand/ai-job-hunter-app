import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { ScrapeBoardRequest, ScrapeUrlRequest } from '@ajh/shared';

import { keys } from './query-client';

export const usePostings = () =>
  useQuery({
    queryKey: keys.postings.all,
    queryFn: () => window.api.scrape.listPostings(),
  });

export const useInteractions = (interactionType?: string) =>
  useQuery({
    queryKey: keys.postings.interactions(interactionType),
    queryFn: () => window.api.scrape.listInteractions({ interactionType }),
  });

export const useScrapeBoard = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: ScrapeBoardRequest) => window.api.scrape.board(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.postings.all }),
  });
};

export const useScrapeUrl = () =>
  useMutation({
    mutationFn: (req: ScrapeUrlRequest) => window.api.scrape.url(req),
  });

export const useClearPostings = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => window.api.scrape.clearPostings(),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.postings.all }),
  });
};

export const usePersistJob = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: { job: Record<string, unknown>; interactionType: string }) =>
      window.api.scrape.persistJob(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.postings.interactions() }),
  });
};

export const useExportData = () =>
  useMutation({ mutationFn: () => window.api.scrape.exportData() });

export const useImportData = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => window.api.scrape.importData(),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.postings.all }),
  });
};
