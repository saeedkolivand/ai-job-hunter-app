/**
 * useModelPull — re-attach-on-mount coverage.
 *
 * `pullJobId` used to live ONLY in this hook's own `useState`, so any unmount
 * of the panel (Cloud/CLI tab switch, Back/Forward through the wizard, or just
 * navigating away and back) lost it forever: no later `job.stream` /
 * `job.completed` for the still-running pull could ever match again, and the
 * panel re-showed the Download button for work that was actually in flight.
 *
 * This pins the fix: on mount, the hook reads the backend job registry
 * (`jobs_list`) for an already-running `ai.pull_model` job and re-attaches to
 * it, so a LATER event for that same job id is not silently dropped.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import { createMockClient, withProviders } from '@/test-support';

import { useModelPull } from './useModelPull';

vi.mock('@ajh/translations', () => ({ useTranslation: () => ({ t: (k: string) => k }) }));

const notifyApi = { success: vi.fn(), error: vi.fn() };
vi.mock('@ajh/ui', () => ({ useNotification: () => notifyApi }));

afterEach(() => vi.restoreAllMocks());

function makeJob(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    id: 'job-123',
    kind: 'ai.pull_model',
    status: 'running',
    progress: 0,
    payload: null,
    retries: 0,
    maxRetries: 3,
    createdAt: Date.now(),
    updatedAt: Date.now(),
    ...overrides,
  };
}

describe('useModelPull — reattach on mount', () => {
  it('adopts an already-running ai.pull_model job from the registry and resumes tracking its events', async () => {
    let handler: ((e: unknown) => void) | null = null;
    const client = createMockClient({
      'jobs.list': vi.fn().mockResolvedValue([makeJob()]),
      // The post-adoption reconcile read: this job is genuinely still
      // running, so it must be a no-op (state stays 'pulling' below).
      'jobs.get': vi.fn().mockResolvedValue(makeJob()),
      'jobs.onEvent': vi.fn((h: (e: unknown) => void) => {
        handler = h;
        return () => {};
      }),
    });

    const { result } = renderHook(() => useModelPull({ selectedModel: 'llama3' }), {
      wrapper: withProviders(client),
    });

    // The registry query resolves asynchronously; once it does, the hook must
    // stop reporting idle for a pull that is actually running.
    await waitFor(() => expect(result.current.pullState).toBe('pulling'));

    // Proves pullJobId itself was recovered (not just the state flag guessed):
    // only an event carrying the SAME job id moves progress.
    act(() => handler?.({ type: 'job.stream', jobId: 'job-123', data: { p: 0.42 } }));
    expect(result.current.pullProgress).toBeCloseTo(42);
  });

  it('ignores an unrelated or terminal ai.pull_model job and stays idle', async () => {
    const client = createMockClient({
      'jobs.list': vi
        .fn()
        .mockResolvedValue([
          makeJob({ status: 'completed' }),
          makeJob({ id: 'j2', kind: 'ai.embed' }),
        ]),
    });

    const { result } = renderHook(() => useModelPull({ selectedModel: 'llama3' }), {
      wrapper: withProviders(client),
    });

    await waitFor(() => expect(client.jobs.list).toHaveBeenCalled());
    // Flush a macrotask so the registry query has fully settled before
    // asserting the ABSENCE of a state change (a bare microtask can race the
    // query's own internal promise chain).
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(result.current.pullState).toBe('idle');
  });
});

// ── Adoption race: a terminal event that arrives before `pullJobId` commits ──
//
// PR #1036 review finding: `useJobEvents` compares an incoming event's jobId
// against `pullJobId`, which is still `null` for the entire IPC round trip
// `jobs.list()` takes to resolve. `ai.pull_model` fires its ONE
// job.completed/job.failed the instant the pull ends, so a terminal event
// landing in that window is dropped for good — and the registry snapshot the
// reattach effect then adopts was taken BEFORE that completion, so it still
// reads 'running'. Without a reconcile read right after adopting, the panel
// is stuck reporting 'pulling' forever for a job that already finished.
describe('useModelPull — reconciles a stale adoption', () => {
  it('settles a job that already completed before the registry read resolved', async () => {
    const jobId = 'job-race-completed';
    let resolveList: (jobs: unknown[]) => void = () => {};
    const listPromise = new Promise<unknown[]>((resolve) => {
      resolveList = resolve;
    });
    let handler: ((e: unknown) => void) | null = null;

    const client = createMockClient({
      'jobs.list': vi.fn().mockReturnValue(listPromise),
      'jobs.get': vi.fn().mockResolvedValue(makeJob({ id: jobId, status: 'completed' })),
      'jobs.onEvent': vi.fn((h: (e: unknown) => void) => {
        handler = h;
        return () => {};
      }),
    });

    const { result } = renderHook(() => useModelPull({ selectedModel: 'llama3' }), {
      wrapper: withProviders(client),
    });

    // Fires the ACTUAL ordering the review flagged: the terminal event
    // arrives while `jobs.list()` is still in flight, so `pullJobId` is
    // still null and the live listener's identity check drops it.
    act(() => handler?.({ type: 'job.completed', jobId, data: { model: 'llama3', done: true } }));

    // The registry read resolves with a snapshot taken BEFORE that
    // completion — still 'running' — so the reattach effect adopts it. The
    // fix's reconcile read then settles it in the same flush, before this
    // test ever observes the intermediate 'pulling' state — asserting the
    // FINAL settled state plus the `jobs.get` call is what actually pins the
    // fix (an unfixed hook would stay 'pulling' forever instead).
    await act(async () => {
      resolveList([makeJob({ id: jobId })]);
      await listPromise;
    });

    // No second job.completed is ever coming for this job — only the
    // reconcile read (`jobs.get`, right after adoption) can notice it
    // already finished.
    await waitFor(() => expect(result.current.pullState).toBe('done'));
    expect(client.jobs.get).toHaveBeenCalledWith(jobId);
    expect(notifyApi.success).toHaveBeenCalled();
  });

  it('settles a job that already failed before the registry read resolved', async () => {
    const jobId = 'job-race-failed';
    let resolveList: (jobs: unknown[]) => void = () => {};
    const listPromise = new Promise<unknown[]>((resolve) => {
      resolveList = resolve;
    });
    let handler: ((e: unknown) => void) | null = null;

    const client = createMockClient({
      'jobs.list': vi.fn().mockReturnValue(listPromise),
      'jobs.get': vi.fn().mockResolvedValue(makeJob({ id: jobId, status: 'failed' })),
      'jobs.onEvent': vi.fn((h: (e: unknown) => void) => {
        handler = h;
        return () => {};
      }),
    });

    const { result } = renderHook(() => useModelPull({ selectedModel: 'llama3' }), {
      wrapper: withProviders(client),
    });

    act(() => handler?.({ type: 'job.failed', jobId, data: 'model not found' }));

    await act(async () => {
      resolveList([makeJob({ id: jobId })]);
      await listPromise;
    });

    await waitFor(() => expect(result.current.pullState).toBe('error'));
    expect(client.jobs.get).toHaveBeenCalledWith(jobId);
    expect(notifyApi.error).toHaveBeenCalled();
  });
});
