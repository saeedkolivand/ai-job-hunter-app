import { useEffect, useMemo, useRef } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { JobEvent, JobRecord } from '@ajh/shared';

import { getClient, useAppClient } from '@/providers/AppClientProvider';

import { keys, queryClient } from '../query-client';

export const useJobQueue = () => {
  const api = useAppClient();
  return useQuery({ queryKey: keys.jobs.all, queryFn: () => api.jobs.list() });
};

/**
 * Single-job detail.
 *
 * HARD REQUIREMENT: a `useJobEvents()` subscription MUST be mounted somewhere in
 * the tree, or this query never updates. There is no `refetchInterval` — job
 * status is event-driven only. `useJobEvents` invalidates `keys.jobs.all`
 * (`['jobs']`) on every event type (queued/started/progress/stream/completed/
 * failed/cancelled); because React Query invalidation is prefix-based,
 * invalidating `['jobs']` also marks the detail query `['jobs', id]` stale, so
 * the detail refetches on every transition without a polling timer. Current
 * callers (worker-activity / job views) already mount it; an isolated future
 * use of `useJob` WITHOUT a mounted `useJobEvents()` will silently stop
 * receiving updates — do not remove the subscription.
 */
export const useJob = (jobId: string) => {
  const api = useAppClient();
  return useQuery({
    queryKey: keys.jobs.detail(jobId),
    queryFn: () => api.jobs.get(jobId),
    enabled: !!jobId,
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

export interface WorkerActivityByKind {
  kind: string;
  label: string;
  count: number;
}

export interface WorkerActivity {
  /** Number of jobs currently running (or streaming). */
  active: number;
  /** Number of jobs waiting in the queue (pending). */
  queued: number;
  /** The running/streaming job records. */
  running: JobRecord[];
  /** The queued (pending) job records. */
  queuedJobs: JobRecord[];
  /** Active jobs grouped by kind, with localized labels. */
  byKind: WorkerActivityByKind[];
  /** Convenience flag — true when any job is running. */
  isActive: boolean;
}

/**
 * Live view of background-job activity (a "worker" = any active background job:
 * AI generation, scraping, autopilot, embeddings, extraction).
 *
 * Reads the job queue and subscribes to job events (which already invalidate the
 * queue on every event), so consumers update in real time. The kind→label map is
 * passed in as a param so this service stays free of i18n/feature coupling — pass
 * `useKindLabelMap()` from the caller.
 */
export const useWorkerActivity = (kindLabelMap: Record<string, string>): WorkerActivity => {
  const { data } = useJobQueue();
  // Subscribe to live job events; this also keeps `keys.jobs.all` fresh.
  useJobEvents();

  return useMemo(() => {
    const jobs = data ?? [];
    const running = jobs.filter((j) => j.status === 'running' || j.status === 'streaming');
    const queuedJobs = jobs.filter((j) => j.status === 'queued');

    const counts = new Map<string, number>();
    for (const job of running) counts.set(job.kind, (counts.get(job.kind) ?? 0) + 1);
    const byKind: WorkerActivityByKind[] = [...counts.entries()].map(([kind, count]) => ({
      kind,
      label: kindLabelMap[kind] ?? kind,
      count,
    }));

    return {
      active: running.length,
      queued: queuedJobs.length,
      running,
      queuedJobs,
      byKind,
      isActive: running.length > 0,
    };
  }, [data, kindLabelMap]);
};

export const useJobEvents = (onEvent?: (event: JobEvent) => void) => {
  const api = useAppClient();
  const qc = useQueryClient();
  // Keep the latest handler in a ref so the listener can subscribe ONCE.
  // Re-subscribing on every render (the handler is usually an inline closure)
  // races with the async Tauri `listen`, leaving windows where no native
  // listener is attached — during which a `job.completed` event is lost.
  const handlerRef = useRef(onEvent);
  handlerRef.current = onEvent;
  useEffect(() => {
    const offRaw = api.jobs.onEvent((event: unknown) => {
      void qc.invalidateQueries({ queryKey: keys.jobs.all });
      handlerRef.current?.(event as JobEvent);
    });
    const off = offRaw as unknown as (() => void) | undefined;
    return () => off?.();
  }, [api, qc]);
};
