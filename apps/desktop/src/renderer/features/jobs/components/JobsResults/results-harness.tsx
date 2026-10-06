/**
 * Harness for the JobsResults suites.
 *
 * Scores are on-demand (fetched when the user opens a job), so only `scraping`
 * gates the list and rows render in the `filtered` input order. The suites drive
 * the REAL MatchScoresProvider with that model; everything around it is stubbed.
 * The `vi.mock` calls here apply to the suites that import this module.
 */
import type { ReactNode } from 'react';
import { type Mock, vi } from 'vitest';
import { render } from '@testing-library/react';

import { type BoardScrapeSummary, PROVIDER_SLOTS } from '@ajh/shared';
import { TEST_IDS } from '@ajh/test-ids';

import { MatchScoresProvider } from '@/features/jobs/providers';
import type { Posting } from '@/features/jobs/types';

import { JobsResults } from './index';

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k, i18n: { language: 'en' } }),
}));

export const mockNavigate: Mock = vi.fn();
vi.mock('@tanstack/react-router', () => ({
  useRouter: () => ({ navigate: mockNavigate }),
}));

export const mockSetSettings: Mock = vi.fn();
export const mockSetJobs: Mock = vi.fn();

export const STORE_STATE: {
  setSettings: Mock;
  jobs: { viewMode: string; selectedId: string | null };
  setJobs: Mock;
} = {
  setSettings: mockSetSettings,
  jobs: { viewMode: 'list', selectedId: null },
  setJobs: mockSetJobs,
};

vi.mock('@/store/session-store', () => ({
  useSessionStore: (sel?: (s: typeof STORE_STATE) => unknown) =>
    sel ? sel(STORE_STATE) : STORE_STATE,
}));

/** useHasProviderKey stub state — Adzuna keys present and resolved by default. */
export const providerKeys = {
  has: {} as Record<string, boolean>,
  isSuccess: true,
};

/** Both Adzuna keys absent. */
export function withoutAdzunaKeys() {
  providerKeys.has[PROVIDER_SLOTS.adzunaAppId] = false;
  providerKeys.has[PROVIDER_SLOTS.adzunaAppKey] = false;
}

vi.mock('@/services/use-ai-provider', () => ({
  useHasProviderKey: (provider: string, enabled = true) => ({
    data: enabled ? { has: providerKeys.has[provider] ?? false } : undefined,
    isSuccess: enabled ? providerKeys.isSuccess : false,
  }),
}));

// MatchScoresProvider dependency — provider calls useJobMatchScore per row.
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
      Array.from({ length: count }, (_, index) => ({
        key: index,
        index,
        start: index * 88,
      })),
    measureElement: () => {},
  }),
}));

export function posting(id: string, title: string): Posting {
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

interface ResultsOpts {
  filtered: Posting[];
  scraping?: boolean;
  scrapeProgress?: number | null;
  resumeId?: string | null;
  boardSummaries?: BoardScrapeSummary[];
  failureNote?: string | null;
  totalCount?: number;
}

const RESUME_ID = 'resume-xyz';

export function renderResults(opts: ResultsOpts) {
  const resumeId = 'resumeId' in opts ? (opts.resumeId ?? null) : RESUME_ID;
  const wrapper = ({ children }: { children: ReactNode }) => (
    <MatchScoresProvider resumeId={resumeId}>{children}</MatchScoresProvider>
  );
  return render(
    <JobsResults
      filtered={opts.filtered}
      formatRelativeTime={formatRelativeTime}
      scraping={opts.scraping ?? false}
      scrapeProgress={opts.scrapeProgress}
      boardSummaries={opts.boardSummaries}
      failureNote={opts.failureNote}
      totalCount={opts.totalCount}
      onShowMore={noop}
      onScrape={noop}
    />,
    { wrapper }
  );
}

/** Re-render the same tree with a new `filtered` / `scraping`. */
export function rerenderResults(
  view: ReturnType<typeof renderResults>,
  filtered: Posting[],
  scraping: boolean
) {
  view.rerender(
    <JobsResults
      filtered={filtered}
      formatRelativeTime={formatRelativeTime}
      scraping={scraping}
      onShowMore={noop}
      onScrape={noop}
    />
  );
}

export function rowOrder(): string[] {
  return Array.from(document.querySelectorAll(`[data-testid="${TEST_IDS.jobs.postingRow}"]`)).map(
    (el) => el.getAttribute('data-id') ?? ''
  );
}

/** Call from `beforeEach`. */
export function resetResults() {
  providerKeys.has = {
    [PROVIDER_SLOTS.adzunaAppId]: true,
    [PROVIDER_SLOTS.adzunaAppKey]: true,
  };
  providerKeys.isSuccess = true;
  mockNavigate.mockClear();
  mockSetSettings.mockClear();
  mockSetJobs.mockClear();
  STORE_STATE.jobs = { viewMode: 'list', selectedId: null };
}
