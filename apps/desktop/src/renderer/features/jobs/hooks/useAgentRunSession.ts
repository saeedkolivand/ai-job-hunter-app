import { useCallback, useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';

import type { AgentConfirmPayload, AgentFlowKind, AgentStepEvent, JobEvent } from '@ajh/shared';

import { useMachine } from '@/hooks/use-machine';
import { agentRunMachine, type AgentRunState, stepToEvent } from '@/lib/machines/agent-run.machine';
import { useAgentRun, useAgentStepEvents, useCancelJob, useJob, useJobEvents } from '@/services';
import { keys } from '@/services/query-client';

export interface AgentRunSessionOptions {
  /** Which flow (`AGENT_FLOW_KINDS`) `start` runs. Stated, never defaulted —
   *  the wire's default exists for older callers, not for new ones. */
  kind: AgentFlowKind;
  /** The candidate's MASTER résumé id (`ToolContext::resume_id`, the ground
   *  truth claims are checked against — never a generation's id). `null`
   *  disables `start`. */
  resumeId: string | null;
  /** The posting id the run is about. */
  jobId: string;
  /** Copy for a run the backend failed without a readable message. */
  fallbackError: string;
}

/**
 * The wire `StoppedReason` a COMPLETED run recorded, or `null` when the payload
 * carried none.
 *
 * A completed agent job is not the same as a finished one: the loop also
 * "completes" at its step / token / tool-call ceiling, so a surface that reads
 * `job.completed` as success alone reports a truncated run as a clean finish.
 * Guarded rather than cast — this is an IPC payload, contract-optional.
 */
function readStoppedReason(data: unknown): string | null {
  if (!data || typeof data !== 'object') return null;
  const reason = (data as { stoppedReason?: unknown }).stoppedReason;
  return typeof reason === 'string' && reason.trim() ? reason : null;
}

export interface AgentRunSession {
  state: AgentRunState;
  steps: AgentStepEvent[];
  /** See {@link readStoppedReason} — `null` until a run completes (and on a run
   *  that completed without recording one). */
  stoppedReason: string | null;
  /** The one Write action currently suspended awaiting the user's decision. */
  pendingConfirm: AgentConfirmPayload | null;
  /** The background job id of the run in flight (`null` until `agent.run`'s
   *  round-trip resolves). */
  runJobId: string | null;
  error: string | null;
  /** In flight — INCLUDING suspended on a confirm, where the run is genuinely
   *  blocked on the user rather than idle. */
  busy: boolean;
  /** Stopped or failed before finishing: whatever step was in flight never
   *  completed. `done` is deliberately excluded. */
  interrupted: boolean;
  /** Anything to show at all — a run has been started this session. */
  active: boolean;
  stopRequested: boolean;
  start: () => Promise<void>;
  stop: () => void;
  /** Clear a finished run's log so the surface goes back to offering a fresh
   *  one. Refuses while a run is in flight (that is what `stop` is for). */
  dismiss: () => void;
  /** Called by the confirm card once a decision actually took effect. */
  resolveConfirm: (event: 'APPROVE' | 'DENY') => void;
}

/**
 * One `agent.run` lifecycle — the wiring every agentic surface needs, with no
 * copy and no layout of its own.
 *
 * Extracted from `PrepApplicationPanel`, whose inline version this mirrors
 * trap for trap (that panel is deliberately left alone here — a shipped flow
 * is not worth re-plumbing to prove a hook; it can adopt this later):
 *
 * - **Every subscriber hears every run.** `agent:step` and `jobs:event` are
 *   shared channels (`AGENT_RUN_CONCURRENCY_MAX` > 1, and a panel can outlive
 *   the run it started), so both handlers filter on `event.jobId === runJobId`.
 *   `activeRef` covers the sliver before `runJobId` is set and gives `start` a
 *   pre-render "already running?" read, so a double-click cannot spawn two runs.
 * - **A confirm is superseded by any later step.** The backend's 300 s confirm
 *   timeout denies-and-resumes server-side, so a fresh `turn` can arrive with
 *   no resolve ever reaching the renderer — the stale card is cleared rather
 *   than left next to live narration (fail-closed: a late decision on that
 *   callId returns `{ ok: false }` anyway).
 * - **A fast fail can beat the subscription.** The backend can emit the
 *   terminal `jobs:event` before `agent.run`'s round-trip resolves, so that
 *   event is dropped and the machine would sit busy forever. Once `runJobId`
 *   is known the job RECORD is reconciled once (`reconciledRef`), and only
 *   while still busy, so it never overrides the normal event path. The
 *   improve-resume flow fails fast on purpose (no generation for this posting
 *   yet), which makes this the likely path there rather than the rare one.
 */
export function useAgentRunSession({
  kind,
  resumeId,
  jobId,
  fallbackError,
}: AgentRunSessionOptions): AgentRunSession {
  const [state, send] = useMachine(agentRunMachine, 'idle');
  const [steps, setSteps] = useState<AgentStepEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [stoppedReason, setStoppedReason] = useState<string | null>(null);
  const [stopRequested, setStopRequested] = useState(false);
  const [runJobId, setRunJobId] = useState<string | null>(null);
  const [pendingConfirm, setPendingConfirm] = useState<AgentConfirmPayload | null>(null);
  const activeRef = useRef(false);
  const reconciledRef = useRef(false);
  /** An approved Write whose effect has not been refetched yet — see
   *  {@link refreshWrittenDocument}. */
  const wroteRef = useRef(false);

  const runAgent = useAgentRun();
  const cancelJob = useCancelJob();
  const queryClient = useQueryClient();

  /**
   * Refetch what an approved Write changed. Both gated writes land in the
   * `ai_generations` aggregate for this posting, which is ALSO where the
   * pipeline's run detail reads its résumé text and quality report from
   * (`resume_pipeline_get` → `find_for_job`) — so a saved review leaves the
   * panel showing the pre-save document with no way back: the run query has no
   * interval once the run is terminal, and the client's refetch-on-focus and
   * -reconnect are both off. The card would say "saved" next to text that
   * disagrees with it, in one viewport.
   *
   * `keys.pipeline.all` is the PREFIX (`['pipeline']`), so it covers the run
   * detail and the runs list in one call rather than relying on a key built
   * from a possibly-null run id.
   */
  const refreshWrittenDocument = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: keys.pipeline.all });
    void queryClient.invalidateQueries({ queryKey: keys.aiGenerations.all });
  }, [queryClient]);

  const busy = state !== 'idle' && state !== 'done' && state !== 'error' && state !== 'cancelled';
  const interrupted = state === 'error' || state === 'cancelled';

  const handleStep = useCallback(
    (event: AgentStepEvent) => {
      if (!activeRef.current || event.jobId !== runJobId) return;
      setSteps((prev) => [...prev, event]);
      setPendingConfirm(
        event.kind === 'confirm_request' && event.confirm ? event.confirm : null // any other step supersedes a stale confirm
      );
      // The write runs AFTER `agent.confirm` returns (the command only hands the
      // decision to the suspended tool), so the refetch fired on approve can
      // race it. The next step is proof the tool finished — refetch again, once.
      if (wroteRef.current && event.kind !== 'confirm_request') {
        wroteRef.current = false;
        refreshWrittenDocument();
      }
      const machineEvent = stepToEvent(event, kind);
      if (machineEvent) send(machineEvent);
    },
    [runJobId, send, kind, refreshWrittenDocument]
  );
  useAgentStepEvents(handleStep);

  /** Shared terminal handling for the live event path and the reconciliation
   *  fallback below — same effects whichever observes the finish first. */
  const finishRun = useCallback(
    (outcome: 'completed' | 'failed' | 'cancelled', data?: unknown) => {
      activeRef.current = false;
      setPendingConfirm(null);
      // Backstop for a run that ended without another step after the write
      // (the summary turn is not guaranteed — a ceiling can end the loop first).
      if (wroteRef.current) {
        wroteRef.current = false;
        refreshWrittenDocument();
      }
      if (outcome === 'completed') {
        setStoppedReason(readStoppedReason(data));
        send('COMPLETE');
      } else if (outcome === 'failed') {
        setError(typeof data === 'string' && data.trim() ? data : fallbackError);
        send('ERROR');
      } else {
        // A deliberate Stop is not a failure — its own state and copy.
        // Cancelling a suspended run resolves its pending confirm server-side
        // too (`AgentGate::remove`), so nothing is left here to act on.
        send('CANCEL');
      }
    },
    [send, fallbackError, refreshWrittenDocument]
  );

  const handleJobEvent = useCallback(
    (event: JobEvent) => {
      if (!activeRef.current || event.jobId !== runJobId) return;
      if (event.type === 'job.completed') finishRun('completed', event.data);
      else if (event.type === 'job.failed') finishRun('failed', event.data);
      else if (event.type === 'job.cancelled') finishRun('cancelled');
    },
    [runJobId, finishRun]
  );
  useJobEvents(handleJobEvent);

  const jobQuery = useJob(runJobId ?? '');
  useEffect(() => {
    if (!runJobId || reconciledRef.current || !busy) return;
    const job = jobQuery.data;
    if (
      !job ||
      (job.status !== 'completed' && job.status !== 'failed' && job.status !== 'cancelled')
    ) {
      return;
    }
    reconciledRef.current = true;
    if (job.status === 'completed') finishRun('completed', job.result);
    else if (job.status === 'failed') finishRun('failed', job.error);
    else finishRun('cancelled');
  }, [runJobId, jobQuery.data, busy, finishRun]);

  const start = useCallback(async () => {
    // Every terminal state accepts START directly (see `agentRunMachine`), so
    // the re-run affordance must itself refuse while a run is already live.
    if (!resumeId || busy || activeRef.current) return;
    setSteps([]);
    setError(null);
    setStoppedReason(null);
    setStopRequested(false);
    setRunJobId(null);
    setPendingConfirm(null);
    activeRef.current = true;
    reconciledRef.current = false;
    wroteRef.current = false;
    send('START');
    try {
      // Routing (provider/model/baseUrl) is backend-owned — resolved server-side
      // from the active-provider store. The request carries identity + which
      // flow, and nothing else.
      const started = await runAgent.mutateAsync({ resumeId, jobId, kind });
      setRunJobId(started.jobId);
    } catch (err) {
      activeRef.current = false;
      setError(err instanceof Error ? err.message : fallbackError);
      send('ERROR');
    }
  }, [resumeId, busy, runAgent, jobId, kind, send, fallbackError]);

  const stop = useCallback(() => {
    if (!runJobId) return;
    setStopRequested(true);
    void cancelJob.mutateAsync(runJobId).catch(() => {});
  }, [runJobId, cancelJob]);

  const dismiss = useCallback(() => {
    if (busy || activeRef.current) return;
    setSteps([]);
    setError(null);
    setStoppedReason(null);
    setRunJobId(null);
    setPendingConfirm(null);
    send('RESET');
  }, [busy, send]);

  const resolveConfirm = useCallback(
    (event: 'APPROVE' | 'DENY') => {
      setPendingConfirm(null);
      if (event === 'APPROVE') {
        // Fire now for the common case, and remember it so the guaranteed-after
        // refetch still happens (see `refreshWrittenDocument`). A DENY changed
        // nothing, so it invalidates nothing.
        wroteRef.current = true;
        refreshWrittenDocument();
      }
      send(event);
    },
    [send, refreshWrittenDocument]
  );

  return {
    state,
    steps,
    stoppedReason,
    pendingConfirm,
    runJobId,
    error,
    busy,
    interrupted,
    active: state !== 'idle',
    stopRequested,
    start,
    stop,
    dismiss,
    resolveConfirm,
  };
}
