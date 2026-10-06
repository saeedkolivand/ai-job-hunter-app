/**
 * JobsPage — allPostings dedup/merge (useMemo, backend-wins), via the rendered
 * list, plus the pure merge formula the component's useMemo mirrors.
 *
 * The `usePostings` mock always returns `[]`, so the component-level cases drive
 * the dynamic input (`livePostings`) and assert on what the stubbed JobsResults
 * received. The merge formula itself is covered against the real
 * `mergePostings` helper.
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { waitFor } from '@testing-library/react';

import { mergePostings } from '@/features/jobs/lib/merge-postings';
import type { Posting } from '@/features/jobs/types';

import { makePosting } from './fixtures';
import {
  fireStreamEvent,
  lastFiltered,
  renderedIds,
  renderJobsPage,
  rerenderJobsPage,
  resetStream,
  scrapingState,
} from './stream-harness';

describe('JobsPage — allPostings dedup (via rendered list)', () => {
  beforeEach(() => resetStream());

  it('empty livePostings and empty postings → no rows rendered', () => {
    renderJobsPage();
    expect(renderedIds()).toEqual([]);
  });

  it('two stream ticks with the same id → item appears exactly once in the rendered list', async () => {
    const view = renderJobsPage();

    // Event 1: component calls setLivePostings with a functional updater.
    fireStreamEvent(makePosting('x', { title: 'X first' }));
    await waitFor(() => expect(scrapingState.setLivePostings).toHaveBeenCalled());

    // Apply the updater the component produced so the mock's livePostings reflects
    // what real React state would hold, then re-render to propagate it.
    const updater1 = scrapingState.setLivePostings.mock.calls.at(-1)?.[0];
    if (typeof updater1 === 'function') {
      scrapingState.livePostings = updater1([]);
    }
    scrapingState.setLivePostings.mockClear();
    rerenderJobsPage(view);

    // Event 2: same id — the component's dedup guard must discard it.
    fireStreamEvent(makePosting('x', { title: 'X duplicate' }));
    await waitFor(() => expect(scrapingState.setLivePostings).toHaveBeenCalled());

    const updater2 = scrapingState.setLivePostings.mock.calls.at(-1)?.[0];
    if (typeof updater2 === 'function') {
      scrapingState.livePostings = updater2(scrapingState.livePostings);
    }
    rerenderJobsPage(view);

    // Assert on what the COMPONENT rendered — lastFiltered is captured by the
    // JobsResults stub and reflects the actual filtered prop passed to it.
    expect(lastFiltered.value.filter((p) => p.id === 'x')).toHaveLength(1);
    expect(renderedIds().filter((id) => id === 'x')).toHaveLength(1);

    view.unmount();
  });

  it('livePostings-only items appear and sort to the front of the merged list', () => {
    // livePostings=[p1,p3], postings=[] → allPostings=[p1,p3]
    scrapingState.livePostings = [makePosting('live-1'), makePosting('live-3')];
    renderJobsPage();

    const ids = renderedIds();
    // Both live items must appear.
    expect(ids).toContain('live-1');
    expect(ids).toContain('live-3');
    // live-1 precedes live-3 in input order → front-of-list preserved.
    expect(ids.indexOf('live-1')).toBeLessThan(ids.indexOf('live-3'));
  });
});

describe('allPostings merge formula — pure function', () => {
  // Uses the real mergePostings helper so test behaviour stays in lockstep
  // with the component's useMemo.
  const ids = (list: Posting[]) => list.map((p) => p.id);

  it('item in both livePostings and postings appears exactly once', () => {
    const shared = makePosting('shared');
    const result = mergePostings([shared], [shared]);
    expect(result.filter((p) => p.id === 'shared')).toHaveLength(1);
  });

  it('backend copy wins when id is in both — livePostings copy is dropped', () => {
    // Backend posting has interactions; live posting does not. Backend must win.
    const backendCopy = makePosting('shared', { company: 'BackendCo' });
    const liveCopy = makePosting('shared', { company: 'LiveCo' });
    const result = mergePostings([backendCopy], [liveCopy]);
    expect(result).toHaveLength(1);
    expect(result[0]?.company).toBe('BackendCo');
  });

  it('livePostings-only item appears (streamed mid-scrape, not yet in backend)', () => {
    const result = mergePostings([makePosting('backend')], [makePosting('live')]);
    expect(ids(result)).toContain('live');
    expect(ids(result)).toContain('backend');
  });

  it('postings-only item appears', () => {
    const result = mergePostings([makePosting('backend')], []);
    expect(ids(result)).toEqual(['backend']);
  });

  it('empty livePostings → result equals postings (same ids, same order)', () => {
    const postings = [makePosting('a'), makePosting('b'), makePosting('c')];
    const result = mergePostings(postings, []);
    expect(ids(result)).toEqual(['a', 'b', 'c']);
  });

  it('livePostings-only items appear when postings is empty', () => {
    const result = mergePostings([], [makePosting('live-1'), makePosting('live-2')]);
    expect(ids(result)).toContain('live-1');
    expect(ids(result)).toContain('live-2');
  });

  it('item present in livePostings but NOT in postings is not filtered out', () => {
    const result = mergePostings([], [makePosting('live')]);
    expect(ids(result)).toContain('live');
  });

  it('all items overlapping → output length equals postings length (no duplicates)', () => {
    const both = [makePosting('a'), makePosting('b')];
    const result = mergePostings(both, both);
    expect(result).toHaveLength(2);
    expect(new Set(ids(result)).size).toBe(2);
  });

  it('duplicate within livePostings itself — only one occurrence appears', () => {
    // Two DISTINCT objects with the same id: backend wins; only one entry emitted.
    const result = mergePostings(
      [makePosting('backend')],
      [makePosting('dup'), makePosting('dup')]
    );
    expect(result.filter((p) => p.id === 'dup')).toHaveLength(1);
  });

  it('backend ordering preserved when no live items exist', () => {
    const result = mergePostings([makePosting('stored-1'), makePosting('stored-2')], []);
    expect(ids(result)).toEqual(['stored-1', 'stored-2']);
  });
});
