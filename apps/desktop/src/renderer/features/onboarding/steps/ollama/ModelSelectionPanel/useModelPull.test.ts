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
