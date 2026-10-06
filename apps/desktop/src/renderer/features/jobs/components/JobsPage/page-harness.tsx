/**
 * Stubbed-children harness for the JobsPage event / diagnostics / view suites.
 *
 * All heavy dependencies are module-mocked. `useJobEvents` is intercepted via
 * `jobEvents` (see test-support) so a suite fires synthetic events directly.
 * Mutable containers are plain objects so `vi.mock` factories (which run in an
 * isolated scope) can mutate them.
 *
 * The session store is NOT mocked: it owns the scrape bookkeeping the page reads
 * back synchronously (`useSessionStore.getState()`), so a hand-rolled stub would
 * have to re-implement zustand to stay honest. `resetPage()` seeds it per test.
 */
import type { ReactNode, Ref } from 'react';
import { type Mock, vi } from 'vitest';
import { render } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

import type { Posting } from '@/features/jobs/types';
import { makeJobsDefaults, useSessionStore } from '@/store/session-store';

import { makePosting } from './fixtures';
import { JobsPage } from './index';
import { jobEvents } from './job-events';

// noteScrapeFinished spy — replaced per-test via .mockImplementation.
export const scrapingMock = {
  noteScrapeFinished: vi.fn<(jobId: string, outcome: { ok: boolean; note?: string }) => void>(),
};

export const notifyMock: Record<'error' | 'success' | 'info' | 'warning', Mock> = {
  error: vi.fn(),
  success: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
};

// BoardSummaryChips capture — records the `summaries` prop each time the strip
// renders, so tests can assert the page retained + forwarded the per-board data
// (the strip replaced the old transient skip-toasts).
export const boardChips = { summaries: null as unknown };

// JobsResults prop capture — asserts the same per-board summaries + failure
// note reach the empty-state wiring, not just the header strip.
export const resultsProps = {
  boardSummaries: undefined as unknown,
  failureNote: undefined as unknown,
  totalCount: undefined as unknown,
  filtered: undefined as unknown,
  // The empty state's "Search jobs" CTA — must open the same scrape drawer as
  // the command bar's primary action.
  onScrape: undefined as unknown,
};

// ScrapeForm prop capture — the drawer hands the form its dismiss callback
// (`onToggle`) and its submit (`onStart`); either must close the drawer.
export const scrapeFormContainer = {
  onToggle: null as (() => void) | null,
  onStart: null as (() => void) | null,
};

// Drawer prop capture — asserts the focus-return fallback the page wires in.
export const drawerContainer = {
  returnFocusTo: null as { current: HTMLElement | null } | null,
};

// usePostings — mutable container so tests can simulate "results present" vs
// "zero results" for the header-strip mutual-exclusivity gating (the header
// strip/note only render alongside a non-empty results list; the empty state
// owns the zero-results explanation). Defaults to empty; individual tests set
// it explicitly so the scenario is never implicit.
export const postingsContainer: { data: Posting[] } = { data: [] };

export const segmentedControlContainer = {
  onChange: null as ((v: string) => void) | null,
};

export const samplePosting = (id: string) => makePosting(id, { title: 'Engineer' });

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
    livePostings: [],
    setLivePostings: vi.fn(),
    startScrape: vi.fn(),
    cancelScrape: vi.fn(),
    noteScrapeFinished: (...args: [string, { ok: boolean; note?: string }]) =>
      scrapingMock.noteScrapeFinished(...args),
  }),
}));

vi.mock('@/services', () => ({
  usePostings: () => postingsContainer,
  useClearPostings: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useInvalidatePostings: () => vi.fn(),
  useJobPreferences: () => ({ data: undefined }),
  useGeocodeSuggest: () => vi.fn().mockResolvedValue([]),
  useJobEvents: (cb: (event: unknown) => void) => {
    jobEvents.handler = cb;
  },
}));

vi.mock('@/features/jobs/components/JobsResults', () => ({
  JobsResults: ({
    boardSummaries,
    failureNote,
    totalCount,
    filtered,
    onScrape,
  }: {
    boardSummaries?: unknown;
    failureNote?: unknown;
    totalCount?: unknown;
    filtered?: unknown;
    onScrape?: unknown;
  }) => {
    // Records the summaries + failure note + unfiltered count forwarded into
    // the empty-state wiring, plus the sorted `filtered` list so the stable
    // sort (PR H) is assertable end-to-end.
    resultsProps.boardSummaries = boardSummaries;
    resultsProps.failureNote = failureNote;
    resultsProps.totalCount = totalCount;
    resultsProps.filtered = filtered;
    resultsProps.onScrape = onScrape;
    return <div data-testid={TEST_IDS.jobs.jobsResults} />;
  },
}));

vi.mock('@/components/scrape/BoardSummaryChips', () => ({
  BoardSummaryChips: ({ summaries }: { summaries: unknown }) => {
    boardChips.summaries = summaries;
    return <div data-testid="board-summary-chips" />;
  },
  // Readable fake so tests can assert JobsPage forwards the sanitized value
  // (not the raw error) without depending on the real redaction internals.
  sanitizeReason: (raw: string) => `sanitized:${raw}`,
}));

vi.mock('@/features/jobs/components/ScrapeForm', () => ({
  ScrapeForm: ({ onToggle, onStart }: { onToggle?: () => void; onStart?: () => void }) => {
    scrapeFormContainer.onToggle = onToggle ?? null;
    scrapeFormContainer.onStart = onStart ?? null;
    return <div data-testid={TEST_IDS.jobs.scrapeForm} />;
  },
}));

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({
    // t() renders "key[param=value,...]" so tests can see both the key and all params.
    t: (k: string, p?: Record<string, unknown>) => {
      if (!p) return k;
      const params = Object.entries(p)
        .map(([key, val]) => `${key}=${String(val)}`)
        .join(',');
      return `${k}[${params}]`;
    },
  }),
}));

vi.mock('@ajh/ui', () => ({
  // Ref-forwarding stub: the drawer's focus-return fallback hangs off the
  // Scrape button's `ref`.
  Button: ({
    children,
    onClick,
    ref,
  }: {
    children: ReactNode;
    onClick?: () => void;
    ref?: Ref<HTMLDivElement>;
  }) => (
    <div role="button" ref={ref} onClick={onClick}>
      {children}
    </div>
  ),
  ConfirmModal: () => null,
  Drawer: ({
    open,
    children,
    returnFocusTo,
  }: {
    open: boolean;
    children: ReactNode;
    returnFocusTo?: { current: HTMLElement | null };
  }) => {
    drawerContainer.returnFocusTo = returnFocusTo ?? null;
    return open ? <div role="dialog">{children}</div> : null;
  },
  Dropdown: () => null,
  Input: () => null,
  SegmentedControl: ({ onChange }: { onChange?: (v: string) => void }) => {
    segmentedControlContainer.onChange = onChange ?? null;
    return null;
  },
  Tag: Object.assign(({ children }: { children: ReactNode }) => <span>{children}</span>, {
    CheckableTag: ({ children }: { children: ReactNode }) => <span>{children}</span>,
  }),
  useNotification: () => notifyMock,
}));

/**
 * Call from `beforeEach`: seed the (real, module-scoped) jobs slice and clear
 * every capture. `scrapeJobId: 'job-123'` stands in for an in-flight scrape —
 * the page's active-job guard compares event ids against it.
 */
export function resetPage() {
  useSessionStore.setState({
    jobs: { ...makeJobsDefaults(), viewMode: 'list', scrapeJobId: 'job-123' },
  });
  scrapingMock.noteScrapeFinished.mockClear();
  for (const spy of Object.values(notifyMock)) spy.mockClear();
  boardChips.summaries = null;
  resultsProps.boardSummaries = undefined;
  resultsProps.failureNote = undefined;
  resultsProps.totalCount = undefined;
  resultsProps.filtered = undefined;
  segmentedControlContainer.onChange = null;
  postingsContainer.data = [];
}

export function renderJobsPage() {
  jobEvents.handler = null;
  return render(<JobsPage />);
}
