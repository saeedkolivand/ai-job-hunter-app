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

vi.mock('@/features/jobs/hooks/useDefaultResumeId', () => ({
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
  JobsResults: () => <div data-testid={TEST_IDS.jobs.jobsResults} />,
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
  // Skipped-boards (needs-login) notification tests
  // ---------------------------------------------------------------------------

  it('single skipped board → warning fired once, sticky (duration:0), correct key + params', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [{ board: 'indeed', count: 0, skipped: 'needs-login' }],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    expect(notifyMock.warning).toHaveBeenCalledTimes(1);

    const call = notifyMock.warning.mock.calls[0]?.[0] as { message: string; duration: number };
    // duration:0 = sticky; user must dismiss manually.
    expect(call.duration).toBe(0);
    // t() mock: "jobs.needsLogin.skippedNote[boards=<boardName>,count=<n>]"
    // boardName = t('jobs.boards.indeed') = 'jobs.boards.indeed' (identity mock)
    expect(call.message).toContain('jobs.needsLogin.skippedNote');
    expect(call.message).toContain('boards=jobs.boards.indeed');
    expect(call.message).toContain('count=1');
  });

  it('two skipped boards → warning with count:2 and both board names in boards param', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [
          { board: 'indeed', count: 0, skipped: 'needs-login' },
          { board: 'xing', count: 0, skipped: 'needs-login' },
        ],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    expect(notifyMock.warning).toHaveBeenCalledTimes(1);

    const call = notifyMock.warning.mock.calls[0]?.[0] as { message: string; duration: number };
    expect(call.duration).toBe(0);
    expect(call.message).toContain('jobs.needsLogin.skippedNote');
    // Both translated board keys must appear in the boards param value.
    expect(call.message).toContain('jobs.boards.indeed');
    expect(call.message).toContain('jobs.boards.xing');
    expect(call.message).toContain('count=2');
  });

  it('skipped + failed in same payload → both warning AND partial-failure note fire', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [
          { board: 'linkedin', count: 5 },
          { board: 'indeed', count: 0, skipped: 'needs-login' },
          { board: 'xing', count: 0, error: 'rate limited' },
        ],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());

    // Partial-failure note: xing had an error → note is defined.
    const outcome = scrapingMock.noteScrapeFinished.mock.calls[0]?.[1];
    expect(outcome?.ok).toBe(true);
    expect(outcome?.note).toBeDefined();
    expect(outcome?.note).toContain('jobs.boards.xing');

    // Skipped warning: indeed was skipped → warning fired.
    expect(notifyMock.warning).toHaveBeenCalledTimes(1);
    const warningCall = notifyMock.warning.mock.calls[0]?.[0] as {
      message: string;
      duration: number;
    };
    expect(warningCall.message).toContain('jobs.needsLogin.skippedNote');
    expect(warningCall.message).toContain('jobs.boards.indeed');
  });

  it('no skipped boards (normal completion) → warning is NOT called', async () => {
    renderJobsPage();

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
    expect(notifyMock.warning).not.toHaveBeenCalled();
  });

  // ---------------------------------------------------------------------------
  // Skipped-boards (needs-company) notification tests
  // ---------------------------------------------------------------------------

  it('single needs-company board → warning fired once, sticky (duration:0), correct key + params', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [{ board: 'greenhouse', count: 0, skipped: 'needs-company' }],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    expect(notifyMock.warning).toHaveBeenCalledTimes(1);

    const call = notifyMock.warning.mock.calls[0]?.[0] as { message: string; duration: number };
    // duration:0 = sticky; user must add a company slug to dismiss the root cause.
    expect(call.duration).toBe(0);
    // t() mock: "jobs.needsCompany.skippedNote[boards=<boardName>,count=<n>]"
    // boardName = t('jobs.boards.greenhouse') = 'jobs.boards.greenhouse' (identity mock)
    expect(call.message).toContain('jobs.needsCompany.skippedNote');
    expect(call.message).toContain('boards=jobs.boards.greenhouse');
    expect(call.message).toContain('count=1');
  });

  it('two needs-company boards → warning with count:2 and both board names in boards param', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [
          { board: 'greenhouse', count: 0, skipped: 'needs-company' },
          { board: 'lever', count: 0, skipped: 'needs-company' },
        ],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    expect(notifyMock.warning).toHaveBeenCalledTimes(1);

    const call = notifyMock.warning.mock.calls[0]?.[0] as { message: string; duration: number };
    expect(call.duration).toBe(0);
    expect(call.message).toContain('jobs.needsCompany.skippedNote');
    expect(call.message).toContain('jobs.boards.greenhouse');
    expect(call.message).toContain('jobs.boards.lever');
    expect(call.message).toContain('count=2');
  });

  it('no needs-company boards (normal completion) → needs-company warning NOT fired', async () => {
    renderJobsPage();

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
    // No warning at all (neither needs-login nor needs-company).
    expect(notifyMock.warning).not.toHaveBeenCalled();
  });

  it('needs-company + needs-login in same payload → two warning calls, each with correct key', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: {
        boards: [
          { board: 'linkedin', count: 5 },
          { board: 'indeed', count: 0, skipped: 'needs-login' },
          { board: 'greenhouse', count: 0, skipped: 'needs-company' },
        ],
      },
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    // Both warnings fire (one per skip reason).
    expect(notifyMock.warning).toHaveBeenCalledTimes(2);

    const messages = (
      notifyMock.warning.mock.calls as Array<[{ message: string; duration: number }]>
    ).map((c) => c[0].message);
    expect(messages.some((m) => m.includes('jobs.needsLogin.skippedNote'))).toBe(true);
    expect(messages.some((m) => m.includes('jobs.needsCompany.skippedNote'))).toBe(true);
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

  it('switching to "split" calls setJobs with viewMode:split AND detailCollapsed:false', () => {
    renderJobsPage();
    expect(segmentedControlContainer.onChange).toBeTypeOf('function');

    act(() => {
      segmentedControlContainer.onChange?.('split');
    });

    expect(setJobsSpy).toHaveBeenCalledWith({ viewMode: 'split', detailCollapsed: false });
  });

  it('switching to "list" calls setJobs with only viewMode:list (no detailCollapsed)', () => {
    renderJobsPage();
    expect(segmentedControlContainer.onChange).toBeTypeOf('function');

    act(() => {
      segmentedControlContainer.onChange?.('list');
    });

    expect(setJobsSpy).toHaveBeenCalledWith({ viewMode: 'list' });
    // Critically: detailCollapsed must NOT be present in the list-mode call.
    const callArg = setJobsSpy.mock.calls[0]?.[0] as Record<string, unknown> | undefined;
    expect(callArg).not.toHaveProperty('detailCollapsed');
  });
});
