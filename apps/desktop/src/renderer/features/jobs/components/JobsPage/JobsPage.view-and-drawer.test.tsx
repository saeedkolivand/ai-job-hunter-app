/**
 * JobsPage — view-mode toggle, stable "newest" sort, the scrape drawer, and the
 * work-type control visibility gate.
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { act, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

import { useSessionStore } from '@/store/session-store';

import { makePosting } from './fixtures';
import { fireJobEvent } from './job-events';
import {
  drawerContainer,
  postingsContainer,
  renderJobsPage,
  resetPage,
  resultsProps,
  samplePosting,
  scrapeFormContainer,
  segmentedControlContainer,
} from './page-harness';

beforeEach(resetPage);

describe('JobsPage — SegmentedControl viewMode toggle', () => {
  it('switching to "split" stores viewMode:split', () => {
    renderJobsPage();
    expect(segmentedControlContainer.onChange).toBeTypeOf('function');

    act(() => {
      segmentedControlContainer.onChange?.('split');
    });

    expect(useSessionStore.getState().jobs.viewMode).toBe('split');
  });

  it('switching to "list" stores viewMode:list', () => {
    useSessionStore.setState((s) => ({ jobs: { ...s.jobs, viewMode: 'split' } }));
    renderJobsPage();
    expect(segmentedControlContainer.onChange).toBeTypeOf('function');

    act(() => {
      segmentedControlContainer.onChange?.('list');
    });

    expect(useSessionStore.getState().jobs.viewMode).toBe('list');
  });
});

// Stable "newest" sort (PR H, audit quick win 8): equal timestamps get a
// deterministic id tiebreak, and undated postings (no `postedAt`) collect in a
// trailing band instead of interleaving via the `capturedAt` fallback.
// `makeJobsDefaults()` defaults to sortBy: 'newest', so these assert that path.

// Distinct url/title so mergePostings' canonical-key dedup keeps every row.
const sortPosting = (id: string, opts: { postedAt?: number; capturedAt: number }) =>
  makePosting(id, { title: `Engineer ${id}`, ...opts });

function filteredIds(): string[] {
  return (resultsProps.filtered as Array<{ id: string }>).map((p) => p.id);
}

describe('JobsPage — stable newest sort', () => {
  it('equal postedAt falls back to a deterministic id tiebreak (not input order)', () => {
    // Fed in reverse id order; a plain stable sort would keep it, so a passing
    // ['a','b'] proves the id tiebreak actually ran.
    postingsContainer.data = [
      sortPosting('b', { postedAt: 1000, capturedAt: 5 }),
      sortPosting('a', { postedAt: 1000, capturedAt: 5 }),
    ];
    renderJobsPage();
    expect(filteredIds()).toEqual(['a', 'b']);
  });

  it('undated postings (no postedAt) trail the dated ones, never interleaved', () => {
    postingsContainer.data = [
      // No postedAt but a very recent capture — must NOT jump above the dated row.
      sortPosting('undated', { capturedAt: 9999 }),
      sortPosting('dated', { postedAt: 100, capturedAt: 1 }),
    ];
    renderJobsPage();
    expect(filteredIds()).toEqual(['dated', 'undated']);
  });

  it('dated band is newest-first; undated band trails, sorted by capture then id', () => {
    postingsContainer.data = [
      sortPosting('old', { postedAt: 100, capturedAt: 1 }),
      sortPosting('new', { postedAt: 200, capturedAt: 1 }),
      sortPosting('u2', { capturedAt: 50 }),
      sortPosting('u1', { capturedAt: 50 }),
    ];
    renderJobsPage();
    // Dated newest-first: new, old. Undated trail; equal capture → id tiebreak.
    expect(filteredIds()).toEqual(['new', 'old', 'u1', 'u2']);
  });
});

// The form moved out of the page flow into a right slide-over. It must be
// closed on mount (so it can never displace the results list), open from the
// command bar's Scrape action, and close again from the form's own dismiss
// control. jsdom can't measure layout, so the "tall form content can't clip the
// Start button" guarantee is asserted as the scroll class on the drawer body.
describe('JobsPage — scrape drawer', () => {
  const scrapeButton = () => screen.getByRole('button', { name: /jobs\.scrapeJobs/ });
  const scrapeForm = () => screen.queryByTestId(TEST_IDS.jobs.scrapeForm);

  it('is closed on mount — the form never occupies page flow', () => {
    renderJobsPage();
    expect(scrapeForm()).not.toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('opens from the command bar Scrape action and renders the reused ScrapeForm', async () => {
    const user = userEvent.setup();
    renderJobsPage();

    await user.click(scrapeButton());

    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByTestId(TEST_IDS.jobs.scrapeForm)).toBeInTheDocument();
  });

  it('closes on Search — in the click handler, not from a background stream event', async () => {
    const user = userEvent.setup();
    renderJobsPage();

    await user.click(scrapeButton());
    expect(screen.getByTestId(TEST_IDS.jobs.scrapeForm)).toBeInTheDocument();

    act(() => {
      scrapeFormContainer.onStart?.();
    });

    // Closing here (user-initiated) rather than from an effect on the first
    // streamed posting is what keeps focus predictable: the old effect yanked
    // focus mid-interaction and, on the first-run path, closed the drawer only
    // AFTER the empty-state CTA it would return focus to had unmounted.
    expect(scrapeForm()).not.toBeInTheDocument();
  });

  it('stays closed when a scrape returns ZERO results (nothing ever streams)', async () => {
    const user = userEvent.setup();
    renderJobsPage();

    await user.click(scrapeButton());
    act(() => {
      scrapeFormContainer.onStart?.();
    });

    // A completion with no postings must not strand the drawer open — the old
    // livePostings-driven close never fired for an empty result set.
    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: { boards: [{ board: 'linkedin', count: 0 }] },
    });

    expect(scrapeForm()).not.toBeInTheDocument();
  });

  it('hands the drawer an always-mounted focus-return target', async () => {
    const user = userEvent.setup();
    renderJobsPage();

    await user.click(scrapeButton());

    // The empty-state CTA can unmount while the drawer is open, so the drawer
    // needs a fallback that never does — the command bar's Scrape button.
    expect(drawerContainer.returnFocusTo?.current).toBe(scrapeButton());
  });

  it("closes again via the form's own dismiss control (onToggle)", async () => {
    const user = userEvent.setup();
    renderJobsPage();

    await user.click(scrapeButton());
    expect(screen.getByTestId(TEST_IDS.jobs.scrapeForm)).toBeInTheDocument();

    act(() => {
      scrapeFormContainer.onToggle?.();
    });

    expect(scrapeForm()).not.toBeInTheDocument();
  });

  it('the empty-state CTA opens the same drawer', async () => {
    renderJobsPage();
    expect(scrapeForm()).not.toBeInTheDocument();

    // JobsResults is stubbed — invoke the `onScrape` prop it was handed, which
    // is the empty state's "Search jobs" CTA on the real component.
    act(() => {
      (resultsProps.onScrape as (() => void) | undefined)?.();
    });

    expect(screen.getByTestId(TEST_IDS.jobs.scrapeForm)).toBeInTheDocument();
  });
});

// End-to-end through the REAL JobsCommandBar (not stubbed in this file),
// proving JobsPage derives `hasDeclaredWorkType` from the actual merged/rendered
// posting list rather than some parallel computation that could drift from it.
describe('JobsPage — work-type control visibility gate', () => {
  const workTypeGroup = { name: 'jobs.workType.label' };

  it('hides the work-type group when nothing on screen declares a workType and no selection is active', () => {
    postingsContainer.data = [samplePosting('a'), samplePosting('b')];
    renderJobsPage();
    expect(screen.queryByRole('group', workTypeGroup)).not.toBeInTheDocument();
  });

  it('shows the work-type group when at least one visible posting declares a workType', () => {
    postingsContainer.data = [samplePosting('a'), { ...samplePosting('b'), workType: 'remote' }];
    renderJobsPage();
    expect(screen.getByRole('group', workTypeGroup)).toBeInTheDocument();
  });

  // The trap: a selection made while an earlier, declaring result set was on
  // screen must stay visible (and clearable) even after a fresh search whose
  // results declare nothing — a naive `hasDeclaredWorkType`-only gate would
  // hide the only control that shows/clears a filter that is STILL applied,
  // leaving the page silently short of results with no visible cause.
  it('keeps the work-type group visible for an active selection even when the current results declare nothing', () => {
    postingsContainer.data = [samplePosting('a'), samplePosting('b')]; // nothing declares workType
    act(() => {
      useSessionStore.getState().setJobs({ workTypes: ['remote'] });
    });
    renderJobsPage();

    const group = screen.getByRole('group', workTypeGroup);
    expect(group).toBeInTheDocument();
  });
});
