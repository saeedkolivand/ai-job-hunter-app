/**
 * useAutopilotRun — resolved `{ error }` payload handling.
 *
 * The backend `autopilot_run` command RESOLVES (does not reject) with an
 * `{ error }` payload on a scrape failure or unknown id. This exercises the
 * real hook to verify that such a resolution routes to the error state — the
 * SAME state the reject path uses — rather than being treated as 'done'.
 */
import { describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { renderHookWithClient } from '@/test-support';

// ---------------------------------------------------------------------------
// Stubs — declared before the module under test is imported.
// ---------------------------------------------------------------------------

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k, i18n: { language: 'en' } }),
}));

const runMutateAsync = vi.fn();

vi.mock('@/services', async (importActual) => {
  const actual = await importActual<Record<string, unknown>>();
  return {
    ...actual,
    useRunAutopilot: () => ({ mutateAsync: runMutateAsync }),
    usePauseAutopilot: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
    useResumeAutopilot: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
    useRemoveAutopilot: () => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) }),
    useAutopilotStepEvents: () => {},
  };
});

// Import under test AFTER mocks.
import { useAutopilotRun } from './useAutopilotRun';

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
