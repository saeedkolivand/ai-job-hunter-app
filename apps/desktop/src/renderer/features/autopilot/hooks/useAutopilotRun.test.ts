/**
 * useAutopilotRun — resolved `{ error }` payload handling.
 *
 * The backend `autopilot_run` command RESOLVES (does not reject) with an
 * `{ error }` payload on a scrape failure or unknown id. This exercises the
 * real hook to verify that such a resolution routes to the error state — the
 * SAME state the reject path uses — rather than being treated as 'done'.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { useSessionStore } from '@/store/session-store';
import { renderHookWithClient } from '@/test-support';

// ---------------------------------------------------------------------------
// Stubs — declared before the module under test is imported.
// ---------------------------------------------------------------------------

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k, i18n: { language: 'en' } }),
}));

const runMutateAsync = vi.fn();

/** Stable across renders — the hook lists it in `handleStep`'s dep array, so an
 *  identity that changed every render would silently re-subscribe each time. */
const invalidateAutopilots = vi.hoisted(() => vi.fn());

/** Captures the step handler the hook registers, so a test can deliver step
 *  events the way the Tauri event bridge does. */
const stepEvents = vi.hoisted(() => ({
  handler: null as
    ((event: { jobId: string; autopilotId: string; step: string; detail: string }) => void) | null,
}));

vi.mock('@/services', async (importActual) => {
  const actual = await importActual<Record<string, unknown>>();
  return {
    ...actual,
    useRunAutopilot: () => ({ mutateAsync: runMutateAsync }),
    usePauseAutopilot: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
    useResumeAutopilot: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
    useRemoveAutopilot: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
    useInvalidateAutopilots: () => invalidateAutopilots,
    useAutopilotStepEvents: (cb: (event: never) => void) => {
      stepEvents.handler = cb as unknown as typeof stepEvents.handler;
    },
  };
});

// Import under test AFTER mocks.
import { useAutopilotRun } from './useAutopilotRun';

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// The error banner now lives in the (real, unmocked) session store instead of
// a local `useState`, so it survives across a test unless reset — mirrors
// `AutopilotPage.focus.test.tsx`'s own store reset.
beforeEach(() => {
  useSessionStore.setState((s) => ({ autopilot: { ...s.autopilot, error: null } }));
});

describe('useAutopilotRun — handleRun error-payload handling', () => {
  it('routes a resolved { error } payload to the error state, not done', async () => {
    runMutateAsync.mockResolvedValueOnce({ error: 'scrape failed: 429', jobId: 'j1' });
    const { result } = renderHookWithClient(() => useAutopilotRun());

    await act(async () => {
      await result.current.handleRun('ap1');
    });

    expect(result.current.runStates.ap1).toBe('error');
    expect(result.current.error).toBe('scrape failed: 429');
  });

  it('routes a resolved success payload to the done state', async () => {
    runMutateAsync.mockResolvedValueOnce({ jobId: 'j2', found: 3, applied: 0 });
    const { result } = renderHookWithClient(() => useAutopilotRun());

    await act(async () => {
      await result.current.handleRun('ap2');
    });

    expect(result.current.runStates.ap2).toBe('done');
    expect(result.current.error).toBeNull();
  });

  it('routes a status:"failed" payload (no error key) to the error state, not done', async () => {
    // The "success theater" case: the run reached the record but every board
    // failed — found:0, no `error` key. Must still surface as an error.
    runMutateAsync.mockResolvedValueOnce({ jobId: 'j3', status: 'failed', found: 0 });
    const { result } = renderHookWithClient(() => useAutopilotRun());

    await act(async () => {
      await result.current.handleRun('ap3');
    });

    expect(result.current.runStates.ap3).toBe('error');
    // `t` is stubbed to echo the key, so the banner carries the honest message.
    expect(result.current.error).toBe('autopilot.wizard.allBoardsFailed');
  });

  it('keeps a status:"completedWithErrors" run in the done state (partial success)', async () => {
    // Some boards failed but others returned jobs — the run still produced
    // results, so it stays 'done'; the persisted card badge carries the warning.
    runMutateAsync.mockResolvedValueOnce({ jobId: 'j4', status: 'completedWithErrors', found: 2 });
    const { result } = renderHookWithClient(() => useAutopilotRun());

    await act(async () => {
      await result.current.handleRun('ap4');
    });

    expect(result.current.runStates.ap4).toBe('done');
    expect(result.current.error).toBeNull();
  });

  it('prefers an { error } over a completedWithErrors status when both are present', async () => {
    // Unreachable today by construction (the backend never emits both
    // together), but pins the intended precedence — `error` wins over
    // `status` — so a future refactor of either side can't silently swap
    // this run into the 'done' state.
    runMutateAsync.mockResolvedValueOnce({
      jobId: 'j6',
      error: 'x',
      status: 'completedWithErrors',
    });
    const { result } = renderHookWithClient(() => useAutopilotRun());

    await act(async () => {
      await result.current.handleRun('ap6');
    });

    expect(result.current.runStates.ap6).toBe('error');
    expect(result.current.error).toBe('x');
  });

  it('falls back to done for an unrecognized future status', async () => {
    runMutateAsync.mockResolvedValueOnce({ jobId: 'j5', status: 'someFutureStatus', found: 1 });
    const { result } = renderHookWithClient(() => useAutopilotRun());

    await act(async () => {
      await result.current.handleRun('ap5');
    });

    expect(result.current.runStates.ap5).toBe('done');
    expect(result.current.error).toBeNull();
  });

  it('routes a resolved { skipped: "already-running" } payload to idle (not done, not error) with a distinct message', async () => {
    runMutateAsync.mockResolvedValueOnce({ skipped: 'already-running' });
    const { result } = renderHookWithClient(() => useAutopilotRun());

    await act(async () => {
      await result.current.handleRun('ap-concurrent');
    });

    // Not the silent-success 'done' state — no run actually happened.
    //
    // 'idle' specifically, and not the 'scraping' the refusal would seem to
    // justify: 'idle' is the value that hands the card back to the backend's
    // persisted `runStatus` (see AutopilotPage), so it still displays as
    // running while the concurrent run lasts AND clears when that run ends.
    // Pinning 'scraping' here would strand the card forever whenever the
    // concurrent run FAILS, because the Rust failure path emits no terminal
    // step for anything to observe.
    expect(result.current.runStates['ap-concurrent']).toBe('idle');
    // Not the red 'error' banner either — a distinct, honest message.
    expect(result.current.error).toBe('autopilot.wizard.alreadyRunning');
  });

  it('clears a stale failure banner once a subsequent run succeeds', async () => {
    runMutateAsync.mockResolvedValueOnce({ jobId: 'j7', status: 'failed', found: 0 });
    const { result } = renderHookWithClient(() => useAutopilotRun());

    await act(async () => {
      await result.current.handleRun('ap7');
    });
    expect(result.current.error).toBe('autopilot.wizard.allBoardsFailed');

    runMutateAsync.mockResolvedValueOnce({ jobId: 'j8', found: 5, applied: 0 });
    await act(async () => {
      await result.current.handleRun('ap7');
    });

    expect(result.current.runStates.ap7).toBe('done');
    expect(result.current.error).toBeNull();
  });
});

describe('useAutopilotRun — batched step events', () => {
  it('keeps both step-log entries when two events for one autopilot land in the same batch', () => {
    const { result } = renderHookWithClient(() => useAutopilotRun());
    const deliver = stepEvents.handler;
    expect(deliver, 'the hook must register a step handler').toBeTruthy();

    // Two events for the SAME autopilotId inside ONE act() — i.e. one React
    // batch, with no re-render in between. Patches computed from the render
    // snapshot both start from the same base, so the second overwrites the
    // first and the scrape_start line is lost.
    act(() => {
      deliver?.({ jobId: 'j1', autopilotId: 'ap-batch', step: 'scrape_start', detail: 'boards' });
      deliver?.({ jobId: 'j1', autopilotId: 'ap-batch', step: 'scrape_done', detail: '12 jobs' });
    });

    const steps = (result.current.stepLogs['ap-batch'] ?? []).map((entry) => entry.step);
    expect(steps).toEqual(['scrape_start', 'scrape_done']);
    // The machine also advanced through BOTH transitions, not just the last.
    expect(result.current.runStates['ap-batch']).toBe('ranking');
  });
});

/**
 * A run the CURRENT mount never started. `runStates` is component-local, so
 * navigating away and back — or a scheduled run nobody clicked — delivers steps
 * with no local state and no preceding `scrape_start`.
 */
describe('useAutopilotRun — steps for a run this mount did not start', () => {
  it('advances an unknown autopilot straight from the mid-run step it first sees', () => {
    const { result } = renderHookWithClient(() => useAutopilotRun());
    const deliver = stepEvents.handler;

    // No handleRun, no scrape_start — the first thing this mount ever hears
    // about `ap-remount` is that scraping already finished.
    act(() => {
      deliver?.({ jobId: 'j9', autopilotId: 'ap-remount', step: 'scrape_done', detail: '8 jobs' });
    });
    expect(result.current.runStates['ap-remount']).toBe('ranking');

    act(() => {
      deliver?.({ jobId: 'j9', autopilotId: 'ap-remount', step: 'complete', detail: 'Found 8' });
    });
    expect(result.current.runStates['ap-remount']).toBe('done');
  });

  it('refreshes the autopilot list when a run ends, but not on every step', () => {
    invalidateAutopilots.mockClear();
    const { result } = renderHookWithClient(() => useAutopilotRun());
    const deliver = stepEvents.handler;
    expect(result.current, 'the hook must render').toBeTruthy();

    // Mid-run chatter must not invalidate — the record has not changed yet, and
    // a refetch per step would refetch several times a run for nothing.
    act(() => {
      deliver?.({ jobId: 'j10', autopilotId: 'ap-inv', step: 'scrape_start', detail: 'boards' });
      deliver?.({ jobId: 'j10', autopilotId: 'ap-inv', step: 'rank_done', detail: 'ranked' });
    });
    expect(invalidateAutopilots).not.toHaveBeenCalled();

    // `complete` is the moment the persisted record gains this run's found jobs
    // and `runStatus`. Without this the card keeps the pre-run data until the
    // next navigation — the whole reason a finished run looked unfinished.
    act(() => {
      deliver?.({ jobId: 'j10', autopilotId: 'ap-inv', step: 'complete', detail: 'Found 8' });
    });
    expect(invalidateAutopilots).toHaveBeenCalledTimes(1);
  });

  it('refreshes the list on a cancelled run too, not only a completed one', () => {
    // `cancelled` is the OTHER terminal step, and it changes the record just as
    // much: `finish_cancelled` clears the live status and wipes the previous
    // run's summaries, so a card left on pre-cancel data is showing a run that
    // was abandoned.
    invalidateAutopilots.mockClear();
    const { result } = renderHookWithClient(() => useAutopilotRun());
    const deliver = stepEvents.handler;
    expect(result.current, 'the hook must render').toBeTruthy();

    act(() => {
      deliver?.({ jobId: 'j11', autopilotId: 'ap-cancel', step: 'cancelled', detail: 'stopped' });
    });

    expect(invalidateAutopilots).toHaveBeenCalledTimes(1);
    expect(result.current.runStates['ap-cancel']).toBe('cancelled');
  });
});

/**
 * The refusal case has no OTHER trace at all: unlike a genuine failure (which
 * at least leaves `runStatus: 'failed'` on the persisted record), "a run is
 * already in progress" is never written anywhere backend-side — the banner
 * IS the only record of it, so it must outlive an unmount on its own.
 */
describe('useAutopilotRun — the error banner survives navigation', () => {
  it('keeps the "already running" refusal visible after this mount is discarded and a fresh one takes its place', async () => {
    runMutateAsync.mockResolvedValueOnce({ skipped: 'already-running' });
    const { result, unmount } = renderHookWithClient(() => useAutopilotRun());

    await act(async () => {
      await result.current.handleRun('ap-nav');
    });
    expect(result.current.error).toBe('autopilot.wizard.alreadyRunning');

    // Simulates navigating away from AutopilotPage (this hook — reducers
    // included — is discarded) and back (a brand-new call, exactly like a
    // fresh route mount).
    unmount();
    const remounted = renderHookWithClient(() => useAutopilotRun());

    expect(remounted.result.current.error).toBe('autopilot.wizard.alreadyRunning');
  });
});
