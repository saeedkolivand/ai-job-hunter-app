/**
 * JobsPage — leading-edge throttle on job.stream (streamInvalidateTimerRef):
 *   - N rapid ticks within 1 s → invalidatePostings called once.
 *   - A tick after the 1 s interval elapses → called again.
 *   - Unmount before the pending timer fires → no extra call after unmount.
 *
 * Fake timers cover the throttle assertions; real timers are restored after
 * each test.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act } from '@testing-library/react';

import { makePosting } from './fixtures';
import { fireStreamEvent, invalidateSpy, renderJobsPage, resetStream } from './stream-harness';

const advance = (ms: number) =>
  act(async () => {
    vi.advanceTimersByTime(ms);
  });

describe('JobsPage — stream invalidation throttle', () => {
  beforeEach(() => {
    resetStream();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('N rapid stream ticks within 1 s → invalidatePostings called exactly once (leading edge)', async () => {
    const { unmount } = renderJobsPage();

    // Fire 5 rapid ticks — all within the 1 s window.
    for (let i = 0; i < 5; i++) {
      fireStreamEvent(makePosting(`item-${i}`));
    }

    // The first tick sets the timer; subsequent ticks within the window do NOT
    // set a new one. The invalidate fires AFTER the timeout, inside the
    // setTimeout callback.
    await advance(999);
    // Still within the window — timer not fired yet.
    expect(invalidateSpy).not.toHaveBeenCalled();

    // Advance past 1 s — timer fires once.
    await advance(2);
    expect(invalidateSpy).toHaveBeenCalledTimes(1);

    unmount();
  });

  it('a tick after the 1 s interval elapses triggers a second invalidation', async () => {
    const { unmount } = renderJobsPage();

    // First tick — starts the timer.
    fireStreamEvent(makePosting('tick-1'));

    // Advance past 1 s — timer fires, ref reset to null.
    await advance(1001);
    expect(invalidateSpy).toHaveBeenCalledTimes(1);

    // Second tick — a new timer is started (ref is null again).
    fireStreamEvent(makePosting('tick-2'));

    await advance(1001);
    expect(invalidateSpy).toHaveBeenCalledTimes(2);

    unmount();
  });

  it('unmount before the pending timer fires → no invalidation after unmount', async () => {
    const { unmount } = renderJobsPage();

    // Fire a tick — starts the 1 s timer.
    fireStreamEvent(makePosting('early'));

    // Unmount BEFORE the 1 s elapses — cleanup effect clears the timer.
    act(() => {
      unmount();
    });

    // Advance past 1 s — the cleared timer must NOT fire.
    await advance(1500);

    expect(invalidateSpy).not.toHaveBeenCalled();
  });

  it('rejection from invalidatePostings does not become an unhandled rejection and throttle continues to work', async () => {
    // Make the first call reject; subsequent calls resolve normally.
    invalidateSpy.mockRejectedValueOnce(new Error('network error'));

    const { unmount } = renderJobsPage();

    // Fire several rapid ticks within the throttle window — only the leading
    // call (after the timer) should run, and it rejects.
    for (let i = 0; i < 3; i++) {
      fireStreamEvent(makePosting(`reject-item-${i}`));
    }

    // (a) Advance past 1 s — timer fires; rejected promise must not throw or
    // cause an unhandled rejection (the .catch(() => {}) absorbs it).
    await advance(1001);
    // The spy was called once (rejected) and the test itself did not throw.
    expect(invalidateSpy).toHaveBeenCalledTimes(1);

    // (b) Throttle still works after a rejection: fire another tick after the
    // window has elapsed — a new timer should be set and the second call
    // (which resolves) triggers a fresh invalidation.
    fireStreamEvent(makePosting('reject-after'));

    await advance(1001);
    expect(invalidateSpy).toHaveBeenCalledTimes(2);

    unmount();
  });

  it('zero ticks → invalidatePostings never called by the throttle', async () => {
    const { unmount } = renderJobsPage();

    await advance(2000);

    expect(invalidateSpy).not.toHaveBeenCalled();
    unmount();
  });

  it('replacePending=true → invalidatePostings called immediately (eager, before throttle window)', async () => {
    // The "first item of a new search" branch: when jobs.replacePending is true
    // the handler clears the latch, replaces livePostings with just the new
    // item, and calls invalidatePostings() DIRECTLY — bypassing the ~1 s timer
    // so the backend cache is flushed without waiting.
    resetStream(true);
    const { unmount } = renderJobsPage();

    // Fire one stream event while replacePending is true.
    fireStreamEvent(makePosting('replace-item'));

    // Assert BEFORE advancing fake timers — the eager call must have happened
    // synchronously within the event handler, not deferred to the throttle.
    expect(invalidateSpy).toHaveBeenCalledTimes(1);

    // The latch must be consumed (reset to false) so a second tick falls
    // through to the normal throttled path (no immediate second call).
    fireStreamEvent(makePosting('follow-up'));

    // Still only the one eager call — the follow-up is now throttled.
    expect(invalidateSpy).toHaveBeenCalledTimes(1);

    // Advance past the throttle window; the follow-up timer fires once more.
    await advance(1001);
    expect(invalidateSpy).toHaveBeenCalledTimes(2);

    unmount();
  });

  it('replacePending=true + rejection → no unhandled rejection, latch consumed, follow-up is throttled', async () => {
    // Make the eager invalidatePostings call reject once.
    invalidateSpy.mockRejectedValueOnce(new Error('eager network error'));
    resetStream(true);
    const { unmount } = renderJobsPage();

    // (a) Fire one stream event while replacePending is true — the eager path
    // runs, the rejection must be swallowed (no unhandled rejection / test throw).
    fireStreamEvent(makePosting('eager-reject-item'));

    // The eager call happened immediately (synchronous — before any timer).
    expect(invalidateSpy).toHaveBeenCalledTimes(1);

    // (b) The latch must have been consumed (reset to false) so the next tick
    // is handled by the throttled path, not another eager call.
    fireStreamEvent(makePosting('eager-follow-up'));

    // Still only the one eager call right now — the follow-up is throttled.
    expect(invalidateSpy).toHaveBeenCalledTimes(1);

    // Advance past the throttle window — the follow-up timer fires once more.
    await advance(1001);
    expect(invalidateSpy).toHaveBeenCalledTimes(2);

    unmount();
  });
});
