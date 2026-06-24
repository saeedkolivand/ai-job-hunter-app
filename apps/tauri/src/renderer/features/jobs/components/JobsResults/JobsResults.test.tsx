/**
 * JobsResults — gating + reveal-sort tests.
 *
 * JobsResults hides the postings list behind a loading state until scraping has
 * finished AND (when a résumé exists) the match-score batch has settled; on
 * reveal it re-sorts rows by `combined` score descending. These tests drive the
 * REAL MatchScoresProvider with a stubbed useJobMatchScores so the gating
 * (`scraping || (hasResume && isPending)`) and the sort run for real, asserting:
 *  - scraping=true → searching state, no rows
 *  - hasResume + isPending → scoring state, no rows
 *  - settled + hasResume → rows sorted by combined desc (DOM order)
 *  - no résumé → rows render immediately in input order
 */
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/dom';
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { type MatchScore, PROVIDER_SLOTS } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

// ── i18n stub — identity t() so we assert on keys ─────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── router stub — navigate is a captured spy so tests can assert calls ────────

const mockNavigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useRouter: () => ({ navigate: mockNavigate }),
}));

// ── session-store stub — setSettings + jobs slice + setJobs ──────────────────

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
  // Component calls useSessionStore() (no selector) AND useSessionStore(sel).
  useSessionStore: (sel?: (s: typeof STORE_STATE) => unknown) =>
    sel ? sel(STORE_STATE) : STORE_STATE,
}));

// ── useHasProviderKey stub — per-provider map so the wrong slot fails ─────────
//
// The component calls useHasProviderKey(PROVIDER_SLOTS.adzunaAppId, ...) and
// useHasProviderKey(PROVIDER_SLOTS.adzunaAppKey, ...) separately. A shared
// scalar would pass even if the component queried the wrong slot, so we use a
// per-provider map keyed by slot name. Missing slots default to false.
//
// stubbedKeyIsSuccess=false simulates the queries still loading; missingAdzunaKeys
// stays false so the generic empty-state is shown (no false-positive "add keys" flash).

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

// ── useJobMatchScores stub — module-level ref set BEFORE each render ───────────

let stubbedQuery: { scoresById: Map<string, MatchScore>; isPending: boolean; isError: boolean } = {
  scoresById: new Map(),
  isPending: false,
  isError: false,
};

vi.mock('@/services', () => ({
  useJobMatchScores: () => stubbedQuery,
}));

// ── PostingRow stub — strip router/services; expose title + id for ordering ────

vi.mock('@/features/jobs/components/PostingRow', () => ({
  PostingRow: ({ posting }: { posting: { id: string; title: string } }) => (
    <div data-testid={TEST_IDS.jobs.postingRow} data-id={posting.id}>
      {posting.title}
    </div>
  ),
}));

// ── JobsSplitView stub — split mode renders this; keep it minimal ─────────────

vi.mock('@/features/jobs/components/JobsSplitView', () => ({
  JobsSplitView: ({ display }: { display: { id: string }[] }) => (
    <div data-testid="jobs-split-view" data-count={display.length} />
  ),
}));

// ── virtualizer stub — render every index in order (jsdom has no layout) ───────

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

function score(jobId: string, combined: number): MatchScore {
  return {
    resumeId: RESUME_ID,
    jobId,
    ats: combined - 10,
    semantic: combined + 5,
    combined,
    gaps: [],
    recommendations: [],
  };
}

const noop = () => {};
const formatRelativeTime = () => '';

function renderResults(opts: {
  filtered: Posting[];
  scraping?: boolean;
  resumeId?: string | null;
  scoresById?: Map<string, MatchScore>;
  isPending?: boolean;
  isError?: boolean;
}) {
  stubbedQuery = {
    scoresById: opts.scoresById ?? new Map(),
    isPending: opts.isPending ?? false,
    isError: opts.isError ?? false,
  };
  const resumeId = 'resumeId' in opts ? (opts.resumeId ?? null) : RESUME_ID;
  const jobIds = opts.filtered.map((p) => p.id);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <MatchScoresProvider resumeId={resumeId} jobIds={jobIds}>
      {children}
    </MatchScoresProvider>
  );
  return render(
    <JobsResults
      filtered={opts.filtered}
      formatRelativeTime={formatRelativeTime}
      scraping={opts.scraping ?? false}
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

// ── tests ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  stubbedQuery = { scoresById: new Map(), isPending: false, isError: false };
  stubbedHasByProvider = {
    [PROVIDER_SLOTS.adzunaAppId]: true,
    [PROVIDER_SLOTS.adzunaAppKey]: true,
  };
  stubbedKeyIsSuccess = true;
  mockNavigate.mockClear();
  mockSetSettings.mockClear();
  mockSetJobs.mockClear();
  // Reset store state to list mode / no selection between tests.
  STORE_STATE.jobs = { viewMode: 'list', selectedId: null };
});

describe('JobsResults — gating', () => {
  it('shows the searching state and no rows while scraping', () => {
    renderResults({ filtered: [posting('a', 'A'), posting('b', 'B')], scraping: true });

    expect(screen.getByText('jobs.searching')).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.jobs.postingRow)).not.toBeInTheDocument();
    // "Show more" is hidden while waiting.
    expect(screen.queryByText('jobs.showMore')).not.toBeInTheDocument();
  });

  it('shows the scoring state and no rows while the batch is in-flight with a résumé', () => {
    renderResults({
      filtered: [posting('a', 'A'), posting('b', 'B')],
      isPending: true, // hasResume defaults true → waiting
    });

    expect(screen.getByText('jobs.scoring')).toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.jobs.postingRow)).not.toBeInTheDocument();
  });

  it('reveals results (escape hatch) when scoring errors even while the batch is still pending', () => {
    // isError=true collapses waiting=false regardless of isPending=true,
    // so the gate opens and unscored rows reveal in input order.
    renderResults({
      filtered: [posting('x', 'X'), posting('y', 'Y')],
      isPending: true,
      isError: true,
    });

    expect(screen.queryByText('jobs.scoring')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.searching')).not.toBeInTheDocument();
    expect(rowOrder()).toEqual(['x', 'y']);
  });
});

describe('JobsResults — reveal sort', () => {
  it('reveals rows sorted by combined score descending when settled with a résumé', () => {
    // filtered input order: low, high, mid. Expect reveal order: high, mid, low.
    const filtered = [posting('low', 'Low'), posting('high', 'High'), posting('mid', 'Mid')];
    const scoresById = new Map<string, MatchScore>([
      ['low', score('low', 20)],
      ['high', score('high', 90)],
      ['mid', score('mid', 55)],
    ]);

    renderResults({ filtered, scoresById, isPending: false });

    expect(screen.queryByText('jobs.scoring')).not.toBeInTheDocument();
    expect(rowOrder()).toEqual(['high', 'mid', 'low']);
  });

  it('sinks rows without a score to the bottom', () => {
    const filtered = [posting('scored', 'Scored'), posting('unscored', 'Unscored')];
    const scoresById = new Map<string, MatchScore>([['scored', score('scored', 40)]]);

    renderResults({ filtered, scoresById, isPending: false });

    expect(rowOrder()).toEqual(['scored', 'unscored']);
  });
});

describe('JobsResults — no résumé', () => {
  it('renders rows immediately in input order without waiting on scores', () => {
    const filtered = [posting('a', 'A'), posting('b', 'B'), posting('c', 'C')];

    renderResults({ filtered, resumeId: null });

    expect(screen.queryByText('jobs.scoring')).not.toBeInTheDocument();
    expect(rowOrder()).toEqual(['a', 'b', 'c']);
  });

  it('shows the empty state (not a loading state) when there is no résumé and nothing matches', () => {
    renderResults({ filtered: [], resumeId: null });

    expect(screen.getByText('jobs.empty')).toBeInTheDocument();
    expect(screen.queryByText('jobs.scoring')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.searching')).not.toBeInTheDocument();
    expect(screen.queryByTestId(TEST_IDS.jobs.postingRow)).not.toBeInTheDocument();
  });

  it('shows the Adzuna-keys CTA when the list is empty and keys are missing', () => {
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppId] = false;
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppKey] = false;
    renderResults({ filtered: [], resumeId: null });

    // Secondary message and settings CTA are shown instead of the generic scrape CTA.
    expect(screen.getByText('jobs.emptyNoAdzunaKeys')).toBeInTheDocument();
    expect(screen.getByText('jobs.emptyNoAdzunaKeysCta')).toBeInTheDocument();
    expect(screen.queryByText('jobs.emptyCta')).not.toBeInTheDocument();
  });

  it('shows the generic CTA when the list is empty and keys are present', () => {
    // stubbedHasByProvider defaults to both true in beforeEach — no override needed
    renderResults({ filtered: [], resumeId: null });

    expect(screen.getByText('jobs.emptyCta')).toBeInTheDocument();
    expect(screen.queryByText('jobs.emptyNoAdzunaKeys')).not.toBeInTheDocument();
  });

  it('shows the generic CTA (not the keys CTA) while the key queries are still loading', () => {
    // isSuccess=false means the query has not resolved yet — keys may exist but
    // we don't know.  missingAdzunaKeys must be false so the generic empty-state
    // is shown, preventing a false-positive "add keys" flash.
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppId] = false; // would trigger CTA if isSuccess were true
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppKey] = false;
    stubbedKeyIsSuccess = false; // queries unresolved / loading
    renderResults({ filtered: [], resumeId: null });

    expect(screen.getByText('jobs.emptyCta')).toBeInTheDocument();
    expect(screen.queryByText('jobs.emptyNoAdzunaKeys')).not.toBeInTheDocument();
  });

  it('wraps the empty-state variant swap in a live region so AT users hear the change', () => {
    // The missingAdzunaKeys → keys-present swap replaces the EmptyState in place;
    // a single polite live region around both variants announces the new text.
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppId] = false;
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppKey] = false;
    const { container } = renderResults({ filtered: [], resumeId: null });

    const live = container.querySelector('[role="status"][aria-live="polite"]');
    expect(live).not.toBeNull();
    expect(live).toHaveTextContent('jobs.emptyNoAdzunaKeys');
  });

  it('clicking the missing-keys CTA calls setSettings({activeSection:"job"}) and navigates to /settings', async () => {
    // Arrange: show the Adzuna-keys empty state (empty list, both slots missing, resolved).
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppId] = false;
    stubbedHasByProvider[PROVIDER_SLOTS.adzunaAppKey] = false;
    renderResults({ filtered: [], resumeId: null });

    // The CTA button text is the i18n key (identity t()).
    const cta = screen.getByText('jobs.emptyNoAdzunaKeysCta');
    expect(cta).toBeInTheDocument();

    // Act: click via user-event (fires real pointer + keyboard events).
    await userEvent.click(cta);

    // Assert: session store receives the correct section, then router navigates.
    expect(mockSetSettings).toHaveBeenCalledOnce();
    expect(mockSetSettings).toHaveBeenCalledWith({ activeSection: 'job' });
    expect(mockNavigate).toHaveBeenCalledOnce();
    expect(mockNavigate).toHaveBeenCalledWith({ to: '/settings' });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// Split-mode auto-select
// ─────────────────────────────────────────────────────────────────────────────

describe('JobsResults — split-mode auto-select', () => {
  it('selects display[0] immediately when split mode has results and no current selection', () => {
    STORE_STATE.jobs = { viewMode: 'split', selectedId: null };
    const p1 = posting('a', 'A');
    const p2 = posting('b', 'B');

    renderResults({ filtered: [p1, p2], resumeId: null });

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'a', detailCollapsed: false });
  });

  it('does NOT clobber a valid manual selection on a plain re-render', () => {
    // User already selected 'b', which is still in display — selectionInDisplay=true
    // so the effect is a no-op (no auto-select to 'a').
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
    // Start: split mode, 'b' is selected and present in display.
    STORE_STATE.jobs = { viewMode: 'split', selectedId: 'b' };
    const p1 = posting('a', 'A');
    const p2 = posting('b', 'B');

    const { rerender } = renderResults({ filtered: [p1, p2], resumeId: null });
    mockSetJobs.mockClear();

    // Simulate filtering: 'b' is removed from display, only 'a' remains.
    // selectedId is still 'b' in the store (STORE_STATE hasn't changed), but
    // selectionInDisplay becomes false → effect must re-select 'a'.
    rerender(
      <JobsResults
        filtered={[p1]}
        formatRelativeTime={() => ''}
        scraping={false}
        onShowMore={noop}
        onScrape={noop}
      />
    );

    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'a', detailCollapsed: false });
  });

  it('re-selects display[0] when scraping transitions true→false in split mode', () => {
    STORE_STATE.jobs = { viewMode: 'split', selectedId: null };
    const p1 = posting('x', 'X');
    const p2 = posting('y', 'Y');

    // Initial render: scraping=true → waiting=true, results gated (spinner shown).
    const { rerender } = renderResults({ filtered: [p1, p2], resumeId: null, scraping: true });
    mockSetJobs.mockClear();

    // Transition: scraping false → waiting goes true→false. rerender inherits the
    // original MatchScoresProvider wrapper automatically.
    rerender(
      <JobsResults
        filtered={[p1, p2]}
        formatRelativeTime={() => ''}
        scraping={false}
        onShowMore={noop}
        onScrape={noop}
      />
    );

    // Effect fires on the waiting transition: should auto-select the top result.
    expect(mockSetJobs).toHaveBeenCalledWith({ selectedId: 'x', detailCollapsed: false });
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
