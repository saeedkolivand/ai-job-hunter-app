import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import type { ScrapeProgressEvent } from '@ajh/shared';

import { createMockClient, withProviders } from '@/test-support';

import { useScrapeProgress } from './use-scrape-progress';

afterEach(() => vi.restoreAllMocks());

function setup(initialJobId: string | null) {
  let handler: ((e: ScrapeProgressEvent) => void) | null = null;
  const off = vi.fn();
  const client = createMockClient({
    'scrape.onProgress': vi.fn((h: (e: ScrapeProgressEvent) => void) => {
      handler = h;
      return off;
    }),
  });
  const hook = renderHook((jobId: string | null) => useScrapeProgress(jobId), {
    wrapper: withProviders(client),
    initialProps: initialJobId,
  });
  return {
    hook,
    off,
    emit: (e: ScrapeProgressEvent) => act(() => handler?.(e)),
  };
}

describe('useScrapeProgress', () => {
  it('maps a progress event for the active job id to its fraction', () => {
    const { hook, emit } = setup('job-1');
    expect(hook.result.current).toBeNull();
    emit({ jobId: 'job-1', progress: 0.5 });
    expect(hook.result.current).toBe(0.5);
  });

  it('ignores events for other/stale job ids', () => {
    const { hook, emit } = setup('job-1');
    emit({ jobId: 'job-1', progress: 0.25 });
    expect(hook.result.current).toBe(0.25);
    emit({ jobId: 'stale-job', progress: 0.9 });
    expect(hook.result.current).toBe(0.25);
  });

  it('resets when a new scrape starts and when it completes', () => {
    const { hook, emit } = setup('job-1');
    emit({ jobId: 'job-1', progress: 0.75 });
    expect(hook.result.current).toBe(0.75);

    // New scrape → progress resets, only the new id is honored.
    hook.rerender('job-2');
    expect(hook.result.current).toBeNull();
    emit({ jobId: 'job-1', progress: 1 }); // late event from the old scrape
    expect(hook.result.current).toBeNull();
    emit({ jobId: 'job-2', progress: 0.33 });
    expect(hook.result.current).toBe(0.33);

    // Completion (id → null) resets.
    hook.rerender(null);
    expect(hook.result.current).toBeNull();
  });

  it('unsubscribes on unmount', () => {
    const { hook, off } = setup('job-1');
    hook.unmount();
    expect(off).toHaveBeenCalledTimes(1);
  });
});
