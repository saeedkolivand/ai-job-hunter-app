import { useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { JobEvent } from '@ajh/shared';

import { keys, queryClient } from './query-client';

export const useJobQueue = () =>
  useQuery({
    queryKey: keys.jobs.all,
    queryFn: () => window.api.jobs.list(),
  });

export const useJob = (jobId: string) =>
  useQuery({
    queryKey: keys.jobs.detail(jobId),
    queryFn: () => window.api.jobs.get(jobId),
    enabled: !!jobId,
    refetchInterval: 2_000, // active jobs update frequently
  });

export const useCancelJob = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (jobId: string) => window.api.jobs.cancel(jobId),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.jobs.all }),
  });
};

/** Fetch a single job outside of a component (e.g. inside event callbacks). */
export const fetchJob = (jobId: string) =>
  queryClient.fetchQuery({
    queryKey: keys.jobs.detail(jobId),
    queryFn: () => window.api.jobs.get(jobId),
    staleTime: 0,
  });

export const useRetryJob = () => {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (jobId: string) => window.api.jobs.retry(jobId),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.jobs.all }),
  });
};

/**
 * Subscribe to real-time job events.
 * Automatically invalidates the job queue cache on every event so the UI
 * stays in sync without manual polling.
 */
export const useJobEvents = (onEvent?: (event: JobEvent) => void) => {
  const qc = useQueryClient();
  useEffect(() => {
    const offRaw = window.api?.jobs.onEvent((event: unknown) => {
      void qc.invalidateQueries({ queryKey: keys.jobs.all });
      onEvent?.(event as JobEvent);
    });
    const off = offRaw as unknown as (() => void) | undefined;
    return () => off?.();
  }, [qc, onEvent]);
};
