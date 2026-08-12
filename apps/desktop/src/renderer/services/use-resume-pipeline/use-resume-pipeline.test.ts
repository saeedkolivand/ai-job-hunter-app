/**
 * The staged-run service hooks — and, above all, the ONE decision the record
 * poll makes.
 *
 * A staged run has no reliable terminal EVENT (a stop at a stage boundary emits
 * nothing), so the record poll is the only thing that ever notices a run ended.
 * Everything below exists because a poll that quietly switches itself off is
 * indistinguishable, from the UI, from a run that never finishes.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import type { PipelineRunDetail } from '@ajh/shared/ipc';

import type { AppClient } from '@/lib/app-client';
import { createMockClient, renderHookWithClient } from '@/test-support';

import {
  usePipelineRun,
  usePipelineRunsForJob,
  useRefreshRunsForJobOnTerminal,
  useResolveFabrication,
} from './use-resume-pipeline';

const RUN_ID = 'run-1';
/** Must match `LIVE_RUN_POLL_MS`. */
const POLL_MS = 4_000;

function detail(status: PipelineRunDetail['status']): PipelineRunDetail {
  return {
    runId: RUN_ID,
    jobUrl: 'https://example.test/job',
    kind: 'resume',
    depth: 'quality',
    status,
    startedAt: 1,
    metrics: {},
    events: [],
    report: null,
    resumeText: 'final document',
  };
}

function clientWith(get: (...args: never[]) => unknown): AppClient {
  return createMockClient({ 'resumePipeline.get': get });
}

/** Settle promises and advance the poll clock inside one act(). */
async function tick(ms = 0) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

describe('usePipelineRun — the live record poll', () => {
  it('polls while the record says running', async () => {
    const get = vi.fn().mockResolvedValue(detail('running'));
    renderHookWithClient(() => usePipelineRun(RUN_ID, true), { client: clientWith(get) });

    await tick();
    expect(get).toHaveBeenCalledTimes(1);
    await tick(POLL_MS);
    expect(get).toHaveBeenCalledTimes(2);
  });

  it('stops on its own once a fetched record is terminal', async () => {
    const get = vi.fn().mockResolvedValue(detail('completed'));
    renderHookWithClient(() => usePipelineRun(RUN_ID, true), { client: clientWith(get) });

    await tick();
    expect(get).toHaveBeenCalledTimes(1);
    // Three intervals' worth: a caller that forgets to flip `live` must not
    // keep the app talking to the backend forever.
    await tick(POLL_MS * 3);
    expect(get).toHaveBeenCalledTimes(1);
  });

  it('does not poll at all when the caller is not live', async () => {
    const get = vi.fn().mockResolvedValue(detail('running'));
    renderHookWithClient(() => usePipelineRun(RUN_ID, false), { client: clientWith(get) });

    await tick();
    await tick(POLL_MS * 3);
    expect(get).toHaveBeenCalledTimes(1);
  });

  /**
   * The silent-death case. A gate written as `data?.status === 'running'` reads
   * the `undefined` left by a failed FIRST fetch as "not running" and never
   * schedules another request — the run's only completion signal is then never
   * fetched, and the caller waits forever.
   */
  it('KEEPS polling after the very first fetch fails, and recovers', async () => {
    const get = vi
      .fn()
      .mockRejectedValueOnce(new Error('ipc down'))
      .mockResolvedValue(detail('completed'));
    const { result } = renderHookWithClient(() => usePipelineRun(RUN_ID, true), {
      client: clientWith(get),
    });

    await tick();
    expect(result.current.isError).toBe(true);
    expect(result.current.data).toBeUndefined();
    expect(get).toHaveBeenCalledTimes(1);

    await tick(POLL_MS);
    expect(get).toHaveBeenCalledTimes(2);
    // One more turn for the recovered record to land in the observer.
    await tick(1);
    expect(result.current.data?.status).toBe('completed');
  });

  it('surfaces the failure to the caller instead of swallowing it', async () => {
    const get = vi.fn().mockRejectedValue(new Error('ipc down'));
    const { result } = renderHookWithClient(() => usePipelineRun(RUN_ID, true), {
      client: clientWith(get),
    });

    await tick();
    expect(result.current.isError).toBe(true);
    expect(result.current.error?.message).toBe('ipc down');
  });

  it('keeps polling when a LATER fetch fails — one dropped read is not the end', async () => {
    const get = vi
      .fn()
      .mockResolvedValueOnce(detail('running'))
      .mockRejectedValueOnce(new Error('blip'))
      .mockResolvedValue(detail('running'));
    const { result } = renderHookWithClient(() => usePipelineRun(RUN_ID, true), {
      client: clientWith(get),
    });

    await tick();
    await tick(POLL_MS);
    expect(get).toHaveBeenCalledTimes(2);
    // The last GOOD record is retained, which is what keeps the gate open.
    expect(result.current.data?.status).toBe('running');
    await tick(POLL_MS);
    expect(get).toHaveBeenCalledTimes(3);
  });

  it('never fetches without a run id', async () => {
    const get = vi.fn().mockResolvedValue(detail('running'));
    renderHookWithClient(() => usePipelineRun(null, true), { client: clientWith(get) });

    await tick();
    await tick(POLL_MS * 2);
    expect(get).not.toHaveBeenCalled();
  });
});

/**
 * The invalidation nobody clicks for.
 *
 * `runsForJob` has no `refetchInterval`, and the app's global
 * `refetchOnWindowFocus`/`refetchOnMount` are off — so every transition it
 * renders has to be invalidated by whoever caused it. Three invalidators exist
 * and all three hang off a user action (start, regenerate, resolve). A run
 * ENDING is not one: it is discovered by the record poll, seconds later, with
 * no mutation in sight. Without this hook the list shows a finished run as
 * "Running" until something unrelated happens to refetch it.
 */
describe('useRefreshRunsForJobOnTerminal', () => {
  const JOB_URL = 'https://example.test/job';

  /** The list query and the invalidator together, so the assertion is on a real
   *  refetch rather than on a spy over the query client. */
  function renderBoth(status: PipelineRunDetail['status'] | null) {
    const listForJob = vi.fn().mockResolvedValue([]);
    const view = renderHookWithClient(
      ({ st }: { st: PipelineRunDetail['status'] | null }) => {
        usePipelineRunsForJob(JOB_URL);
        useRefreshRunsForJobOnTerminal(JOB_URL, st);
      },
      {
        client: createMockClient({ 'resumePipeline.listForJob': listForJob }),
        initialProps: { st: status },
      }
    );
    return { ...view, listForJob };
  }

  it.each<PipelineRunDetail['status']>(['completed', 'needsReview', 'failed', 'cancelled'])(
    'refetches the posting run list when the run reaches %s',
    async (terminal) => {
      const { rerender, listForJob } = renderBoth('running');
      await tick();
      expect(listForJob).toHaveBeenCalledTimes(1);

      rerender({ st: terminal });
      await tick();
      expect(listForJob).toHaveBeenCalledTimes(2);
    }
  );

  // The list would otherwise be refetched on every poll tick of a long run —
  // up to 75 minutes of them.
  it('does NOT refetch while the run is still running', async () => {
    const { rerender, listForJob } = renderBoth('running');
    await tick();
    expect(listForJob).toHaveBeenCalledTimes(1);

    rerender({ st: 'running' });
    await tick(POLL_MS * 3);
    expect(listForJob).toHaveBeenCalledTimes(1);
  });

  // `needsReview` → `completed`, when the last verdict lands: a second terminal
  // transition, and the list's chip is wrong until it is refetched too.
  it('refreshes again on a terminal-to-terminal transition', async () => {
    // Mounting AT a terminal status costs no extra fetch: the invalidation
    // lands while the list's first request is still in flight.
    const { rerender, listForJob } = renderBoth('needsReview');
    await tick();
    expect(listForJob).toHaveBeenCalledTimes(1);

    rerender({ st: 'completed' });
    await tick();
    expect(listForJob).toHaveBeenCalledTimes(2);
  });

  it('does nothing without a posting url or a status', async () => {
    const listForJob = vi.fn().mockResolvedValue([]);
    renderHookWithClient(() => useRefreshRunsForJobOnTerminal(null, 'completed'), {
      client: createMockClient({ 'resumePipeline.listForJob': listForJob }),
    });
    await tick();
    expect(listForJob).not.toHaveBeenCalled();
  });
});

describe('useResolveFabrication', () => {
  it('writes the returned detail straight into the run cache', async () => {
    const resolved = detail('needsReview');
    const resolveFabrication = vi.fn().mockResolvedValue(resolved);
    const { result, queryClient } = renderHookWithClient(() => useResolveFabrication(), {
      client: createMockClient({ 'resumePipeline.resolveFabrication': resolveFabrication }),
    });

    await act(async () => {
      await result.current.mutateAsync({
        runId: RUN_ID,
        issueKey: 'factual.unsourced_metric#0',
        decision: 'remove',
      });
    });

    expect(queryClient.getQueryData(['pipeline', 'run', RUN_ID])).toBe(resolved);
  });
});
