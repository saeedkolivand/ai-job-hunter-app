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

import type { MatchScore } from '@ajh/shared';

// ── i18n stub — identity t() so we assert on keys ─────────────────────────────

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

// ── useJobMatchScores stub — module-level ref set BEFORE each render ───────────

let stubbedQuery: { scoresById: Map<string, MatchScore>; isPending: boolean } = {
  scoresById: new Map(),
  isPending: false,
};

vi.mock('@/services', () => ({
  useJobMatchScores: () => stubbedQuery,
}));

// ── PostingRow stub — strip router/services; expose title + id for ordering ────

vi.mock('@/features/jobs/components/PostingRow', () => ({
  PostingRow: ({ posting }: { posting: { id: string; title: string } }) => (
    <div data-testid="posting-row" data-id={posting.id}>
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
}) {
  stubbedQuery = {
    scoresById: opts.scoresById ?? new Map(),
    isPending: opts.isPending ?? false,
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
  return Array.from(document.querySelectorAll('[data-testid="posting-row"]')).map(
    (el) => el.getAttribute('data-id') ?? ''
  );
}

// ── tests ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  stubbedQuery = { scoresById: new Map(), isPending: false };
});

describe('JobsResults — gating', () => {
  it('shows the searching state and no rows while scraping', () => {
    renderResults({ filtered: [posting('a', 'A'), posting('b', 'B')], scraping: true });

    expect(screen.getByText('jobs.searching')).toBeInTheDocument();
    expect(screen.queryByTestId('posting-row')).not.toBeInTheDocument();
    // "Show more" is hidden while waiting.
    expect(screen.queryByText('jobs.showMore')).not.toBeInTheDocument();
  });

  it('shows the scoring state and no rows while the batch is in-flight with a résumé', () => {
    renderResults({
      filtered: [posting('a', 'A'), posting('b', 'B')],
      isPending: true, // hasResume defaults true → waiting
    });

    expect(screen.getByText('jobs.scoring')).toBeInTheDocument();
    expect(screen.queryByTestId('posting-row')).not.toBeInTheDocument();
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
    expect(screen.queryByTestId('posting-row')).not.toBeInTheDocument();
  });
});
