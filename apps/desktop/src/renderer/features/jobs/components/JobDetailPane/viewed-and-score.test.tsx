/**
 * JobDetailPane — viewed-on-dwell, score-on-open, persist-then-score.
 *
 *  - The viewed effect uses a ref so re-renders with the SAME posting.id do not
 *    re-fire; switching posting.id fires exactly once more.
 *  - scoreJob is called ONCE on open after description is ready; NOT called for
 *    the whole list; NOT called when description is still loading.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import {
  flushMicrotasks,
  formatRelativeTime,
  JobDetailPane,
  makePosting,
  mockScoreJob,
  mockTrackInteraction,
  mockUpdateDescMutateAsync,
  openPane,
  rerenderPane,
  resetPaneMocks,
  resolveInFlight,
  resolveSettled,
} from './harness';

beforeEach(resetPaneMocks);

const advance = (ms: number) =>
  act(async () => {
    vi.advanceTimersByTime(ms);
  });

describe('JobDetailPane — viewed dwell timer', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    mockTrackInteraction.mockClear();
  });

  it('does NOT call trackInteraction("viewed") before 5s have elapsed', async () => {
    await openPane(makePosting('job-1'));
    // Advance to just under the threshold — must not have fired yet.
    await advance(4999);
    expect(mockTrackInteraction).not.toHaveBeenCalledWith('viewed');
  });

  it('calls trackInteraction("viewed") exactly once after 5s dwell', async () => {
    await openPane(makePosting('job-1'));
    await advance(5000);
    expect(mockTrackInteraction).toHaveBeenCalledTimes(1);
    expect(mockTrackInteraction).toHaveBeenCalledWith('viewed');
  });

  it('does NOT re-fire when re-rendered with the same posting.id after dwell', async () => {
    const posting = makePosting('job-1');
    const view = render(
      <JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />
    );
    await advance(5000);
    mockTrackInteraction.mockClear();

    // Re-render with same id (e.g. title update) — timer must NOT restart/refire.
    await rerenderPane(view, { ...posting, title: 'Updated title' });
    await advance(5000);

    expect(mockTrackInteraction).not.toHaveBeenCalledWith('viewed');
  });

  it('cancels and restarts timer when posting.id changes; fires once per job', async () => {
    const view = render(
      <JobDetailPane posting={makePosting('job-1')} formatRelativeTime={formatRelativeTime} />
    );
    // Switch jobs before the dwell fires — old timer cancels.
    await advance(3000);
    await rerenderPane(view, makePosting('job-2'));
    // Advance 5s from job-2 mount — only job-2's timer fires.
    await advance(5000);

    // job-1's timer was cancelled; only job-2 fires.
    expect(mockTrackInteraction).toHaveBeenCalledTimes(1);
    expect(mockTrackInteraction).toHaveBeenCalledWith('viewed');
  });

  it('unmount cancels the pending timer — "viewed" is never tracked after unmount', async () => {
    // Render a posting and advance 4s (timer is pending, has not fired yet).
    const { unmount } = render(
      <JobDetailPane posting={makePosting('job-unmount')} formatRelativeTime={formatRelativeTime} />
    );
    await advance(4000);
    // No call yet — still within the 5s dwell.
    expect(mockTrackInteraction).not.toHaveBeenCalledWith('viewed');

    // Unmount before the timer fires — clearTimeout in the cleanup must cancel it.
    unmount();

    // Advance well past the threshold; the callback must NOT fire post-unmount.
    await advance(5000);

    expect(mockTrackInteraction).not.toHaveBeenCalledWith('viewed');
  });
});

describe('JobDetailPane — score on open', () => {
  it('calls scoreJob(posting.id) once when description is immediately available', async () => {
    const posting = makePosting('score-ready', { description: 'Full description text.' });
    await openPane(posting);

    expect(mockScoreJob).toHaveBeenCalledTimes(1);
    expect(mockScoreJob).toHaveBeenCalledWith(posting.id);
  });

  it('does NOT call scoreJob while description is still loading (resolve in-flight)', async () => {
    resolveInFlight();
    await openPane(makePosting('score-loading', { description: '' }));

    // Description is empty + resolve in-flight → scoreJob must not fire yet.
    expect(mockScoreJob).not.toHaveBeenCalled();
  });

  it('does NOT score in the pre-fetch window (isFetched=false, isFetching=false)', async () => {
    // idleStub has isFetched=false — the query has not started yet.
    // Before the isFetched guard, resolveSettled would be true here (both flags false)
    // and scoring would fire on the snippet before the resolve query even begins.
    await openPane(
      makePosting('score-prefetch', { source: 'aggregator', description: 'Short snippet.' })
    );
    await flushMicrotasks();

    // isFetched=false → resolveSettled=false → scoreJob must NOT fire.
    expect(mockScoreJob).not.toHaveBeenCalled();
  });

  it('non-aggregator full description — scores immediately, no persist', async () => {
    const posting = makePosting('score-full', { description: 'Full description text.' });
    await openPane(posting);
    await flushMicrotasks();

    expect(mockScoreJob).toHaveBeenCalledTimes(1);
    expect(mockScoreJob).toHaveBeenCalledWith(posting.id);
    // No persist needed for already-full descriptions.
    expect(mockUpdateDescMutateAsync).not.toHaveBeenCalled();
  });

  it('resolve-not-longer — scores immediately, no persist', async () => {
    // Resolve returns a SHORTER or equal description → resolvedLonger=false.
    resolveSettled('Shorter.');
    await openPane(
      makePosting('score-not-longer', {
        source: 'aggregator',
        description: 'Original snippet that is longer than resolved.',
      })
    );
    await flushMicrotasks();

    expect(mockScoreJob).toHaveBeenCalledTimes(1);
    expect(mockUpdateDescMutateAsync).not.toHaveBeenCalled();
  });

  it('persist-then-score ORDER — update resolves before scoreJob fires on resolved-longer', async () => {
    // Track call order: 'update' pushed when persist resolves, 'score' pushed when scoreJob called.
    const callOrder: string[] = [];

    mockUpdateDescMutateAsync.mockImplementation(async () => {
      callOrder.push('update');
      return undefined;
    });
    mockScoreJob.mockImplementation((_id: string) => {
      callOrder.push('score');
    });

    resolveSettled('This is the full job description fetched from the redirect target.');
    const posting = makePosting('score-order', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await openPane(posting);
    // Let the async persist + scoreJob chain settle.
    await flushMicrotasks(2);

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);
    expect(mockScoreJob).toHaveBeenCalledTimes(1);
    expect(mockScoreJob).toHaveBeenCalledWith(posting.id);
    // Critical ordering assertion: update must precede score.
    expect(callOrder).toEqual(['update', 'score']);
  });

  it('persist failure is non-fatal — scoreJob still fires after updateDescription rejects', async () => {
    mockUpdateDescMutateAsync.mockRejectedValue(new Error('persist failed'));

    resolveSettled('Full description from resolve endpoint.');
    const posting = makePosting('score-persist-fail', {
      source: 'aggregator',
      description: 'Short snippet.',
    });
    await openPane(posting);
    await flushMicrotasks(2);

    // scoreJob fires even though persist failed.
    expect(mockScoreJob).toHaveBeenCalledTimes(1);
    expect(mockScoreJob).toHaveBeenCalledWith(posting.id);
  });

  it('one-shot guard — re-render with same posting does NOT call scoreJob again', async () => {
    const posting = makePosting('score-once', { description: 'Full description.' });
    const view = render(
      <JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />
    );
    await flushMicrotasks();

    expect(mockScoreJob).toHaveBeenCalledTimes(1);

    await rerenderPane(view, posting);

    // Guard ref blocks a second call.
    expect(mockScoreJob).toHaveBeenCalledTimes(1);
  });

  it('remount via new posting.id resets the guard and calls scoreJob for the new job', async () => {
    const postingA = makePosting('score-a', { description: 'Description A.' });
    const postingB = makePosting('score-b', { description: 'Description B.' });

    const view = render(
      <JobDetailPane posting={postingA} formatRelativeTime={formatRelativeTime} />
    );
    await flushMicrotasks();
    expect(mockScoreJob).toHaveBeenCalledTimes(1);

    // Switch job — key={posting.id} remounts DetailContent, resetting scoredRef.
    await rerenderPane(view, postingB);
    await flushMicrotasks();

    expect(mockScoreJob).toHaveBeenCalledTimes(2);
    expect(mockScoreJob).toHaveBeenLastCalledWith(postingB.id);
  });
});

describe('JobDetailPane — updateDescription persist on upgrade', () => {
  const aggregator = (id: string, description: string) =>
    makePosting(id, { source: 'aggregator', description });

  it('calls updateDescription once when the resolved description is longer', async () => {
    const fullDesc = 'This is the much longer full job description fetched from the target page.';
    resolveSettled(fullDesc);
    const posting = aggregator('persist-upgrade', 'Short snippet.');
    await openPane(posting);
    await flushMicrotasks();

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);
    expect(mockUpdateDescMutateAsync).toHaveBeenCalledWith({
      url: posting.url,
      description: fullDesc,
    });
  });

  it('one-shot guard — re-render does NOT call updateDescription again', async () => {
    resolveSettled('Much longer description than the snippet.');
    const posting = aggregator('persist-once', 'Short snippet.');
    const view = render(
      <JobDetailPane posting={posting} formatRelativeTime={formatRelativeTime} />
    );
    await flushMicrotasks();

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);

    await rerenderPane(view, posting);

    expect(mockUpdateDescMutateAsync).toHaveBeenCalledTimes(1);
  });

  it('does NOT call updateDescription when the resolved description is NOT longer', async () => {
    resolveSettled('Tiny.');
    await openPane(aggregator('no-persist', 'A snippet longer than tiny.'));

    expect(mockUpdateDescMutateAsync).not.toHaveBeenCalled();
  });
});
