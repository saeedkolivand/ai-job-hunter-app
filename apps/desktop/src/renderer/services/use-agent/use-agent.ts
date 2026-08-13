import { useEffect, useRef } from 'react';
import { useMutation } from '@tanstack/react-query';

import type { AgentConfirmRequest, AgentRunRequest, AgentStepEvent } from '@ajh/shared';

import { useAppClient } from '@/providers/AppClientProvider';

/**
 * Kick off the "prep this application" agentic run. Resolves immediately with
 * `{ jobId }`; progress streams via `useAgentStepEvents` and the run finishes
 * as a `jobs:event` (consume with `useJobEvents` / `useJob`, same as any other
 * background job). Mirrors {@link useRunAutopilot}'s run-mutation shape.
 */
export const useAgentRun = () => {
  const api = useAppClient();
  return useMutation({
    mutationFn: (req: AgentRunRequest) => api.agent.run(req),
  });
};

/**
 * Resolve a suspended Write confirmation (the Phase-3 human-in-the-loop
 * confirm gate) — approve, edit-then-approve, or deny the pending call named
 * by a `confirm_request` step. `{ ok: false }` means the call is no longer
 * actionable (already resolved, timed out, cancelled, or unknown) — the
 * caller (`AgentConfirm`) surfaces that and stops treating it as pending; it
 * never throws for that case.
 */
export const useAgentConfirm = () => {
  const api = useAppClient();
  return useMutation({
    mutationFn: (req: AgentConfirmRequest) => api.agent.confirm(req),
  });
};

/**
 * Subscribe to the `agent:step` narration stream. Mirrors
 * {@link useAutopilotStepEvents}. `AgentStepEvent.jobId` names the `agent.run`
 * the step belongs to (`AGENT_RUN_CONCURRENCY_MAX` allows more than one run in
 * flight) — a mounted subscriber receives steps from EVERY in-flight run, so
 * callers must filter on `event.jobId` against their own run's id (see
 * `PrepApplicationPanel`) rather than assuming every event is theirs.
 */
export const useAgentStepEvents = (onStep?: (event: AgentStepEvent) => void) => {
  const api = useAppClient();
  // Keep the latest handler in a ref so the listener subscribes ONCE — the same
  // shape `useJobEvents` already uses, and for the same reason. `onStep` in the
  // dependency array re-runs this effect whenever the caller's callback changes
  // identity, which for every real caller happens exactly when its run id
  // arrives (it is a `useCallback` keyed on that id). Tauri's `listen` attaches
  // ASYNCHRONOUSLY, so unsubscribe-then-resubscribe leaves a window with no
  // native listener attached — right at the moment the run starts emitting. A
  // `confirm_request` lost in that window leaves a SUSPENDED run rendered as a
  // running one until the 300 s gate timeout denies it.
  const handlerRef = useRef(onStep);
  handlerRef.current = onStep;
  useEffect(() => {
    const off = api.agent.onStep((event: unknown) => {
      handlerRef.current?.(event as AgentStepEvent);
    });
    return () => off?.();
  }, [api]);
};
