/**
 * JobsResults — gating + list-order tests.
 *
 * Scores are now on-demand (fetched when the user opens a job), so:
 *  - only `scraping` gates the list (no score-batch wait)
 *  - rows render in the `filtered` input order — never reordered by score
 *  - the "scoring" spinner is gone; only "searching" gates
 *
 * These tests drive the REAL MatchScoresProvider with the on-demand model.
 */
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/dom';
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { type BoardScrapeSummary, PROVIDER_SLOTS } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

// ── i18n stub ─────────────────────────────────────────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── router stub ───────────────────────────────────────────────────────────────

const mockNavigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useRouter: () => ({ navigate: mockNavigate }),
}));

// ── session-store stub ────────────────────────────────────────────────────────

const mockSetSettings = vi.fn();
const mockSetJobs = vi.fn();

const STORE_STATE: {
  setSettings: typeof mockSetSettings;
  jobs: { viewMode: string; selectedId: string | null };
  setJobs: typeof mockSetJobs;
} = {
  setSettings: mockSetSettings,
  jobs: { viewMode: 'list', selectedId: null },
  setJobs: mockSetJobs,
};

vi.mock('@/store/session-store', () => ({
  useSessionStore: (sel?: (s: typeof STORE_STATE) => unknown) =>
    sel ? sel(STORE_STATE) : STORE_STATE,
}));

// ── useHasProviderKey stub ────────────────────────────────────────────────────

let stubbedHasByProvider: Record<string, boolean> = {
  [PROVIDER_SLOTS.adzunaAppId]: true,
  [PROVIDER_SLOTS.adzunaAppKey]: true,
};
let stubbedKeyIsSuccess = true;
vi.mock('@/services/use-ai-provider', () => ({
  useHasProviderKey: (provider: string, enabled = true) => ({
    data: enabled ? { has: stubbedHasByProvider[provider] ?? false } : undefined,
    isSuccess: enabled ? stubbedKeyIsSuccess : false,
  }),
}));

// ── MatchScoresProvider dependency — provider calls useJobMatchScore per row ──

vi.mock('@/services', () => ({
  useJobMatchScore: () => ({ data: undefined }),
}));

// ── PostingRow stub ───────────────────────────────────────────────────────────

vi.mock('@/features/jobs/components/PostingRow', () => ({
  PostingRow: ({ posting }: { posting: { id: string; title: string } }) => (
    <div data-testid={TEST_IDS.jobs.postingRow} data-id={posting.id}>
      {posting.title}
    </div>
  ),
}));

// ── JobsSplitView stub ────────────────────────────────────────────────────────

vi.mock('@/features/jobs/components/JobsSplitView', () => ({
  JobsSplitView: ({ display }: { display: { id: string }[] }) => (
    <div data-testid="jobs-split-view" data-count={display.length} />
  ),
}));

// ── virtualizer stub ──────────────────────────────────────────────────────────

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 88,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        key: index,
        index,
        start: index * 88,
      })),
    measureElement: () => {},
  }),
}));

import { MatchScoresProvider } from '@/features/jobs/providers';
import type { Posting } from '@/features/jobs/types';

import { JobsResults } from './index';

// ── constants ─────────────────────────────────────────────────────────────────

const RESUME_ID = 'resume-xyz';

function posting(id: string, title: string): Posting {
  return {
    id,
    source: 'linkedin',
    externalId: id,
    url: `https://example.com/${id}`,
    title,
    company: 'Acme',
    description: '',
    capturedAt: 0,
  };
}

const noop = () => {};
const formatRelativeTime = () => '';

function renderResults(opts: {
  filtered: Posting[];
  scraping?: boolean;
  resumeId?: string | null;
  boardSummaries?: BoardScrapeSummary[];
  failureNote?: string | null;
}) {
  const resumeId = 'resumeId' in opts ? (opts.resumeId ?? null) : RESUME_ID;
  const wrapper = ({ children }: { children: ReactNode }) => (
    <MatchScoresProvider resumeId={resumeId}>{children}</MatchScoresProvider>
  );
  return render(
    <JobsResults
      filtered={opts.filtered}
      formatRelativeTime={formatRelativeTime}
      scraping={opts.scraping ?? false}
      boardSummaries={opts.boardSummaries}
      failureNote={opts.failureNote}
      onShowMore={noop}
      onScrape={noop}
    />,
    { wrapper }
  );
}

function rowOrder(): string[] {
  return Array.from(document.querySelectorAll(`[data-testid="${TEST_IDS.jobs.postingRow}"]`)).map(
    (el) => el.getAttribute('data-id') ?? ''
  );
}

// ── reset ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  stubbedHasByProvider = {
    [PROVIDER_SLOTS.adzunaAppId]: true,
    [PROVIDER_SLOTS.adzunaAppKey]: true,
  };
  stubbedKeyIsSuccess = true;
  mockNavigate.mockClear();
  mockSetSettings.mockClear();
  mockSetJobs.mockClear();
  STORE_STATE.jobs = { viewMode: 'list', selectedId: null };
});

// ── gating ────────────────────────────────────────────────────────────────────

describe('JobsResults — gating', () => {
  it('shows the searching state and no rows while scraping with no results yet (fresh search)', () => {
    // Skeleton only fires on a fresh search: scraping=true AND filtered is empty.
    // During show-more (filtered has items + scraping) the list stays visible.
    renderResults({ filtered: [], scraping: true });

    expect(screen.getByText('jobs.searching')).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.jobs.postingRow)).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.showMore')).not.toBeInTheDocument();
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

// ── list order (input order preserved — no score reorder) ─────────────────────

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

// ── empty state ───────────────────────────────────────────────────────────────

describe('JobsResults — empty state', () => {
  it('shows the empty state when filtered is empty and not scraping', () => {
    renderResults({ filtered: [], resumeId: null });

    expect(screen.getByText('jobs.empty')).toBeInTheDocument();
    expect(screen.queryByText('jobs.searching')).not.toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.jobs.postingRow)).not.toBeInTheDocument();
  });

  it('shows the Adzuna-keys CTA when the list is empty and keys are missing', () => {
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppId] = false;
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppKey] = false;
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
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppId] = false;
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppKey] = false;
    stubbedKeyIsSuccess = false;
    renderResults({ filtered: [], resumeId: null });

    expect(screen.getByText('jobs.emptyCta')).toBeInTheDocument();
    expect(screen.queryByText('jobs.emptyNoAdzunaKeys')).not.toBeInTheDocument();
  });

  it('wraps the empty-state variant swap in a live region so AT users hear the change', () => {
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppId] = false;
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppKey] = false;
    const { container } = renderResults({ filtered: [], resumeId: null });

    const live = container.querySelector('[role="status"][aria-live="polite"]');
    expect(live).not.toBeNull();
    expect(live).toHaveTextContent('jobs.emptyNoAdzunaKeys');
  });

  it('clicking the missing-keys CTA calls setSettings and navigates to /settings', async () => {
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppId] = false;
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppKey] = false;
    renderResults({ filtered: [], resumeId: null });

    const cta = screen.getByText('jobs.emptyNoAdzunaKeysCta');
    await userEvent.click(cta);

    expect(mockSetSettings).toHaveBeenCalledOnce();
    expect(mockSetSettings).toHaveBeenCalledWith({ activeSection: 'job' });
    expect(mockNavigate).toHaveBeenCalledOnce();
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/settings' });
  });
});

// ── board diagnostics wired into the empty state (HIGH #4) ───────────────────

describe('JobsResults — board diagnostics in the empty state', () => {
  it('renders the chip strip when filtered=[] and boardSummaries has skipped/error entries', () => {
    const boardSummaries: BoardScrapeSummary[] = [
      { board: 'linkedin', count: 0, error: 'blocked' },
      { board: 'indeed', count: 0, skipped: 'needs-login' },
    ];
    renderResults({ filtered: [], resumeId: null, boardSummaries });

    expect(screen.getByRole('group', { name: 'jobs.boardSummary.label' })).toBeInTheDocument();
    expect(screen.getByText('jobs.boards.linkedin')).toBeInTheDocument();
    expect(screen.getByText('jobs.boards.indeed')).toBeInTheDocument();
  });

  it('does NOT render the strip when boardSummaries is undefined', () => {
    renderResults({ filtered: [], resumeId: null });
    expect(
      screen.queryByRole('group', { name: 'jobs.boardSummary.label' })
    ).not.toBeInTheDocument();
  });

  it('does NOT render the strip when boardSummaries is an empty array', () => {
    renderResults({ filtered: [], resumeId: null, boardSummaries: [] });
    expect(
      screen.queryByRole('group', { name: 'jobs.boardSummary.label' })
    ).not.toBeInTheDocument();
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
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppId] = false;
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppKey] = false;
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
    expect(
      screen.queryByRole('group', { name: 'jobs.boardSummary.label' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.lastScrapeFailed')).not.toBeInTheDocument();
  });
});

// ── split-mode auto-select ────────────────────────────────────────────────────

describe('JobsResults — split-mode auto-select', () => {
  it('selects display[0] immediately when split mode has results and no current selection', () => {
    STORE_STATE.jobs = { viewMode: 'split', selectedId: null };
    const p1 = posting('a', 'A');
    const p2 = posting('b', 'B');

    renderResults({ filtered: [p1, p2], resumeId: null });

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'a' });
  });

  it('does NOT clobber a valid manual selection on a plain re-render', () => {
    STORE_STATE.jobs = { viewMode: 'split', selectedId: 'b' };
    const p1 = posting('a', 'A');
    const p2 = posting('b', 'B');

    renderResults({ filtered: [p1, p2], resumeId: null });

    const autoSelectCalls = mockSetJobs.mock.calls.filter(
      (args) => (args[0] as { selectedId?: string }).selectedId === 'a'
    );
    expect(autoSelectCalls).toHaveLength(0);
  });

  it('re-selects display[0] when the selected job is filtered OUT of display after mount', () => {
    STORE_STATE.jobs = { viewMode: 'split', selectedId: 'b' };
    const p1 = posting('a', 'A');
    const p2 = posting('b', 'B');

    const { rerender } = renderResults({ filtered: [p1, p2], resumeId: null });
    mockSetJobs.mockClear();

    rerender(
      <JobsResults
        filtered={[p1]}
        formatRelativeTime={() => ''}
        scraping={false}
        onShowMore={noop}
        onScrape={noop}
      />
    );

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'a' });
  });

  it('re-selects display[0] when a fresh scrape finishes (filtered empty→populated, scraping true→false)', () => {
    // waiting = scraping && filtered.length === 0.
    // Fresh search: start with empty filtered + scraping=true (waiting=true),
    // then results arrive → !selectionInDisplay fires auto-select.
    // Show-more does NOT trigger this path — it starts with items already present.
    STORE_STATE.jobs = { viewMode: 'split', selectedId: null };
    const p1 = posting('x', 'X');
    const p2 = posting('y', 'Y');

    // Initial render: no results yet, scraping in progress → waiting=true
    const { rerender } = renderResults({ filtered: [], resumeId: null, scraping: true });
    mockSetJobs.mockClear();

    // Scrape finishes: results arrive + scraping done → !selectionInDisplay → auto-select topId
    rerender(
      <JobsResults
        filtered={[p1, p2]}
        formatRelativeTime={() => ''}
        scraping={false}
        onShowMore={noop}
        onScrape={noop}
      />
    );

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'x' });
  });

  it('does not auto-select in list mode', () => {
    STORE_STATE.jobs = { viewMode: 'list', selectedId: null };
    const p1 = posting('a', 'A');

    renderResults({ filtered: [p1], resumeId: null });

    const autoSelectCalls = mockSetJobs.mock.calls.filter(
      (args) => (args[0] as { selectedId?: string }).selectedId === 'a'
    );
    expect(autoSelectCalls).toHaveLength(0);
  });
});

// ── selection-preservation across re-scrapes and show-more ───────────────────
//
// auto-select fires only when !selectionInDisplay (selectedId not in filtered).
// These tests verify that a user's chosen job is NOT replaced when new items
// arrive (show-more, live prepend, or a re-scrape that keeps their selection).

describe('JobsResults — selection preserved across re-scrapes and show-more', () => {
  it('preserves the selected job when show-more completes (scraping true→false, selection stays in list)', () => {
    // auto-select must NOT fire when selectionInDisplay is already true.
    STORE_STATE.jobs = { viewMode: 'split', selectedId: 'b' };
    const p1 = posting('a', 'A');
    const p2 = posting('b', 'B');

    // Initial render: list is already populated (show-more scenario, not fresh search).
    const { rerender } = renderResults({ filtered: [p1, p2], scraping: true, resumeId: null });
    mockSetJobs.mockClear();

    // Show-more completes: more items arrive prepended, scraping=false.
    // The previously selected 'b' is still in the list.
    const p0 = posting('new', 'New');
    rerender(
      <JobsResults
        filtered={[p0, p1, p2]}
        formatRelativeTime={() => ''}
        scraping={false}
        onShowMore={noop}
        onScrape={noop}
      />
    );

    // Selection 'b' is still valid — setJobs must NOT be called to replace it.
    const overrideCalls = mockSetJobs.mock.calls.filter(
      (args) => (args[0] as { selectedId?: string }).selectedId !== 'b'
    );
    expect(overrideCalls).toHaveLength(0);
  });

  it('preserves the selected job when new items are live-prepended during an active scrape', () => {
    // Simulates live prepend: scraping stays true, new items arrive at top.
    STORE_STATE.jobs = { viewMode: 'split', selectedId: 'original' };
    const original = posting('original', 'Original');

    const { rerender } = renderResults({
      filtered: [original],
      scraping: true,
      resumeId: null,
    });
    mockSetJobs.mockClear();

    // New items are prepended while scraping continues. 'original' stays in list.
    const newer1 = posting('newer1', 'Newer 1');
    const newer2 = posting('newer2', 'Newer 2');
    rerender(
      <JobsResults
        filtered={[newer2, newer1, original]}
        formatRelativeTime={() => ''}
        scraping={true}
        onShowMore={noop}
        onScrape={noop}
      />
    );

    // topId is now 'newer2', but selectedId 'original' is still in the list.
    // setJobs must NOT fire to replace the user's selection.
    const overrideCalls = mockSetJobs.mock.calls.filter(
      (args) => (args[0] as { selectedId?: string }).selectedId !== 'original'
    );
    expect(overrideCalls).toHaveLength(0);
  });

  it('auto-selects the new topId when a re-scrape returns results but selection is absent (null)', () => {
    // Complementary case: selection was null going into the re-scrape (e.g. user
    // cleared it). When results arrive the detail pane must not stay blank.
    STORE_STATE.jobs = { viewMode: 'split', selectedId: null };
    const p1 = posting('a', 'A');
    const p2 = posting('b', 'B');

    const { rerender } = renderResults({ filtered: [], scraping: true, resumeId: null });
    mockSetJobs.mockClear();

    rerender(
      <JobsResults
        filtered={[p1, p2]}
        formatRelativeTime={() => ''}
        scraping={false}
        onShowMore={noop}
        onScrape={noop}
      />
    );

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'a' });
  });

  it('auto-selects topId when selection is filtered out during show-more rerender', () => {
    // Edge case: show-more changes the active filter, removing the selected job.
    // The effect must fall back to topId (not leave detail pane blank).
    STORE_STATE.jobs = { viewMode: 'split', selectedId: 'b' };
    const p1 = posting('a', 'A');
    const p2 = posting('b', 'B');

    const { rerender } = renderResults({ filtered: [p1, p2], scraping: true, resumeId: null });
    mockSetJobs.mockClear();

    // After show-more, 'b' is no longer in the filtered list.
    const p3 = posting('c', 'C');
    rerender(
      <JobsResults
        filtered={[p1, p3]}
        formatRelativeTime={() => ''}
        scraping={false}
        onShowMore={noop}
        onScrape={noop}
      />
    );

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'a' });
  });

  it('does NOT auto-select in list mode during show-more rerenders', () => {
    // List mode is always a no-op regardless of scraping transitions.
    STORE_STATE.jobs = { viewMode: 'list', selectedId: null };
    const p1 = posting('a', 'A');

    const { rerender } = renderResults({ filtered: [p1], scraping: true, resumeId: null });
    mockSetJobs.mockClear();

    const p2 = posting('b', 'B');
    rerender(
      <JobsResults
        filtered={[p1, p2]}
        formatRelativeTime={() => ''}
        scraping={false}
        onShowMore={noop}
        onScrape={noop}
      />
    );

    expect(mockSetJobs).not.toHaveBeenCalled();
  });
});
