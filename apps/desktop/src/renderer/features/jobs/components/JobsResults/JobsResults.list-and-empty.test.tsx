/**
 * JobsResults — gating, list order, and the empty state (with its per-board
 * diagnostics).
 *
 *  - only `scraping` gates the list (no score-batch wait)
 *  - rows render in the `filtered` input order — never reordered by score
 */
import { beforeEach, describe, expect, it } from 'vitest';
import { screen } from '@testing-library/dom';
import userEvent from '@testing-library/user-event';

import type { BoardScrapeSummary } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

import {
  mockNavigate,
  mockSetSettings,
  posting,
  providerKeys,
  renderResults,
  resetResults,
  rowOrder,
  withoutAdzunaKeys,
} from './results-harness';

beforeEach(resetResults);

const boardGroup = { name: 'jobs.boardSummary.label' };

describe('JobsResults — gating', () => {
  it('shows the searching state and no rows while scraping with no results yet (fresh search)', () => {
    // Skeleton only fires on a fresh search: scraping=true AND filtered is empty.
    // During show-more (filtered has items + scraping) the list stays visible.
    renderResults({ filtered: [], scraping: true });

    expect(screen.getByText('jobs.searching')).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.jobs.postingRow)).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.showMore')).not.toBeInTheDocument();
  });

  it('leaves the scanning copy to the command bar — the bar under the progress fill is unlabelled', () => {
    renderResults({ filtered: [], scraping: true, scrapeProgress: 0.42 });

    // The command bar's status strip sits ~60px above with the SAME copy and
    // owns Cancel; two identical lines updating out of step read as a stutter.
    expect(screen.queryByText(/jobs\.scanningPercent/)).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.scanning')).not.toBeInTheDocument();
    // The fill itself stays — it is the non-duplicated part of the signal.
    expect(screen.getByText('jobs.searching')).toBeInTheDocument();
  });

  it('keeps existing rows visible during show-more (scraping=true with results already present)', () => {
    renderResults({ filtered: [posting('a', 'A'), posting('b', 'B')], scraping: true });

    expect(screen.queryByText('jobs.searching')).not.toBeInTheDocument();
    expect(rowOrder()).toEqual(['a', 'b']);
  });

  it('reveals rows immediately when not scraping (no score-batch wait)', () => {
    // With the on-demand model, rows render as soon as scraping=false — no scoring wait.
    renderResults({ filtered: [posting('a', 'A'), posting('b', 'B')] });

    expect(screen.queryByText('jobs.searching')).not.toBeInTheDocument();
    expect(rowOrder()).toEqual(['a', 'b']);
  });
});

describe('JobsResults — list order', () => {
  it('renders rows in filtered input order regardless of any cached scores', () => {
    const filtered = [
      posting('first', 'First'),
      posting('second', 'Second'),
      posting('third', 'Third'),
    ];

    renderResults({ filtered });

    expect(rowOrder()).toEqual(['first', 'second', 'third']);
  });

  it('renders rows in filtered input order when no scores are cached', () => {
    const filtered = [posting('c', 'C'), posting('a', 'A'), posting('b', 'B')];

    renderResults({ filtered, resumeId: null });

    expect(rowOrder()).toEqual(['c', 'a', 'b']);
  });

  it('renders rows immediately in input order when resumeId is null', () => {
    const filtered = [posting('a', 'A'), posting('b', 'B'), posting('c', 'C')];

    renderResults({ filtered, resumeId: null });

    expect(rowOrder()).toEqual(['a', 'b', 'c']);
  });
});

describe('JobsResults — empty state', () => {
  it('shows the empty state when filtered is empty and not scraping', () => {
    renderResults({ filtered: [], resumeId: null });

    expect(screen.getByText('jobs.empty')).toBeInTheDocument();
    expect(screen.queryByText('jobs.searching')).not.toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.jobs.postingRow)).not.toBeInTheDocument();
  });

  it('shows the Adzuna-keys CTA when the list is empty and keys are missing', () => {
    withoutAdzunaKeys();
    renderResults({ filtered: [], resumeId: null });

    expect(screen.getByText('jobs.emptyNoAdzunaKeys')).toBeInTheDocument();
    expect(screen.getByText('jobs.emptyNoAdzunaKeysCta')).toBeInTheDocument();
    expect(screen.queryByText('jobs.emptyCta')).not.toBeInTheDocument();
  });

  it('shows the generic CTA when the list is empty and keys are present', () => {
    renderResults({ filtered: [], resumeId: null });

    expect(screen.getByText('jobs.emptyCta')).toBeInTheDocument();
    expect(screen.queryByText('jobs.emptyNoAdzunaKeys')).not.toBeInTheDocument();
  });

  it('shows the generic CTA (not the keys CTA) while the key queries are still loading', () => {
    withoutAdzunaKeys();
    providerKeys.isSuccess = false;
    renderResults({ filtered: [], resumeId: null });

    expect(screen.getByText('jobs.emptyCta')).toBeInTheDocument();
    expect(screen.queryByText('jobs.emptyNoAdzunaKeys')).not.toBeInTheDocument();
  });

  it('wraps the empty-state variant swap in a live region so AT users hear the change', () => {
    withoutAdzunaKeys();
    const { container } = renderResults({ filtered: [], resumeId: null });

    const live = container.querySelector('[role="status"][aria-live="polite"]');
    expect(live).not.toBeNull();
    expect(live).toHaveTextContent('jobs.emptyNoAdzunaKeys');
  });

  it('clicking the missing-keys CTA calls setSettings and navigates to /settings', async () => {
    withoutAdzunaKeys();
    renderResults({ filtered: [], resumeId: null });

    const cta = screen.getByText('jobs.emptyNoAdzunaKeysCta');
    await userEvent.click(cta);

    expect(mockSetSettings).toHaveBeenCalledOnce();
    expect(mockSetSettings).toHaveBeenCalledWith({ activeSection: 'job' });
    expect(mockNavigate).toHaveBeenCalledOnce();
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/settings' });
  });
});

describe('JobsResults — board diagnostics in the empty state', () => {
  it('renders the chip strip when filtered=[] and boardSummaries has skipped/error entries', () => {
    const boardSummaries: BoardScrapeSummary[] = [
      { board: 'linkedin', count: 0, error: 'blocked' },
      { board: 'indeed', count: 0, skipped: 'needs-login' },
    ];
    renderResults({ filtered: [], resumeId: null, boardSummaries });

    expect(screen.getByRole('group', boardGroup)).toBeInTheDocument();
    expect(screen.getByText('jobs.boards.linkedin')).toBeInTheDocument();
    expect(screen.getByText('jobs.boards.indeed')).toBeInTheDocument();
  });

  it('does NOT render the strip when boardSummaries is undefined', () => {
    renderResults({ filtered: [], resumeId: null });
    expect(screen.queryByRole('group', boardGroup)).not.toBeInTheDocument();
  });

  it('does NOT render the strip when boardSummaries is an empty array', () => {
    renderResults({ filtered: [], resumeId: null, boardSummaries: [] });
    expect(screen.queryByRole('group', boardGroup)).not.toBeInTheDocument();
  });

  it('renders the failure note when set', () => {
    renderResults({ filtered: [], resumeId: null, failureNote: 'connection refused' });
    expect(screen.getByText('jobs.lastScrapeFailed')).toBeInTheDocument();
  });

  it('does NOT render the failure note when absent', () => {
    renderResults({ filtered: [], resumeId: null });
    expect(screen.queryByText('jobs.lastScrapeFailed')).not.toBeInTheDocument();
  });

  it('suppresses BOTH the chip strip and the failure note when missingAdzunaKeys already explains the zero (no triple-explaining)', () => {
    withoutAdzunaKeys();
    const boardSummaries: BoardScrapeSummary[] = [
      { board: 'aggregator', count: 0, skipped: 'needs-keys' },
    ];
    renderResults({
      filtered: [],
      resumeId: null,
      boardSummaries,
      failureNote: 'boom',
    });

    expect(screen.getByText('jobs.emptyNoAdzunaKeys')).toBeInTheDocument();
    expect(screen.queryByRole('group', boardGroup)).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.lastScrapeFailed')).not.toBeInTheDocument();
  });

  // `totalCount` (unfiltered posting count) lets the empty state tell "the
  // text filter hid every posting that exists" apart from "there really are
  // zero postings". Only the genuinely-zero case may re-show the last
  // scrape's diagnostics — a filter-hides-all view must NOT imply the scrape
  // itself found nothing.

  it('filter hides all postings (totalCount > 0, filtered = []) → NO chips/note even though both are set', () => {
    const boardSummaries: BoardScrapeSummary[] = [
      { board: 'linkedin', count: 12, error: 'blocked' },
    ];
    renderResults({
      filtered: [],
      resumeId: null,
      boardSummaries,
      failureNote: 'connection refused',
      totalCount: 12, // 12 postings exist; the active text filter hid all of them
    });

    expect(screen.queryByRole('group', boardGroup)).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.lastScrapeFailed')).not.toBeInTheDocument();
    // The plain empty-state title still renders (filter just hid everything).
    expect(screen.getByText('jobs.empty')).toBeInTheDocument();
  });

  it('genuinely zero postings (totalCount = 0) → chips/note DO render', () => {
    const boardSummaries: BoardScrapeSummary[] = [
      { board: 'linkedin', count: 0, error: 'blocked' },
    ];
    renderResults({
      filtered: [],
      resumeId: null,
      boardSummaries,
      failureNote: 'connection refused',
      totalCount: 0,
    });

    expect(screen.getByRole('group', boardGroup)).toBeInTheDocument();
    expect(screen.getByText('jobs.lastScrapeFailed')).toBeInTheDocument();
  });

  it('totalCount omitted (no filtering call site) → falls back to filtered, chips/note still render for filtered=[]', () => {
    const boardSummaries: BoardScrapeSummary[] = [
      { board: 'linkedin', count: 0, error: 'blocked' },
    ];
    renderResults({ filtered: [], resumeId: null, boardSummaries, failureNote: 'boom' });

    expect(screen.getByRole('group', boardGroup)).toBeInTheDocument();
    expect(screen.getByText('jobs.lastScrapeFailed')).toBeInTheDocument();
  });
});
