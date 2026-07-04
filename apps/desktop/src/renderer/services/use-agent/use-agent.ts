import { useEffect } from 'react';
import { useMutation } from '@tanstack/react-query';

import type { AgentRunRequest, AgentStepEvent } from '@ajh/shared';

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
 * Subscribe to the `agent:step` narration stream. Mirrors
 * {@link useAutopilotStepEvents}. `AgentStepEvent.jobId` names the `agent.run`
 * the step belongs to (`AGENT_RUN_CONCURRENCY_MAX` allows more than one run in
 * flight) — a mounted subscriber receives steps from EVERY in-flight run, so
 * callers must filter on `event.jobId` against their own run's id (see
 * `PrepApplicationPanel`) rather than assuming every event is theirs.
 */
export const useAgentStepEvents = (onStep?: (event: AgentStepEvent) => void) => {
  const api = useAppClient();
  useEffect(() => {
    const off = api.agent.onStep((event: unknown) => {
      onStep?.(event as AgentStepEvent);
    });
    return () => off?.();
  }, [api, onStep]);
};
