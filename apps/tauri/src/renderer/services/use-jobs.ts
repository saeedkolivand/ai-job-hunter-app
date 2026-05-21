import { useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { JobEvent } from '@ajh/shared';

import { getClient, useAppClient } from '@/providers/AppClientProvider';

import { keys, queryClient } from './query-client';

export const useJobQueue = () => {
  const api = useAppClient();
  return useQuery({ queryKey: keys.jobs.all, queryFn: () => api.jobs.list() });
};

export const useJob = (jobId: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.jobs.detail(jobId),
    queryFn: () => api.jobs.get(jobId),
    enabled: !!jobId,
    refetchInterval: 2_000,
  });
};

export const useCancelJob = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (jobId: string) => api.jobs.cancel(jobId),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.jobs.all }),
  });
};

/** Fetch a single job outside of a component (e.g. inside event callbacks). */
export const fetchJob = (jobId: string) =>
  queryClient.fetchQuery({
    queryKey: keys.jobs.detail(jobId),
    queryFn: () => getClient().jobs.get(jobId),
    staleTime: 0,
  });

export const useRetryJob = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (jobId: string) => api.jobs.retry(jobId),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.jobs.all }),
  });
};

export const useJobEvents = (onEvent?: (event: JobEvent) => void) => {
  const api = useAppClient();
  const qc = useQueryClient();
  useEffect(() => {
    const offRaw = api.jobs.onEvent((event: unknown) => {
      void qc.invalidateQueries({ queryKey: keys.jobs.all });
      onEvent?.(event as JobEvent);
    });
    const off = offRaw as unknown as (() => void) | undefined;
    return () => off?.();
  }, [api, qc, onEvent]);
};
