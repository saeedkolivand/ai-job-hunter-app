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

// ── session-store stub — setSettings is a captured spy ───────────────────────

const mockSetSettings = vi.fn();
vi.mock('@/store/session-store', () => ({
  useSessionStore: (sel: (s: { setSettings: typeof mockSetSettings }) => unknown) =>
    sel({ setSettings: mockSetSettings }),
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
