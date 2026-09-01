/**
 * JobsResults — hybrid-search states (idle/searching/results/noResults/
 * stale/error), gated behind `hybridSearch` (optional — omitted callers, and
 * every OTHER JobsResults test, get the `idle` default and are unaffected).
 *
 * Covers the task's three designed situations:
 *   1. idle — untouched, covered by JobsResults.test.tsx.
 *   2. a query with zero hits reads as "no match", never "you haven't scraped".
 *   3. degraded-but-useful: `arms.dense === 'skipped'` surfaces a one-click
 *      enable action; `arms.rerank === 'unavailable'` surfaces plainly.
 * Plus `staleCorpus` and a genuine error, both distinct from each other and
 * from the zero-hit case.
 */
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/dom';
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { TEST_IDS } from '@ajh/test-ids';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    t: (k: string, p?: Record<string, unknown>) =>
      p
        ? `${k}[${Object.entries(p)
            .map(([key, val]) => `${key}=${String(val)}`)
            .join(',')}]`
        : k,
    i18n: { language: 'en' },
  }),
}));

const mockNavigate = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useRouter: () => ({ navigate: mockNavigate }),
}));

const STORE_STATE = {
  setSettings: vi.fn(),
  jobs: { viewMode: 'list' as const, selectedId: null as string | null },
  setJobs: vi.fn(),
};
vi.mock('@/store/session-store', () => ({
  useSessionStore: (sel?: (s: typeof STORE_STATE) => unknown) =>
    sel ? sel(STORE_STATE) : STORE_STATE,
}));

vi.mock('@/services/use-ai-provider', () => ({
  useHasProviderKey: () => ({ data: { has: true }, isSuccess: true }),
}));

vi.mock('@/services', () => ({
  useJobMatchScore: () => ({ data: undefined }),
}));

vi.mock('@/features/jobs/components/PostingRow', () => ({
  PostingRow: ({ posting }: { posting: { id: string; title: string } }) => (
    <div data-testid={TEST_IDS.jobs.postingRow} data-id={posting.id}>
      {posting.title}
    </div>
  ),
}));

vi.mock('@/features/jobs/components/JobsSplitView', () => ({
  JobsSplitView: ({ display }: { display: { id: string }[] }) => (
    <div data-testid="jobs-split-view" data-count={display.length} />
  ),
}));

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 88,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({ key: index, index, start: index * 88 })),
    measureElement: () => {},
  }),
}));

import { MatchScoresProvider } from '@/features/jobs/providers';
import type { Posting } from '@/features/jobs/types';

import { JobsResults } from './index';

function posting(id: string): Posting {
  return {
    id,
    source: 'linkedin',
    externalId: id,
    url: `https://example.com/${id}`,
    title: `Title ${id}`,
    company: 'Acme',
    description: '',
    capturedAt: 0,
  };
}

const noop = () => {};

function renderResults(
  filtered: Posting[],
  hybridSearch: Parameters<typeof JobsResults>[0]['hybridSearch']
) {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <MatchScoresProvider resumeId={null}>{children}</MatchScoresProvider>
  );
  return render(
    <JobsResults
      filtered={filtered}
      formatRelativeTime={() => ''}
      scraping={false}
      onShowMore={noop}
      onScrape={noop}
      hybridSearch={hybridSearch}
    />,
    { wrapper }
  );
}

beforeEach(() => {
  STORE_STATE.jobs = { viewMode: 'list', selectedId: null };
  vi.clearAllMocks();
});

describe('JobsResults — hybrid search: searching', () => {
  it('shows a loading state instead of the scrape-empty CTA', () => {
    renderResults([], {
      state: 'searching',
      arms: null,
      corpusSize: 0,
      onRetry: noop,
      onClear: noop,
      onEnableSemanticRanking: noop,
    });
    expect(screen.getByText('jobs.hybridSearch.searching')).toBeInTheDocument();
    expect(screen.queryByText('jobs.emptyCta')).not.toBeInTheDocument();
  });
});

describe('JobsResults — hybrid search: zero hits', () => {
  it('reads as "no match", never as "nothing scraped"', () => {
    renderResults([], {
      state: 'noResults',
      arms: { lexical: 'ran', dense: 'skipped', rerank: 'skipped' },
      corpusSize: 42,
      onRetry: noop,
      onClear: noop,
      onEnableSemanticRanking: noop,
    });
    expect(screen.getByText('jobs.hybridSearch.noResultsTitle')).toBeInTheDocument();
    expect(screen.getByText('jobs.hybridSearch.noResultsDesc[count=42]')).toBeInTheDocument();
    // Distinct from the genuinely-nothing-scraped copy.
    expect(screen.queryByText('jobs.empty')).not.toBeInTheDocument();
    expect(screen.queryByText('jobs.emptyCta')).not.toBeInTheDocument();
  });

  it('"Clear search" reverts to browsing', async () => {
    const user = userEvent.setup();
    const onClear = vi.fn();
    renderResults([], {
      state: 'noResults',
      arms: { lexical: 'ran', dense: 'ran', rerank: 'ran' },
      corpusSize: 3,
      onRetry: noop,
      onClear,
      onEnableSemanticRanking: noop,
    });
    await user.click(screen.getByRole('button', { name: 'jobs.hybridSearch.clearSearch' }));
    expect(onClear).toHaveBeenCalledTimes(1);
  });

  it('a "results" outcome that re-filters to zero (hideAgency changed since settling) reads the same as noResults', () => {
    renderResults([], {
      state: 'results',
      arms: { lexical: 'ran', dense: 'ran', rerank: 'ran' },
      corpusSize: 3,
      onRetry: noop,
      onClear: noop,
      onEnableSemanticRanking: noop,
    });
    expect(screen.getByText('jobs.hybridSearch.noResultsTitle')).toBeInTheDocument();
  });
});

describe('JobsResults — hybrid search: staleCorpus vs a genuine error', () => {
  it('staleCorpus gets its own copy + a retry action', async () => {
    const user = userEvent.setup();
    const onRetry = vi.fn();
    renderResults([], {
      state: 'stale',
      arms: { lexical: 'ran', dense: 'skipped', rerank: 'skipped' },
      corpusSize: 0,
      onRetry,
      onClear: noop,
      onEnableSemanticRanking: noop,
    });
    expect(screen.getByText('jobs.hybridSearch.staleTitle')).toBeInTheDocument();
    expect(screen.queryByText('jobs.hybridSearch.errorTitle')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Try again' }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('a genuine error gets its own copy, distinct from staleCorpus', () => {
    renderResults([], {
      state: 'error',
      arms: null,
      corpusSize: 0,
      onRetry: noop,
      onClear: noop,
      onEnableSemanticRanking: noop,
    });
    expect(screen.getByText('jobs.hybridSearch.errorTitle')).toBeInTheDocument();
    expect(screen.queryByText('jobs.hybridSearch.staleTitle')).not.toBeInTheDocument();
  });
});

describe('JobsResults — hybrid search: the "ranked by" banner', () => {
  it('surfaces which arms ran, above the ranked list', () => {
    renderResults([posting('a')], {
      state: 'results',
      arms: { lexical: 'ran', dense: 'ran', rerank: 'ran' },
      corpusSize: 12,
      onRetry: noop,
      onClear: noop,
      onEnableSemanticRanking: noop,
    });
    const banner = screen.getByTestId(TEST_IDS.jobs.searchBanner);
    expect(banner).toHaveTextContent('jobs.hybridSearch.armLexical');
    expect(banner).toHaveTextContent('jobs.hybridSearch.armDense');
    expect(banner).toHaveTextContent('jobs.hybridSearch.armRerank');
  });

  it('gives a one-click path to enable semantic ranking when dense was skipped', async () => {
    const user = userEvent.setup();
    const onEnableSemanticRanking = vi.fn();
    renderResults([posting('a')], {
      state: 'results',
      arms: { lexical: 'ran', dense: 'skipped', rerank: 'ran' },
      corpusSize: 12,
      onRetry: noop,
      onClear: noop,
      onEnableSemanticRanking,
    });
    const banner = screen.getByTestId(TEST_IDS.jobs.searchBanner);
    expect(banner).toHaveTextContent('jobs.hybridSearch.semanticOff');

    await user.click(
      screen.getByRole('button', { name: 'jobs.hybridSearch.enableSemanticRanking' })
    );
    expect(onEnableSemanticRanking).toHaveBeenCalledTimes(1);
  });

  it('never claims semantic ranking ran when it did not (never presents keyword-only as hybrid)', () => {
    renderResults([posting('a')], {
      state: 'results',
      arms: { lexical: 'ran', dense: 'skipped', rerank: 'skipped' },
      corpusSize: 5,
      onRetry: noop,
      onClear: noop,
      onEnableSemanticRanking: noop,
    });
    const banner = screen.getByTestId(TEST_IDS.jobs.searchBanner);
    expect(banner).not.toHaveTextContent('jobs.hybridSearch.armDense');
  });

  it('surfaces rerank unavailable plainly, with no dead-end action', () => {
    renderResults([posting('a')], {
      state: 'results',
      arms: { lexical: 'ran', dense: 'ran', rerank: 'unavailable' },
      corpusSize: 5,
      onRetry: noop,
      onClear: noop,
      onEnableSemanticRanking: noop,
    });
    const banner = screen.getByTestId(TEST_IDS.jobs.searchBanner);
    expect(banner).toHaveTextContent('jobs.hybridSearch.rerankUnavailable');
  });

  it('sits OUTSIDE the virtualized scroll container, not as its first child', () => {
    // Regression guard: the virtualizer computes row offsets against the
    // scroller's own content box with no `scrollMargin` configured. A banner
    // rendered INSIDE that scroller (as a normal-flow first child) shifts
    // every row down by the banner's height without the virtualizer knowing,
    // leaving a gap at the true end of the list once scrolled to the bottom.
    renderResults([posting('a')], {
      state: 'results',
      arms: { lexical: 'ran', dense: 'ran', rerank: 'ran' },
      corpusSize: 1,
      onRetry: noop,
      onClear: noop,
      onEnableSemanticRanking: noop,
    });
    const banner = screen.getByTestId(TEST_IDS.jobs.searchBanner);
    const scroller = document.querySelector('.overflow-y-auto');
    expect(scroller).not.toBeNull();
    expect(scroller?.contains(banner)).toBe(false);
  });
});
