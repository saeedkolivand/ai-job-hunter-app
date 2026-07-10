import { useCallback, useReducer, useState } from 'react';

import type { Autopilot, AutopilotStepEvent } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';

import type { RunStateMap, StepLogMap } from '@/features/autopilot/constants';
import { transition as machineTransition } from '@/lib/machine';
import { autopilotRunMachine, stepToEvent } from '@/lib/machines/autopilot-run.machine';
import {
  useAutopilotStepEvents,
  usePauseAutopilot,
  useRemoveAutopilot,
  useResumeAutopilot,
  useRunAutopilot,
} from '@/services';

export function useAutopilotRun() {
  const { t } = useTranslation();
  const [runStates, setRunStates] = useReducer(
    (prev: RunStateMap, patch: Partial<RunStateMap>): RunStateMap =>
      ({ ...prev, ...patch }) as RunStateMap,
    {} as RunStateMap
  );
  const [stepLogs, setStepLogs] = useReducer(
    (prev: StepLogMap, patch: Partial<StepLogMap>): StepLogMap =>
      ({ ...prev, ...patch }) as StepLogMap,
    {} as StepLogMap
  );
  const [error, setError] = useState<string | null>(null);

  const runAutopilot = useRunAutopilot();
  const pauseAutopilot = usePauseAutopilot();
  const resumeAutopilot = useResumeAutopilot();
  const removeAutopilot = useRemoveAutopilot();

  const handleStep = useCallback(
    (event: AutopilotStepEvent) => {
      const ev = stepToEvent(event.step);
      if (ev) {
        setRunStates({
          [event.autopilotId]: machineTransition(
            autopilotRunMachine,
            runStates[event.autopilotId] ?? 'idle',
            ev
          ),
        });
      }
      setStepLogs({
        [event.autopilotId]: [
          ...(stepLogs[event.autopilotId] ?? []).slice(-49),
          { step: event.step, detail: event.detail, ts: Date.now() },
        ],
      });
    },
    [runStates, stepLogs]
  );

  useAutopilotStepEvents(handleStep);

  const handleRun = async (id: string) => {
    setRunStates({ [id]: 'scraping' });
    setStepLogs({ [id]: [] });
    try {
      // `autopilot_run` RESOLVES (not rejects) with an `{ error }` payload when a
      // scrape fails or the id is unknown, so a resolved value is not proof of
      // success — inspect it and route to the SAME error state the reject path
      // below uses instead of falsely reporting 'done'.
      const result = await runAutopilot.mutateAsync(id);
      if (result.error) {
        setRunStates({ [id]: 'error' });
        setError(result.error);
        return;
      }
      setRunStates({ [id]: 'done' });
    } catch (err) {
      setRunStates({ [id]: 'error' });
      setError(err instanceof Error ? err.message : t('autopilot.wizard.runFailed'));
    }
  };

  const handleTogglePause = async (ap: Autopilot) => {
    if (ap.status === 'paused') await resumeAutopilot.mutateAsync(ap._id);
    else await pauseAutopilot.mutateAsync(ap._id);
  };

  const handleDelete = async (id: string) => {
    await removeAutopilot.mutateAsync(id);
  };

  return {
    runStates,
    stepLogs,
    error,
    setError,
    handleRun,
    handleTogglePause,
    handleDelete,
  };
}
