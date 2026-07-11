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
    // Clear any stale failure banner from a prior run before this one resolves
    // — otherwise a successful run after a failed one leaves the old error up.
    setError(null);
    try {
      // `autopilot_run` RESOLVES (not rejects) even when the run failed, so a
      // resolved value is NOT proof of success — inspect it before reporting
      // 'done'. Two failure shapes both route to the SAME error state + banner
      // the reject path below uses:
      //   • `{ error }`        — a scrape failure or unknown id (early Rust return).
      //   • `status:'failed'`  — the run reached the record but zero boards
      //     succeeded (the "success theater" case: found:0, no `error` key).
      const result = await runAutopilot.mutateAsync(id);
      // Concurrent-run guard (PR H): a second invocation while one is already
      // in flight (another manual click, or a scheduler tick racing this one)
      // resolves with `{ skipped: "already-running" }` instead of running.
      // Nothing happened from THIS call, so it's neither a failure (no red
      // 'error' state) nor a success ('done' would misreport a run that never
      // occurred) — revert the optimistic 'scraping' state back to idle and
      // surface a distinct, honest message via the same banner `error` uses.
      if (result.skipped === 'already-running') {
        setRunStates({ [id]: 'idle' });
        setError(t('autopilot.wizard.alreadyRunning'));
        return;
      }
      if (result.error || result.status === 'failed') {
        setRunStates({ [id]: 'error' });
        setError(result.error ?? t('autopilot.wizard.allBoardsFailed'));
        return;
      }
      // `completedWithErrors` (some boards failed, others returned jobs) and any
      // unrecognized/future status stay a 'done' run — the durable partial-failure
      // signal is the persisted per-run badge on the card (`ap.runStatus`),
      // refreshed by this mutation's autopilot-list invalidation.
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
