import { useCallback, useReducer } from 'react';

import type { Autopilot, AutopilotStepEvent } from '@ajh/shared';
import { useTranslation } from '@ajh/translations';

import type { RunStateMap, StepLogMap } from '@/features/autopilot/constants';
import { transition as machineTransition } from '@/lib/machine';
import { autopilotRunMachine, stepToEvent } from '@/lib/machines/autopilot-run.machine';
import {
  useAutopilotStepEvents,
  useInvalidateAutopilots,
  usePauseAutopilot,
  useRemoveAutopilot,
  useResumeAutopilot,
  useRunAutopilot,
} from '@/services';
import { useSessionStore } from '@/store/session-store';

/**
 * A reducer patch: the fields to merge, or a function of the CURRENT state that
 * produces them.
 *
 * The functional form is required whenever the new value DERIVES from the old
 * one. Computing it from a render snapshot instead means two events for the same
 * key inside one React batch both read the same base, so the second patch
 * overwrites the first.
 */
type Patch<T> = Partial<T> | ((prev: T) => Partial<T>);

export function useAutopilotRun() {
  const { t } = useTranslation();
  const [runStates, setRunStates] = useReducer(
    (prev: RunStateMap, patch: Patch<RunStateMap>): RunStateMap =>
      ({ ...prev, ...(typeof patch === 'function' ? patch(prev) : patch) }) as RunStateMap,
    {} as RunStateMap
  );
  const [stepLogs, setStepLogs] = useReducer(
    (prev: StepLogMap, patch: Patch<StepLogMap>): StepLogMap =>
      ({ ...prev, ...(typeof patch === 'function' ? patch(prev) : patch) }) as StepLogMap,
    {} as StepLogMap
  );
  // The Run/Apply banner survives navigation via the session store rather than
  // a local `useState` — this hook (and its subscription) is re-created from
  // scratch on every `AutopilotPage` mount, so a local `useState` discarded
  // the banner on any navigate-away-and-back. See `AutopilotSlice.error`'s
  // doc comment for why that matters most for the concurrent-run refusal,
  // which leaves no OTHER trace at all.
  const error = useSessionStore((s) => s.autopilot.error);
  const setAutopilot = useSessionStore((s) => s.setAutopilot);
  const setError = useCallback(
    (message: string | null) => setAutopilot({ error: message }),
    [setAutopilot]
  );

  const runAutopilot = useRunAutopilot();
  const pauseAutopilot = usePauseAutopilot();
  const resumeAutopilot = useResumeAutopilot();
  const removeAutopilot = useRemoveAutopilot();
  const invalidateAutopilots = useInvalidateAutopilots();

  const handleStep = useCallback(
    (event: AutopilotStepEvent) => {
      // Both patches DERIVE from the current value, so they must read the
      // reducer's `prev` — not the render snapshot this callback closed over.
      // Two step events for the same autopilotId inside one React batch would
      // otherwise both start from the same base, and the second would overwrite
      // the first: a dropped step-log line and a skipped state transition.
      const ev = stepToEvent(event.step);
      if (ev) {
        setRunStates((prev) => ({
          [event.autopilotId]: machineTransition(
            autopilotRunMachine,
            prev[event.autopilotId] ?? 'idle',
            ev
          ),
        }));
      }
      // Read the clock at dispatch time, not inside the reducer body: under
      // StrictMode dev double-invocation the reducer runs twice from the same base,
      // and `Date.now()` there would produce two different timestamps (impure).
      const ts = Date.now();
      setStepLogs((prev) => ({
        [event.autopilotId]: [
          ...(prev[event.autopilotId] ?? []).slice(-49),
          { step: event.step, detail: event.detail, ts },
        ],
      }));
      // A run that ENDS is the only moment the persisted record changes, and the
      // `runAutopilot` mutation's own invalidation only fires for a run this
      // mount started and stayed mounted for. Refresh here so a SCHEDULED run
      // nobody clicked — the other case with no mutation watching it — lands
      // its found jobs and its `runStatus` on the card right away instead of
      // waiting for something unrelated to refetch.
      //
      // Does NOT cover a run that finished while the user was on another
      // page: `useAutopilotStepEvents` unsubscribes in this hook's own effect
      // cleanup, so no step event reaches this handler while unmounted,
      // scheduled run or not. What actually recovers THAT case is
      // `useAutopilots`'s own staleness-triggered refetch on the next
      // `AutopilotPage` mount (see its doc comment).
      if (event.step === 'complete' || event.step === 'cancelled') invalidateAutopilots();
    },
    [invalidateAutopilots]
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
      // occurred) — revert the optimistic 'scraping' and surface a distinct,
      // honest message via the same banner `error` uses.
      //
      // Reverting to 'idle' is safe HERE only because 'idle' is what hands the
      // card back to the backend's `runStatus` (see AutopilotPage). The refusal
      // proves a run is in flight, so the card still shows one — but it now
      // does so on evidence that keeps being re-read, rather than on a local
      // guess. Pinning 'scraping' here instead would strand the card forever
      // when that concurrent run FAILS: the Rust failure path returns early
      // without emitting any step, so no terminal event ever arrives to clear
      // it. Only `runStatus` records that outcome.
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
