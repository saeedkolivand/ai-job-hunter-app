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

import { createMockClient, makeQueryClient, withProviders } from '@/test-support';

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

    // Prove the listener is actually wired before relying on firing it — a
    // `handler?.(…)` on a `null` handler is a silent no-op, and both this and
    // the reconcile assertions below would still pass even if the live
    // subscription were never registered at all (findings the reconcile read
    // is supposed to be tested WITH a real event in flight, not without one).
    await waitFor(() => expect(client.jobs.onEvent).toHaveBeenCalled());
    expect(handler).not.toBeNull();

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
    // already finished. Exact counts (not just "was called"): `jobQueue`'s
    // cached snapshot never refetches inside this test, so an unfixed hook
    // (PR #1036 review finding) re-adopts the same stale 'running' entry the
    // instant `resetTracking` clears `pullJobId`, calling `jobs.get` and the
    // success toast a SECOND time — these pin that regression shut.
    await waitFor(() => expect(result.current.pullState).toBe('done'));
    expect(client.jobs.get).toHaveBeenCalledTimes(1);
    expect(client.jobs.get).toHaveBeenCalledWith(jobId);
    expect(notifyApi.success).toHaveBeenCalledTimes(1);
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

    await waitFor(() => expect(client.jobs.onEvent).toHaveBeenCalled());
    expect(handler).not.toBeNull();

    act(() => handler?.({ type: 'job.failed', jobId, data: 'model not found' }));

    await act(async () => {
      resolveList([makeJob({ id: jobId })]);
      await listPromise;
    });

    await waitFor(() => expect(result.current.pullState).toBe('error'));
    expect(client.jobs.get).toHaveBeenCalledTimes(1);
    expect(client.jobs.get).toHaveBeenCalledWith(jobId);
    expect(notifyApi.error).toHaveBeenCalledTimes(1);
  });
});

// ── mountedRef must survive React.StrictMode's setup→cleanup→setup ──────────
//
// PR #1036 review finding (MINOR): the production app renders inside
// React.StrictMode (main.tsx). StrictMode double-invokes an effect in
// development — setup, cleanup, setup — while preserving hook state between
// the two setups. `mountedRef` used to be set `true` only by the initial
// `useRef(true)` and back to `false` by the cleanup, with nothing restoring
// it on the SECOND setup — so after StrictMode's dance it is permanently
// `false`, and the reconcile read's `if (!mountedRef.current …) return;`
// guard drops every settle for the rest of this hook instance's life.
describe('useModelPull — survives React.StrictMode remount', () => {
  it('still settles a job that completed, after StrictMode setup→cleanup→setup', async () => {
    const jobId = 'job-strictmode';
    const client = createMockClient({
      'jobs.list': vi.fn().mockResolvedValue([makeJob({ id: jobId })]),
      'jobs.get': vi.fn().mockResolvedValue(makeJob({ id: jobId, status: 'completed' })),
      'jobs.onEvent': vi.fn(() => () => {}),
    });
    const queryClient = makeQueryClient();

    // `reactStrictMode: true` is testing-library's OWN option for this —
    // NOT rendering a `<React.StrictMode>` element through a custom
    // `wrapper` component, which looks equivalent but is NOT: testing-library
    // wraps StrictMode around `wrapper` only when told to via this option
    // (`strictModeIfNeeded` in its source), so a `<StrictMode>` nested a
    // level deeper inside a hand-rolled wrapper never gets the double
    // setup→cleanup→setup pass — confirmed empirically (a throwaway probe
    // effect counted setups=1/cleanups=0 with the manual-wrapper form and
    // setups=2/cleanups=1 with this option) before trusting this test to
    // pin the fix. Wrap ONLY this test — adding it to `withProviders`
    // itself would skew every other suite's call counts.
    const { result } = renderHook(() => useModelPull({ selectedModel: 'llama3' }), {
      wrapper: withProviders(client, queryClient),
      reactStrictMode: true,
    });

    // An unfixed hook gets stuck on 'pulling' forever here instead — the
    // reconcile read's own guard silently drops the settle.
    await waitFor(() => expect(result.current.pullState).toBe('done'));
  });
});
