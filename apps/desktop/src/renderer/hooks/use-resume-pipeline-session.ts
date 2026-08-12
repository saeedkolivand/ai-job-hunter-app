import { useCallback, useEffect, useRef, useState } from 'react';

import type { JobEvent, PipelineStageEvent } from '@ajh/shared';
import type { PipelineRunDetail } from '@ajh/shared/ipc';
import type { ResumePipelineRunRequest } from '@ajh/shared/schemas';

import { useMachine } from '@/hooks/use-machine';
import { errorDetail } from '@/lib/error-class';
import {
  type PipelineStageProgress,
  resumePipelineMachine,
  type ResumePipelineState,
  stageToEvent,
  statusToEvent,
} from '@/lib/machines/resume-pipeline.machine';
import { useCancelJob, useJobEvents } from '@/services/use-jobs';
import {
  usePipelineDraftStream,
  usePipelineRun,
  usePipelineStageEvents,
  useRefreshRunsForJobOnTerminal,
  useStartResumePipelineRun,
} from '@/services/use-resume-pipeline';

export type { PipelineStageProgress };

export interface ResumePipelineSession {
  state: ResumePipelineState;
  /** The run is still doing work — nothing final to show yet. */
  busy: boolean;
  runId: string | null;
  /** The umbrella job id: what `cancel` targets and what the draft stream keys on. */
  jobId: string | null;
  stage: PipelineStageProgress | null;
  /**
   * The draft stage's streamed text, for DISPLAY ONLY. Not the run's result —
   * `detail.resumeText` is, and it can differ by up to two repair rounds.
   */
  draft: string;
  /** The run record: the authority on status, report, metrics and document. */
  detail: PipelineRunDetail | null;
  /** A start failure, or the stopped reason of a run that ended badly. */
  error: string | null;
  starting: boolean;
  start: (req: ResumePipelineRunRequest) => Promise<string | null>;
  cancel: () => void;
  reset: () => void;
}

/**
 * Session state for one staged ("quality depth") résumé run.
 *
 * Owns four things a panel would otherwise re-derive: the coarse machine state,
 * the live stage counter, the display-only draft stream, and the run RECORD —
 * which is the only reliable completion signal (see
 * `resume-pipeline.machine`'s module doc). A run that was cancelled or hit its
 * deadline at a stage boundary produces no terminal stage event at all, so
 * terminal detection here reads `detail.status` and nothing else.
 *
 * @param initialRunId Reconnect target. A panel that remounts mid-run (the
 *   modal was closed and reopened, the route changed) passes the run id it
 *   remembers — the hook re-reads the record, replays its persisted stage trail
 *   to recover the counter, and resumes polling if the run is still going. The
 *   draft text is NOT recoverable that way (it was never persisted as a
 *   stream), so a reconnected run shows the finished document instead.
 */
export function useResumePipelineSession(
  initialRunId?: string | null,
  initialJobId?: string | null
): ResumePipelineSession {
  // A reconnect starts BUSY, not idle. Two things depend on it: `idle` accepts
  // only `START` (so a stage event arriving for the run we just re-attached to
  // would be dropped on the floor), and the record poll is gated on `busy` —
  // the one mechanism that notices a run which ended at a stage boundary and
  // therefore emitted no terminal event. A run that turns out to be finished
  // leaves `queued` on its first fetched record, which costs one render.
  const [state, send] = useMachine(resumePipelineMachine, initialRunId ? 'queued' : 'idle');
  const [runId, setRunId] = useState<string | null>(initialRunId ?? null);
  const [jobId, setJobId] = useState<string | null>(initialJobId ?? null);
  const [stage, setStage] = useState<PipelineStageProgress | null>(null);
  const [draft, setDraft] = useState('');
  const [error, setError] = useState<string | null>(null);

  const startRun = useStartResumePipelineRun();
  const cancelJob = useCancelJob();

  const busy = resumePipelineMachine.busyStates?.includes(state) ?? false;
  const {
    data: detail = null,
    isError: recordFailed,
    error: recordError,
  } = usePipelineRun(runId, busy);

  // `runId` is state, so the subscription handlers below need the CURRENT one
  // without re-subscribing on every change (a re-subscribe drops events in the
  // gap). Same discipline as `useNotificationEvents`' setter ref.
  const runIdRef = useRef(runId);
  runIdRef.current = runId;

  const handleStage = useCallback(
    (event: PipelineStageEvent) => {
      // Every in-flight run broadcasts on this channel — take only our own.
      if (!runIdRef.current || event.runId !== runIdRef.current) return;
      setStage({
        stage: event.stage,
        phase: event.phase,
        index: event.index,
        total: event.total,
        attempt: event.attempt,
        ...(event.issueCount != null ? { issueCount: event.issueCount } : {}),
        ...(event.criticalCount != null ? { criticalCount: event.criticalCount } : {}),
      });
      const next = stageToEvent(event.stage, event.phase);
      if (next) send(next);
    },
    [send]
  );
  usePipelineStageEvents(handleStage);

  const appendDraft = useCallback((delta: string) => setDraft((prev) => prev + delta), []);
  usePipelineDraftStream(jobId, appendDraft);

  /**
   * The failures the run RECORD can never report, and the one job event that can.
   *
   * `resume_pipeline_run` returns its two ids immediately and writes the
   * `pipeline_runs` row inside the spawned task — AFTER admission and after it
   * resolves the depth, the provider, the résumé and the cached posting. Each of
   * those five can fail (a full queue, no configured provider, a deleted résumé,
   * a posting that is not in the live cache, `depth: "max"`) and lands on
   * `job_fail` with NO row ever written; `get(runId)` then answers `null` — a
   * real answer, so the poll correctly stops — and `detail.status`, the only
   * completion signal this hook has, never exists. The same hole exists at the
   * other end: if the final `upsert_run` is what failed, the row is left saying
   * `running` forever. Either way the session spins on a run that has ended.
   *
   * So the umbrella job's `job.failed` is consumed WHILE THE MACHINE IS BUSY —
   * not "while no record exists", which misses the stuck-`running` row. It is
   * safe against the record because the backend's terminal arms are exclusive:
   * `job_fail` is emitted only when the row is (or would be) `failed`; a
   * cancelled run gets `job_cancel`, and a deadline-stopped run that still saved
   * its document gets `job_complete` precisely so the renderer does not discard
   * a document the row calls reviewable. A run that already reached a terminal
   * state is left alone, error text included. `job.completed` is never read here
   * for the reason the module doc gives: the draft stream fires it mid-run.
   */
  const jobIdRef = useRef(jobId);
  jobIdRef.current = jobId;
  const busyRef = useRef(busy);
  busyRef.current = busy;
  const handleJobEvent = useCallback(
    (event: JobEvent) => {
      if (event.type !== 'job.failed') return;
      if (!jobIdRef.current || event.jobId !== jobIdRef.current) return;
      if (!busyRef.current) return;
      console.error('[resumePipeline] the umbrella job failed', { jobId: event.jobId });
      setError(typeof event.data === 'string' && event.data ? event.data : null);
      send('ERROR');
    },
    [send]
  );
  useJobEvents(handleJobEvent);

  // Terminal detection, and the ONLY place it happens: the record's status.
  // Not the last stage's `finish` (a boundary stop never reaches one), not the
  // umbrella job's `job.completed` (the draft stream fires that mid-run), not
  // `awaitAiStream` (same stream, same problem).
  const status = detail?.status;
  useEffect(() => {
    if (!status) return;
    const next = statusToEvent(status);
    if (next) send(next);
  }, [status, send]);

  // …and the same discovery is the only thing that can tell the posting's run
  // LIST its run just ended. Nothing was clicked, so none of the three
  // action-driven invalidators fires, and the list has no poll of its own: left
  // alone it renders this run as "Running" indefinitely. The run record is also
  // the authority on which posting to refresh — `jobUrl` off the row, never a
  // caller-supplied one that could disagree with it.
  useRefreshRunsForJobOnTerminal(detail?.jobUrl, status);

  /**
   * The record read itself failed and NOTHING has been fetched yet.
   *
   * That combination is fatal to this session and has to be said out loud: the
   * status is the only completion signal, so a session that never gets a first
   * record has no way to ever leave a busy state — the panel spins forever on a
   * request that already gave up. Surfacing it puts the machine in `error`,
   * where the surface's existing retry (`start` again) and cancel affordances
   * live.
   *
   * Gated on `!detail` deliberately: once a record HAS landed, a later poll
   * failure is a blip, the run is still going, and react-query keeps polling —
   * killing a live run over one dropped read would be the opposite mistake.
   */
  useEffect(() => {
    if (!recordFailed || detail) return;
    console.error('[resumePipeline] reading the run record failed', {
      error: errorDetail(recordError),
    });
    setError(recordError instanceof Error ? recordError.message : String(recordError));
    send('ERROR');
  }, [recordFailed, detail, recordError, send]);

  // Replay a reconnected run's persisted stage trail so the counter is right
  // after a remount. Only while the machine is still at `idle`/`queued` — once
  // live events are arriving they are fresher than the trail.
  const events = detail?.events;
  const replayed = useRef<string | null>(null);
  useEffect(() => {
    if (!runId || !events?.length || replayed.current === runId) return;
    replayed.current = runId;
    const last = events[events.length - 1];
    if (!last) return;
    setStage(
      (current) =>
        current ?? {
          stage: last.stage,
          phase: last.phase,
          index: events.filter((e) => e.phase === 'start').length - 1,
          total: 0,
          attempt: 1,
        }
    );
    const next = stageToEvent(last.stage, last.phase);
    if (next) send(next);
  }, [runId, events, send]);

  const start = useCallback(
    async (req: ResumePipelineRunRequest) => {
      setError(null);
      setStage(null);
      setDraft('');
      replayed.current = null;
      send('START');
      try {
        const started = await startRun.mutateAsync(req);
        setRunId(started.runId);
        setJobId(started.jobId);
        return started.runId;
      } catch (err) {
        console.error('[resumePipeline] start failed', { error: errorDetail(err) });
        setError(err instanceof Error ? err.message : String(err));
        send('ERROR');
        return null;
      }
    },
    [send, startRun]
  );

  /**
   * Cancel through the ordinary job path — the umbrella id from `run`. It
   * aborts the draft stream immediately and every other stage at its next
   * boundary, so the machine is NOT flipped to `cancelled` here: the backend
   * decides (a run that finished a millisecond earlier finished), and the
   * record says so.
   */
  const cancel = useCallback(() => {
    if (!jobId) return;
    cancelJob.mutate(jobId);
  }, [cancelJob, jobId]);

  const reset = useCallback(() => {
    send('RESET');
    setRunId(null);
    setJobId(null);
    setStage(null);
    setDraft('');
    setError(null);
    replayed.current = null;
  }, [send]);

  return {
    state,
    busy,
    runId,
    jobId,
    stage,
    draft,
    detail,
    error,
    starting: startRun.isPending,
    start,
    cancel,
    reset,
  };
}
