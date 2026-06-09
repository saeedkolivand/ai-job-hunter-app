import type { ReactNode } from 'react';
import { describe, expect, it } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { ScoringSchedulerProvider, useScoringScheduler } from './ScoringScheduler';

function wrapper({ children }: { children: ReactNode }) {
  return <ScoringSchedulerProvider>{children}</ScoringSchedulerProvider>;
}

function renderScheduler() {
  return renderHook(() => useScoringScheduler(), { wrapper });
}

describe('ScoringSchedulerProvider', () => {
  it('enqueues and activates first item immediately', () => {
    const { result } = renderScheduler();

    act(() => {
      result.current.enqueue('job-1');
    });

    expect(result.current.activeSet.has('job-1')).toBe(true);
  });

  it('second item waits until first is removed then activates', () => {
    const { result } = renderScheduler();

    act(() => {
      result.current.enqueue('job-1');
      result.current.enqueue('job-2');
    });

    // CONCURRENCY = 1: job-1 fills the slot, job-2 must be pending.
    expect(result.current.activeSet.has('job-1')).toBe(true);
    expect(result.current.activeSet.has('job-2')).toBe(false);

    // remove() clears job-1 from both queueRef and activeSet, then tryAdvance
    // promotes job-2 since a slot is now free.
    act(() => {
      result.current.remove('job-1');
    });

    expect(result.current.activeSet.has('job-1')).toBe(false);
    expect(result.current.activeSet.has('job-2')).toBe(true);
  });

  it('release without prior remove re-promotes the same job (stays in queue)', () => {
    // This tests the invariant: release only touches activeSet, not queueRef.
    // tryAdvance therefore picks the released job back up immediately.
    // In production usage (RowMatchScore) the job is removed via the cleanup
    // effect, so this edge case does not arise in normal flows.
    const { result } = renderScheduler();

    act(() => {
      result.current.enqueue('job-1');
      result.current.enqueue('job-2');
    });

    expect(result.current.activeSet.has('job-1')).toBe(true);

    // release without remove: job-1 is still in queueRef so tryAdvance
    // promotes it right back — job-2 does NOT advance.
    act(() => {
      result.current.release('job-1');
    });

    expect(result.current.activeSet.has('job-1')).toBe(true);
    expect(result.current.activeSet.has('job-2')).toBe(false);
  });

  it('remove dequeues item and activates next pending item', () => {
    const { result } = renderScheduler();

    act(() => {
      result.current.enqueue('job-1');
      result.current.enqueue('job-2');
    });

    expect(result.current.activeSet.has('job-1')).toBe(true);
    expect(result.current.activeSet.has('job-2')).toBe(false);

    // Removing the active item frees the concurrency slot; tryAdvance runs
    // and promotes job-2 from the queue.
    act(() => {
      result.current.remove('job-1');
    });

    expect(result.current.activeSet.has('job-1')).toBe(false);
    expect(result.current.activeSet.has('job-2')).toBe(true);
  });

  it('idempotent enqueue does not add the same job twice', () => {
    const { result } = renderScheduler();

    act(() => {
      result.current.enqueue('job-1');
      result.current.enqueue('job-1'); // duplicate
    });

    // Only one concurrency slot consumed.
    expect(result.current.activeSet.size).toBe(1);
    expect(result.current.activeSet.has('job-1')).toBe(true);
  });
});
