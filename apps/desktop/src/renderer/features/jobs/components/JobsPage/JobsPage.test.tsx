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
import { act, render, screen, waitFor } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

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

// Notification spies — shared container so vi.mock factory closure can reference them.
const notifyMock = {
  error: vi.fn(),
  success: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
};

// setJobs spy — lifted so SegmentedControl onChange tests can assert call args.
const setJobsSpy = vi.fn();

// BoardSummaryChips capture — records the `summaries` prop each time the strip
// renders, so tests can assert the page retained + forwarded the per-board data
// (the strip replaced the old transient skip-toasts).
const boardChips = { summaries: null as unknown };

// JobsResults prop capture — asserts the same per-board summaries + failure
// note reach the empty-state wiring, not just the header strip.
const resultsProps = { boardSummaries: undefined as unknown, failureNote: undefined as unknown };

// SegmentedControl onChange container — set by the stub when the component mounts.
const segmentedControlContainer = {
  onChange: null as ((v: string) => void) | null,
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
}));

vi.mock('@/hooks/useDefaultResumeId', () => ({
  useDefaultResumeId: () => null,
}));

vi.mock('@/store/session-store', () => ({
  useSessionStore: () => ({
    jobs: { filter: '', sortBy: 'newest', viewMode: 'list' },
    setJobs: (...args: unknown[]) => setJobsSpy(...args),
    setSettings: vi.fn(),
  }),
}));

vi.mock('@/hooks/use-format-relative-time', () => ({
  useFormatRelativeTime: () => (ts: number) => String(ts),
}));

vi.mock('@/components/layout/PageHeader', () => ({
  PageHeader: ({ title, actions }: { title: string; actions?: ReactNode }) => (
    <div data-testid={TEST_IDS.layout.pageHeader}>
      {title}
      {actions}
    </div>
  ),
}));

vi.mock('@/components/layout/PageTransition', () => ({
  PageTransition: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

vi.mock('@/features/jobs/components/JobsResults', () => ({
  JobsResults: ({
    boardSummaries,
    failureNote,
  }: {
    boardSummaries?: unknown;
    failureNote?: unknown;
  }) => {
    // Records the summaries + failure note forwarded into the empty-state wiring.
    resultsProps.boardSummaries = boardSummaries;
    resultsProps.failureNote = failureNote;
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
  ScrapeForm: () => <div data-testid={TEST_IDS.jobs.scrapeForm} />,
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
  SegmentedControl: ({ onChange }: { onChange?: (v: string) => void }) => {
    segmentedControlContainer.onChange = onChange ?? null;
    return null;
  },
  useNotification: () => notifyMock,
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
    notifyMock.error.mockClear();
    notifyMock.success.mockClear();
    notifyMock.info.mockClear();
    notifyMock.warning.mockClear();
    boardChips.summaries = null;
    resultsProps.boardSummaries = undefined;
    resultsProps.failureNote = undefined;
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
    const outcome = scrapingMock.noteScrapeFinished.mock.calls[0]?.[1];
    expect(outcome?.ok).toBe(true);
    expect(outcome?.note).toBeUndefined();
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
    const outcome = scrapingMock.noteScrapeFinished.mock.calls[0]?.[1];
    expect(outcome?.ok).toBe(true);
    expect(outcome?.note).toBeDefined();
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
    const outcome = scrapingMock.noteScrapeFinished.mock.calls[0]?.[1];
    const note = outcome?.note ?? '';

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
    const outcome = scrapingMock.noteScrapeFinished.mock.calls[0]?.[1];
    const note = outcome?.note ?? '';

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
    const outcome = scrapingMock.noteScrapeFinished.mock.calls[0]?.[1];
    expect(outcome?.ok).toBe(true);
  });

  it('job.failed event → ok:false with the error data as note', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.failed',
      jobId: 'job-123',
      data: 'connection refused',
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    const outcome = scrapingMock.noteScrapeFinished.mock.calls[0]?.[1];
    expect(outcome?.ok).toBe(false);
    expect(outcome?.note).toBe('connection refused');
  });

  // ---------------------------------------------------------------------------
  // Per-board chip strip — retention + wiring (replaces the old skip-toasts)
  // ---------------------------------------------------------------------------

  it('retains the full per-board summaries and feeds them to the chip strip', async () => {
    renderJobsPage();

    const boards = [
      { board: 'linkedin', count: 5 },
      { board: 'indeed', count: 0, skipped: 'needs-login' },
      { board: 'xing', count: 0, error: 'rate limited' },
    ];
    fireJobEvent({ type: 'job.completed', jobId: 'job-123', data: { boards } });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    // The strip receives the untouched summaries (counts + skip + error), not a
    // lossy names-only projection that discards the "why".
    expect(boardChips.summaries).toEqual(boards);
    // The same data reaches the empty-state wiring in JobsResults.
    expect(resultsProps.boardSummaries).toEqual(boards);
  });

  it('surfaces a skipped board via the strip, NOT a toast (toasts were folded in)', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: { boards: [{ board: 'indeed', count: 0, skipped: 'needs-login' }] },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    expect(boardChips.summaries).toEqual([{ board: 'indeed', count: 0, skipped: 'needs-login' }]);
    // No transient warning toast — the strip is the persistent surface now.
    expect(notifyMock.warning).not.toHaveBeenCalled();
  });

  it('needs-keys and needs-company skips also route to the strip, no toast', async () => {
    renderJobsPage();

    const boards = [
      { board: 'aggregator', count: 0, skipped: 'needs-keys' },
      { board: 'greenhouse', count: 0, skipped: 'needs-company' },
    ];
    fireJobEvent({ type: 'job.completed', jobId: 'job-123', data: { boards } });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    expect(boardChips.summaries).toEqual(boards);
    expect(notifyMock.warning).not.toHaveBeenCalled();
  });

  it('a stale (inactive-job) completion does NOT overwrite the strip', async () => {
    renderJobsPage();

    // scrapeJobRef.current is 'job-123' (see useScraping mock); fire a DIFFERENT id.
    fireJobEvent({
      type: 'job.completed',
      jobId: 'other-job',
      data: { boards: [{ board: 'indeed', count: 0, skipped: 'needs-login' }] },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    // The active-job guard returns before setLastSummaries, so the header strip
    // never rendered — its capture stays at the reset default.
    expect(boardChips.summaries).toBeNull();
  });

  it('job.failed clears the strip (an outright failure has no per-board data)', async () => {
    renderJobsPage();

    // A completed run first populates the strip...
    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: { boards: [{ board: 'linkedin', count: 3 }] },
    });
    await waitFor(() => expect(boardChips.summaries).toEqual([{ board: 'linkedin', count: 3 }]));

    // ...then an outright failure clears it (surfaced via scrapeOutcome instead).
    fireJobEvent({ type: 'job.failed', jobId: 'job-123', data: 'connection refused' });
    await waitFor(() => expect(resultsProps.boardSummaries).toEqual([]));
  });

  // ---------------------------------------------------------------------------
  // Outright failure note (no per-board summaries to chip) — HIGH #3
  // ---------------------------------------------------------------------------

  it('job.failed persists a SANITIZED failure note (not the raw error) into the header + empty state', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.failed',
      jobId: 'job-123',
      data: 'connection refused at C:\\Users\\me\\x',
    });

    await waitFor(() =>
      expect(resultsProps.failureNote).toBe('sanitized:connection refused at C:\\Users\\me\\x')
    );
    // The header renders it via t('jobs.lastScrapeFailed', { reason }) — the t()
    // mock renders "key[reason=<value>]" so both the key and the SANITIZED value
    // (not the raw string) are visible in the DOM.
    expect(
      screen.getByText(
        'jobs.lastScrapeFailed[reason=sanitized:connection refused at C:\\Users\\me\\x]'
      )
    ).toBeInTheDocument();
  });

  it('a subsequent job.completed clears the failure note', async () => {
    renderJobsPage();

    fireJobEvent({ type: 'job.failed', jobId: 'job-123', data: 'boom' });
    await waitFor(() => expect(resultsProps.failureNote).toBe('sanitized:boom'));

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: { boards: [{ board: 'linkedin', count: 3 }] },
    });
    await waitFor(() =>
      expect(resultsProps.boardSummaries).toEqual([{ board: 'linkedin', count: 3 }])
    );
    expect(resultsProps.failureNote).toBeNull();
  });

  it('a partial-failure completion still keeps the scrapeOutcome note for the form footer', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [
          { board: 'linkedin', count: 5 },
          { board: 'xing', count: 0, error: 'rate limited' },
        ],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    const outcome = scrapingMock.noteScrapeFinished.mock.calls[0]?.[1];
    expect(outcome?.ok).toBe(true);
    expect(outcome?.note).toContain('jobs.boards.xing');
    // The skip-toast path is gone entirely.
    expect(notifyMock.warning).not.toHaveBeenCalled();
  });

  it('malformed data.boards (not an array) → does not throw, noteScrapeFinished called with ok:true and no note', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: { boards: {} },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    const outcome = scrapingMock.noteScrapeFinished.mock.calls[0]?.[1];
    expect(outcome?.ok).toBe(true);
    expect(outcome?.note).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// SegmentedControl view-mode toggle — setJobs branching behavior
// ---------------------------------------------------------------------------

describe('JobsPage — SegmentedControl viewMode toggle', () => {
  beforeEach(() => {
    setJobsSpy.mockClear();
    segmentedControlContainer.onChange = null;
  });

  it('switching to "split" calls setJobs with viewMode:split', () => {
    renderJobsPage();
    expect(segmentedControlContainer.onChange).toBeTypeOf('function');

    act(() => {
      segmentedControlContainer.onChange?.('split');
    });

    expect(setJobsSpy).toHaveBeenCalledWith({ viewMode: 'split' });
  });

  it('switching to "list" calls setJobs with viewMode:list', () => {
    renderJobsPage();
    expect(segmentedControlContainer.onChange).toBeTypeOf('function');

    act(() => {
      segmentedControlContainer.onChange?.('list');
    });

    expect(setJobsSpy).toHaveBeenCalledWith({ viewMode: 'list' });
  });
});
