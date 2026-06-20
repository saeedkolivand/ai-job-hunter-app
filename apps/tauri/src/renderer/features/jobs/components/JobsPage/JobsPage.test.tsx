/**
 * JobsPage — partial-failure scrape summary.
 *
 * Covers the job.completed event handler (lines 108-126 of JobsPage/index.tsx):
 *   - A completed event with failed boards produces ok:true (not ok:false)
 *   - The partial note uses display names via t('jobs.boards.<id>'), not raw ids
 *   - The note follows the "N of M · <names> failed" format
 *   - A completed event with no failed boards produces no note (ok:true, note undefined)
 *   - All boards failing still produces ok:true (the event type is 'job.completed')
 *   - job.failed event → ok:false
 *
 * All heavy dependencies are module-mocked. useJobEvents is intercepted via a
 * shared-object container (avoids the vi.mock factory scope isolation constraint)
 * so we can fire synthetic events directly in each test.
 */
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, waitFor } from '@testing-library/react';

// ---------------------------------------------------------------------------
// Shared containers — these are objects so vi.mock factories can mutate their
// properties even though factories run in an isolated scope.
// ---------------------------------------------------------------------------

// jobEventHandler: set by the useJobEvents mock when the component calls it.
const jobEvents = { handler: null as ((event: unknown) => void) | null };

// noteScrapeFinished spy — replaced per-test via .mockImplementation.
const scrapingMock = {
  noteScrapeFinished: vi.fn<(jobId: string, outcome: { ok: boolean; note?: string }) => void>(),
};

// ---------------------------------------------------------------------------
// Module mocks (hoisted; factories run lazily but MUST NOT close over test-file
// let/const — use the shared object containers declared above instead)
// ---------------------------------------------------------------------------

vi.mock('@/features/jobs/hooks/useScraping', () => ({
  useScraping: () => ({
    scraping: false,
    scrapeOutcome: null,
    livePostings: [],
    setLivePostings: vi.fn(),
    scrapeJobRef: { current: 'job-123' },
    replacePendingRef: { current: false },
    startScrape: vi.fn(),
    cancelScrape: vi.fn(),
    noteScrapeFinished: (...args: [string, { ok: boolean; note?: string }]) =>
      scrapingMock.noteScrapeFinished(...args),
  }),
}));

vi.mock('@/services', () => ({
  usePostings: () => ({ data: [] }),
  useClearPostings: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useInvalidatePostings: () => vi.fn(),
  useJobPreferences: () => ({ data: undefined }),
  useGeocodeSuggest: () => vi.fn().mockResolvedValue([]),
  useJobEvents: (cb: (event: unknown) => void) => {
    jobEvents.handler = cb;
  },
  useJobMatchScores: () => ({ scoresById: new Map(), isPending: false, isError: false }),
}));

vi.mock('@/features/jobs/hooks/useDefaultResumeId', () => ({
  useDefaultResumeId: () => null,
}));

vi.mock('@/store/session-store', () => ({
  useSessionStore: () => ({
    jobs: { filter: '', sortBy: 'newest' },
    setJobs: vi.fn(),
  }),
}));

vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => (ts: number) => String(ts),
}));

vi.mock('@/components/layout/PageHeader', () => ({
  PageHeader: ({ title }: { title: string }) => <div data-testid="page-header">{title}</div>,
}));

vi.mock('@/components/layout/PageTransition', () => ({
  PageTransition: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock('@/features/jobs/components/JobsResults', () => ({
  JobsResults: () => <div data-testid="jobs-results" />,
}));

vi.mock('@/features/jobs/components/ScrapeForm', () => ({
  ScrapeForm: () => <div data-testid="scrape-form" />,
}));

vi.mock('@/features/jobs/providers', () => ({
  MatchScoresProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
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
  Button: ({ children, onClick }: { children: ReactNode; onClick?: () => void }) => (
    <div role="button" onClick={onClick}>
      {children}
    </div>
  ),
  ConfirmModal: () => null,
  Dropdown: () => null,
  Input: () => null,
  useNotification: () => ({ error: vi.fn(), success: vi.fn(), info: vi.fn() }),
}));

// Import AFTER mocks
import { JobsPage } from './index';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderJobsPage() {
  jobEvents.handler = null;
  return render(<JobsPage />);
}

function fireJobEvent(event: unknown) {
  act(() => {
    jobEvents.handler?.(event);
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('JobsPage — job.completed event handler', () => {
  beforeEach(() => {
    scrapingMock.noteScrapeFinished.mockClear();
  });

  it('registers a job events listener on mount', () => {
    renderJobsPage();
    expect(jobEvents.handler).toBeTypeOf('function');
  });

  it('completed event with no failed boards → ok:true, no note', async () => {
    renderJobsPage();
    expect(jobEvents.handler).toBeTypeOf('function');

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [
          { board: 'linkedin', count: 10 },
          { board: 'indeed', count: 5 },
        ],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    const [, outcome] = scrapingMock.noteScrapeFinished.mock.calls[0]!;
    expect(outcome.ok).toBe(true);
    expect(outcome.note).toBeUndefined();
  });

  it('completed event with one failed board → ok:true (partial failure keeps ok)', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [
          { board: 'linkedin', count: 10 },
          { board: 'indeed', count: 0, error: 'rate limited' },
        ],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    const [, outcome] = scrapingMock.noteScrapeFinished.mock.calls[0]!;
    expect(outcome.ok).toBe(true);
    expect(outcome.note).toBeDefined();
  });

  it('partial note uses translated display names via t("jobs.boards.<id>"), not raw ids', async () => {
    // t() mock format: "key[param=value,...]" so we can see exactly what was passed.
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [
          { board: 'linkedin', count: 5 },
          { board: 'indeed', count: 0, error: 'blocked' },
        ],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    const [, outcome] = scrapingMock.noteScrapeFinished.mock.calls[0]!;
    const note = outcome.note ?? '';

    // The note is the result of t('jobs.partialScrapeNote', { done, total, failed }).
    // Our mock returns "jobs.partialScrapeNote[done=1,total=2,failed=<failedNames>]".
    // <failedNames> = t('jobs.boards.indeed') = 'jobs.boards.indeed' (not raw 'indeed').
    expect(note).toContain('jobs.partialScrapeNote');
    // The failed param must contain the translated key 'jobs.boards.indeed', not the raw id
    expect(note).toContain('jobs.boards.indeed');
    // Raw board id alone must not appear as the label
    expect(note).not.toMatch(/failed=indeed[,\]]/);
  });

  it('partial note format — N of M counts are correct', async () => {
    // Two boards, one fails → done=1, total=2
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [
          { board: 'greenhouse', count: 8 },
          { board: 'xing', count: 0, error: 'login required' },
        ],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    const [, outcome] = scrapingMock.noteScrapeFinished.mock.calls[0]!;
    const note = outcome.note ?? '';

    // t() mock → "jobs.partialScrapeNote[done=1,total=2,failed=jobs.boards.xing]"
    expect(note).toContain('done=1'); // 1 board succeeded
    expect(note).toContain('total=2'); // 2 total boards
    expect(note).toContain('jobs.boards.xing'); // display name for the failed board
  });

  it('all boards failing → still ok:true (job.completed event type)', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [
          { board: 'linkedin', count: 0, error: 'blocked' },
          { board: 'indeed', count: 0, error: 'rate limited' },
        ],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    const [, outcome] = scrapingMock.noteScrapeFinished.mock.calls[0]!;
    expect(outcome.ok).toBe(true);
  });

  it('job.failed event → ok:false with the error data as note', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.failed',
      jobId: 'job-123',
      data: 'connection refused',
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    const [, outcome] = scrapingMock.noteScrapeFinished.mock.calls[0]!;
    expect(outcome.ok).toBe(false);
    expect(outcome.note).toBe('connection refused');
  });
});
