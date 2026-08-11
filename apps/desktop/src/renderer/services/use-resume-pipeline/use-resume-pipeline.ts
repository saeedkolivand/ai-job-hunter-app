import { useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import type { AiStreamChunk, PipelineStageEvent } from '@ajh/shared';
import type { PipelineRunDetail, PipelineRunSummary } from '@ajh/shared/ipc';
import type {
  ResumePipelineRegenerateSectionRequest,
  ResumePipelineResolveFabricationRequest,
  ResumePipelineRunRequest,
} from '@ajh/shared/schemas';

import { useAppClient } from '@/providers/AppClientProvider';

import { keys, QUERY_TIMES } from '../query-client';

/**
 * How often a LIVE run's record is re-read while the pipeline is working.
 *
 * A poll rather than pure event-driven state, and deliberately so: a run
 * stopped at a stage BOUNDARY (the user cancelled, or the run deadline expired)
 * emits no `pipeline:stage` event for the stage it refused to start —
 * `RunHooks::before` returns `Err` before its own `start` emit — so there is no
 * event that says "this run ended". The record's `status` is the only signal
 * that always exists. Stage events still drive the progress UI and invalidate
 * this query, so the poll is a floor on latency, not the main mechanism.
 */
const LIVE_RUN_POLL_MS = 4_000;

/**
 * Start one staged résumé run. Resolves with `{ runId, jobId }` as soon as the
 * run is ADMITTED — the concurrency wait happens inside it, so a queued run
 * still returns its ids immediately and reports `job.queued`/`job.started` on
 * `jobs:event` with `{ ahead }`.
 *
 * Neither id means the run finished. `jobId` is the umbrella job: it is what
 * `jobs.cancel` takes, what the draft stage's display-only `ai:stream` deltas
 * carry — and, because that stream marks the job completed when the draft's
 * last delta lands, what fires `job.completed` several stages BEFORE the run
 * ends. Read `usePipelineRun(runId).status` for how a run actually ended.
 */
export const useStartResumePipelineRun = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: ResumePipelineRunRequest) => api.resumePipeline.run(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.pipeline.all }),
  });
};

/**
 * One run with its full stage trail, its report and its document.
 *
 * `live` keeps a poll running while the caller believes the run is in flight;
 * it is the caller's machine state, not the record's own status, because the
 * record is exactly what the poll is fetching. The query stops polling on its
 * own as soon as a fetched record reports a terminal status, so a caller that
 * forgets to flip `live` costs one extra request, never an endless poll.
 */
export const usePipelineRun = (runId: string | null | undefined, live = false) => {
  const api = useAppClient();
  return useQuery<PipelineRunDetail | null>({
    queryKey: keys.pipeline.run(runId ?? ''),
    queryFn: () => api.resumePipeline.get(runId ?? ''),
    enabled: !!runId,
    staleTime: live ? 0 : QUERY_TIMES.MEDIUM,
    refetchInterval: (query) =>
      live && query.state.data?.status === 'running' ? LIVE_RUN_POLL_MS : false,
  });
};

/** The retained runs for one posting, newest first (≤3 — the backend's own retention). */
export const usePipelineRunsForJob = (jobUrl: string | null | undefined) => {
  const api = useAppClient();
  return useQuery<PipelineRunSummary[]>({
    queryKey: keys.pipeline.runsForJob(jobUrl ?? ''),
    queryFn: () => api.resumePipeline.listForJob(jobUrl ?? ''),
    enabled: !!jobUrl,
  });
};

/**
 * Re-generate ONE section of a finished run and splice it back in.
 *
 * Rejected — with a plain validation error the caller must SURFACE rather than
 * swallow or retry — when the section key is outside the closed grammar
 * (`"header"` above all: the contact header is the editor's at export time,
 * ADR-0021), when the run is not the posting's LATEST, and when the shared
 * admission bucket is full (refused, not queued: it is a click).
 *
 * Invalidates both keys: the detail because its document and report changed,
 * the list because the run's metrics did.
 */
export const useRegenerateSection = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: ResumePipelineRegenerateSectionRequest) =>
      api.resumePipeline.regenerateSection(req),
    onSuccess: (detail: PipelineRunDetail) => {
      qc.setQueryData(keys.pipeline.run(detail.runId), detail);
      void qc.invalidateQueries({ queryKey: keys.pipeline.runsForJob(detail.jobUrl) });
    },
  });
};

/**
 * Record the user's Remove/Keep verdict on ONE flagged bullet. The run stays
 * `needsReview` until every finding has one, so the returned detail is what
 * flips the surface to `completed` — never an optimistic local edit.
 *
 * Only the detail is invalidated: a verdict changes the report, not the run
 * summaries the list renders. (Except the last one, which flips `status` — so
 * the list is refreshed too when the returned status is no longer `needsReview`.)
 */
export const useResolveFabrication = () => {
  const api = useAppClient();
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: ResumePipelineResolveFabricationRequest) =>
      api.resumePipeline.resolveFabrication(req),
    onSuccess: (detail: PipelineRunDetail) => {
      qc.setQueryData(keys.pipeline.run(detail.runId), detail);
      if (detail.status !== 'needsReview') {
        void qc.invalidateQueries({ queryKey: keys.pipeline.runsForJob(detail.jobUrl) });
      }
    },
  });
};

/**
 * Subscribe to the `pipeline:stage` progress stream.
 *
 * A mounted subscriber receives stages from EVERY in-flight run (the admission
 * bucket allows more than one), so a caller must filter on `event.runId`
 * against its own — mirrors `useAgentStepEvents`.
 */
export const usePipelineStageEvents = (onStage?: (event: PipelineStageEvent) => void) => {
  const api = useAppClient();
  useEffect(() => {
    const off = api.resumePipeline.onStage((event: unknown) => {
      onStage?.(event as PipelineStageEvent);
    });
    return () => off?.();
  }, [api, onStage]);
};

/**
 * Subscribe to the draft stage's deltas for ONE run, for DISPLAY ONLY.
 *
 * Deliberately not `awaitAiStream`: that helper resolves on the stream's `done`
 * frame, which for a staged run lands while `validate` and up to two repair
 * rounds are still ahead. Here there is no promise to resolve, so there is
 * nothing for a caller to mistake for completion — only text to render.
 *
 * `onDelta` is called with each answer token; reasoning/thinking frames and
 * frames belonging to other jobs are dropped.
 */
export const usePipelineDraftStream = (
  jobId: string | null | undefined,
  onDelta?: (delta: string) => void
) => {
  const api = useAppClient();
  useEffect(() => {
    if (!jobId || !onDelta) return;
    const off = api.ai.onStream((raw: unknown) => {
      const chunk = raw as AiStreamChunk;
      if (chunk.jobId !== jobId || chunk.thinking || !chunk.delta) return;
      onDelta(chunk.delta);
    });
    return () => off?.();
  }, [api, jobId, onDelta]);
};
