/**
 * Harness for the JobsPage stream suites (dedup, invalidation throttle).
 *
 * Live postings are driven through the `useScraping` mock (a mutable container
 * read inside the factory); `job.stream` events are fired through the shared
 * `jobEvents` container. The stubbed JobsResults renders the merged `filtered`
 * list so suites assert on what the component actually produced.
 *
 * The session store is REAL — the page reads the active job id and the replace
 * latch back from it synchronously (`getState()`), which a plain object stub
 * cannot model. `resetStream()` seeds it per test.
 */
import type { ReactNode } from 'react';
import { vi } from 'vitest';
import { act, render, type RenderResult } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

import type { Posting } from '@/features/jobs/types';
import { makeJobsDefaults, useSessionStore } from '@/store/session-store';

import { JobsPage } from './index';
import { jobEvents } from './job-events';

/** useInvalidatePostings returns this spy; tests assert on call count. */
export const invalidateSpy = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);

/**
 * useScraping return value — mutate `livePostings` before rendering to seed the
 * component state. The replace latch lives in the session store
 * (`jobs.replacePending`), so tests set it there to exercise the
 * eager-invalidation branch in the job.stream handler.
 */
export const scrapingState = {
  livePostings: [] as Posting[],
  setLivePostings: vi.fn<(updater: Posting[] | ((prev: Posting[]) => Posting[])) => void>(),
};

/** The merged+filtered `filtered` prop JobsResults last received. */
export const lastFiltered: { value: Posting[] } = { value: [] };

vi.mock('@/features/jobs/hooks/usePostingsSearch', () => ({
  usePostingsSearch: () => ({
    state: 'idle',
    result: null,
    committedQuery: '',
    search: vi.fn(),
    retry: vi.fn(),
    clear: vi.fn(),
    enableSemanticRanking: vi.fn(),
  }),
}));

vi.mock('@/hooks/useDefaultResumeId', () => ({ useDefaultResumeId: () => null }));

vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => (ts: number) => String(ts),
}));

vi.mock('@/components/layout/PageTransition', () => ({
  PageTransition: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock('@/features/jobs/providers', () => ({
  MatchScoresProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock('@/features/jobs/hooks/useScraping', () => ({
  useScraping: () => ({
    scraping: false,
    scrapeOutcome: null,
    livePostings: scrapingState.livePostings,
    setLivePostings: scrapingState.setLivePostings,
    startScrape: vi.fn(),
    cancelScrape: vi.fn(),
    noteScrapeFinished: vi.fn(),
  }),
}));

vi.mock('@/services', () => ({
  usePostings: () => ({ data: [] }),
  useClearPostings: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useInvalidatePostings: () => invalidateSpy,
  useJobPreferences: () => ({ data: undefined }),
  useGeocodeSuggest: () => vi.fn().mockResolvedValue([]),
  useJobEvents: (cb: (event: unknown) => void) => {
    jobEvents.handler = cb;
  },
}));

vi.mock('@/features/jobs/components/JobsResults', () => ({
  JobsResults: ({ filtered }: { filtered: Posting[] }) => {
    lastFiltered.value = filtered;
    return (
      <ul data-testid={TEST_IDS.jobs.jobsResults}>
        {filtered.map((p) => (
          <li key={p.id} data-testid={TEST_IDS.jobs.postingRow} data-id={p.id}>
            {p.title}
          </li>
        ))}
      </ul>
    );
  },
}));

vi.mock('@/features/jobs/components/ScrapeForm', () => ({
  ScrapeForm: () => <div data-testid={TEST_IDS.jobs.scrapeForm} />,
}));

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock('@ajh/ui', () => ({
  Button: ({ children, onClick }: { children: ReactNode; onClick?: () => void }) => (
    <div role="button" onClick={onClick}>
      {children}
    </div>
  ),
  ConfirmModal: () => null,
  Drawer: ({ open, children }: { open: boolean; children: ReactNode }) =>
    open ? <div role="dialog">{children}</div> : null,
  Dropdown: () => null,
  Input: () => null,
  SegmentedControl: () => null,
  Tag: Object.assign(({ children }: { children: ReactNode }) => <span>{children}</span>, {
    CheckableTag: ({ children }: { children: ReactNode }) => <span>{children}</span>,
  }),
  useNotification: () => ({ error: vi.fn(), success: vi.fn(), info: vi.fn(), warning: vi.fn() }),
}));

/**
 * Call from `beforeEach`. `scrapeJobId: 'job-abc'` matches the jobId
 * `fireStreamEvent()` emits, so the page's active-job guard accepts the
 * synthetic stream items.
 */
export function resetStream(replacePending = false) {
  useSessionStore.setState({
    jobs: { ...makeJobsDefaults(), viewMode: 'list', scrapeJobId: 'job-abc', replacePending },
  });
  scrapingState.livePostings = [];
  invalidateSpy.mockClear();
}

export function fireStreamEvent(item: Posting, jobId = 'job-abc') {
  act(() => {
    jobEvents.handler?.({
      type: 'job.stream',
      jobId,
      data: item,
      ts: Date.now(),
    });
  });
}

/** Row ids in DOM order as rendered by the stubbed JobsResults. */
export function renderedIds(): string[] {
  return Array.from(document.querySelectorAll(`[data-testid="${TEST_IDS.jobs.postingRow}"]`)).map(
    (el) => el.getAttribute('data-id') ?? ''
  );
}

export function renderJobsPage() {
  jobEvents.handler = null;
  return render(<JobsPage />);
}

/** Re-render the page (e.g. after `scrapingState.livePostings` changed). */
export function rerenderJobsPage(view: RenderResult) {
  view.rerender(<JobsPage />);
}
