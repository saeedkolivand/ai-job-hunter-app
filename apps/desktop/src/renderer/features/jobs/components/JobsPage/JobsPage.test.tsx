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
const resultsProps = {
  boardSummaries: undefined as unknown,
  failureNote: undefined as unknown,
  totalCount: undefined as unknown,
  filtered: undefined as unknown,
};

// usePostings — mutable container so tests can simulate "results present" vs
// "zero results" for the header-strip mutual-exclusivity gating (the header
// strip/note only render alongside a non-empty results list; the empty state
// owns the zero-results explanation). Defaults to empty; individual tests set
// it explicitly so the scenario is never implicit.
const postingsContainer: { data: Array<Record<string, unknown>> } = { data: [] };

function samplePosting(id: string): Record<string, unknown> {
  return {
    id,
    source: 'linkedin',
    externalId: id,
    url: `https://example.com/${id}`,
    title: 'Engineer',
    company: 'Acme',
    description: '',
    capturedAt: 0,
  };
}

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
  usePostings: () => postingsContainer,
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
    totalCount,
    filtered,
  }: {
    boardSummaries?: unknown;
    failureNote?: unknown;
    totalCount?: unknown;
    filtered?: unknown;
  }) => {
    // Records the summaries + failure note + unfiltered count forwarded into
    // the empty-state wiring, plus the sorted `filtered` list so the stable
    // sort (PR H) is assertable end-to-end.
    resultsProps.boardSummaries = boardSummaries;
    resultsProps.failureNote = failureNote;
    resultsProps.totalCount = totalCount;
    resultsProps.filtered = filtered;
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
    resultsProps.totalCount = undefined;
    postingsContainer.data = [];
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

  it('job.failed event → ok:false with the SANITIZED error data as note', async () => {
    renderJobsPage();

    fireJobEvent({
      type: 'job.failed',
      jobId: 'job-123',
      data: 'connection refused',
    });

    await waitFor(() => expect(scrapingMock.noteScrapeFinished).toHaveBeenCalled());
    const outcome = scrapingMock.noteScrapeFinished.mock.calls[0]?.[1];
    expect(outcome?.ok).toBe(false);
    // Security advisory: the form-footer note is sanitized too, not the raw error.
    expect(outcome?.note).toBe('sanitized:connection refused');
  });

  // ---------------------------------------------------------------------------
  // Per-board chip strip — retention + wiring (replaces the old skip-toasts)
  // ---------------------------------------------------------------------------

  it('retains the full per-board summaries and feeds them to the chip strip', async () => {
    // A partial-failure completion implies results ARE present (linkedin
    // returned 5) — set postings so the header-strip results-present gate is
    // satisfied and the retention can be observed at the header too.
    postingsContainer.data = [samplePosting('a')];
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
    postingsContainer.data = [samplePosting('a')];
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
    postingsContainer.data = [samplePosting('a')];
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
    // Postings present so the header WOULD render if the active-job guard were
    // broken — isolates this test from the separate results-present gate.
    postingsContainer.data = [samplePosting('a')];
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

  it('job.failed clears the retained summaries (an outright failure has no per-board data)', async () => {
    renderJobsPage();

    // A completed run first populates the retained summaries (asserted via the
    // unconditional resultsProps signal — decoupled from header visibility)...
    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: { boards: [{ board: 'linkedin', count: 3 }] },
    });
    await waitFor(() =>
      expect(resultsProps.boardSummaries).toEqual([{ board: 'linkedin', count: 3 }])
    );

    // ...then an outright failure clears it (surfaced via scrapeOutcome instead).
    fireJobEvent({ type: 'job.failed', jobId: 'job-123', data: 'connection refused' });
    await waitFor(() => expect(resultsProps.boardSummaries).toEqual([]));
  });

  it('a stale (inactive-job) job.failed does NOT wipe the strip or paint a foreign error (jobs:event is a shared global channel)', async () => {
    // Postings present so the header WOULD show a wipe/foreign-note if the
    // active-job guard were broken.
    postingsContainer.data = [samplePosting('a')];
    renderJobsPage();

    // First populate the strip via a real completion for the ACTIVE job.
    const boards = [{ board: 'linkedin', count: 5 }];
    fireJobEvent({ type: 'job.completed', jobId: 'job-123', data: { boards } });
    await waitFor(() => expect(boardChips.summaries).toEqual(boards));

    // An unrelated background job (autopilot/AI/agent/pipeline — job.failed is
    // emitted on the SAME `jobs:event` channel) fails with a DIFFERENT jobId.
    fireJobEvent({ type: 'job.failed', jobId: 'unrelated-ai-job', data: 'AI generation failed' });

    // noteScrapeFinished still fires unconditionally (internally buffered/
    // guarded by job id — a foreign id is simply parked, never surfaced)...
    await waitFor(() =>
      expect(scrapingMock.noteScrapeFinished).toHaveBeenCalledWith('unrelated-ai-job', {
        ok: false,
        note: 'sanitized:AI generation failed',
      })
    );
    // ...but the strip and failure note are UNTOUCHED — no foreign wipe/paint.
    expect(boardChips.summaries).toEqual(boards);
    expect(resultsProps.boardSummaries).toEqual(boards);
    expect(resultsProps.failureNote).toBeNull();
    expect(screen.queryByText(/jobs\.lastScrapeFailed/)).not.toBeInTheDocument();
  });

  it('forwards the unfiltered posting count as totalCount (claude review advisory #2)', () => {
    postingsContainer.data = [samplePosting('a'), samplePosting('b'), samplePosting('c')];
    renderJobsPage();

    expect(resultsProps.totalCount).toBe(3);
  });

  // ---------------------------------------------------------------------------
  // Outright failure note (no per-board summaries to chip) — HIGH #3
  // ---------------------------------------------------------------------------

  it('job.failed persists a SANITIZED failure note (not the raw error) for the empty state', async () => {
    // Zero results (default) — the note routes to JobsResults' empty state
    // (verified end-to-end in JobsResults.test.tsx); this test proves the
    // data-layer signal is sanitized before it ever leaves JobsPage. The
    // header's OWN rendering of this note (when results ARE present) is
    // covered by the dedicated mutual-exclusivity block below.
    renderJobsPage();

    fireJobEvent({
      type: 'job.failed',
      jobId: 'job-123',
      data: 'connection refused at C:\\Users\\me\\x',
    });

    await waitFor(() =>
      expect(resultsProps.failureNote).toBe('sanitized:connection refused at C:\\Users\\me\\x')
    );
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
// Header strip mutual exclusivity with the empty state (🟡 fix): the header
// chip strip + failure note must render ONLY alongside a visible results
// list — with zero results, JobsResults' empty state is the SOLE owner of
// the explanation (both used to co-render, duplicating the same message).
// ---------------------------------------------------------------------------

describe('JobsPage — header strip mutual exclusivity with the empty state', () => {
  beforeEach(() => {
    boardChips.summaries = null;
    resultsProps.boardSummaries = undefined;
    resultsProps.failureNote = undefined;
    resultsProps.totalCount = undefined;
    postingsContainer.data = [];
  });

  it('ZERO results: the underlying data still reaches JobsResults, but the header strip does NOT render', async () => {
    renderJobsPage(); // postingsContainer.data = [] → filtered.length === 0

    fireJobEvent({
      type: 'job.completed',
      jobId: 'job-123',
      data: { boards: [{ board: 'linkedin', count: 0, error: 'blocked' }] },
    });

    await waitFor(() =>
      expect(resultsProps.boardSummaries).toEqual([
        { board: 'linkedin', count: 0, error: 'blocked' },
      ])
    );
    // Empty state owns it — the header's own strip instance never rendered.
    expect(boardChips.summaries).toBeNull();
    expect(screen.queryByTestId('board-summary-chips')).not.toBeInTheDocument();
  });

  it('ZERO results: the header failure note does NOT render (empty state owns it)', async () => {
    renderJobsPage();

    fireJobEvent({ type: 'job.failed', jobId: 'job-123', data: 'connection refused' });

    await waitFor(() => expect(resultsProps.failureNote).toBe('sanitized:connection refused'));
    expect(screen.queryByText(/jobs\.lastScrapeFailed/)).not.toBeInTheDocument();
  });

  it('RESULTS PRESENT: the header renders the chip strip', async () => {
    postingsContainer.data = [samplePosting('a')];
    renderJobsPage();

    const boards = [
      { board: 'linkedin', count: 5 },
      { board: 'xing', count: 0, error: 'blocked' },
    ];
    fireJobEvent({ type: 'job.completed', jobId: 'job-123', data: { boards } });

    await waitFor(() => expect(boardChips.summaries).toEqual(boards));
    expect(screen.getByTestId('board-summary-chips')).toBeInTheDocument();
  });

  it('RESULTS PRESENT: the header renders the failure note', async () => {
    postingsContainer.data = [samplePosting('a')];
    renderJobsPage();

    fireJobEvent({ type: 'job.failed', jobId: 'job-123', data: 'connection refused' });

    await waitFor(() => expect(resultsProps.failureNote).toBe('sanitized:connection refused'));
    expect(
      screen.getByText('jobs.lastScrapeFailed[reason=sanitized:connection refused]')
    ).toBeInTheDocument();
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

// ---------------------------------------------------------------------------
// Stable "newest" sort (PR H, audit quick win 8): equal timestamps get a
// deterministic id tiebreak, and undated postings (no `postedAt`) collect in a
// trailing band instead of interleaving via the `capturedAt` fallback. The
// session-store mock hardcodes sortBy: 'newest', so these assert that path.
// ---------------------------------------------------------------------------

function sortPosting(
  id: string,
  opts: { postedAt?: number; capturedAt: number }
): Record<string, unknown> {
  return {
    id,
    source: 'linkedin',
    externalId: id,
    // Distinct url/title so mergePostings' canonical-key dedup keeps every row.
    url: `https://example.com/${id}`,
    title: `Engineer ${id}`,
    company: 'Acme',
    description: '',
    postedAt: opts.postedAt,
    capturedAt: opts.capturedAt,
  };
}

function filteredIds(): string[] {
  return (resultsProps.filtered as Array<{ id: string }>).map((p) => p.id);
}

describe('JobsPage — stable newest sort', () => {
  beforeEach(() => {
    resultsProps.filtered = undefined;
    postingsContainer.data = [];
  });

  it('equal postedAt falls back to a deterministic id tiebreak (not input order)', () => {
    // Fed in reverse id order; a plain stable sort would keep it, so a passing
    // ['a','b'] proves the id tiebreak actually ran.
    postingsContainer.data = [
      sortPosting('b', { postedAt: 1000, capturedAt: 5 }),
      sortPosting('a', { postedAt: 1000, capturedAt: 5 }),
    ];
    renderJobsPage();
    expect(filteredIds()).toEqual(['a', 'b']);
  });

  it('undated postings (no postedAt) trail the dated ones, never interleaved', () => {
    postingsContainer.data = [
      // No postedAt but a very recent capture — must NOT jump above the dated row.
      sortPosting('undated', { capturedAt: 9999 }),
      sortPosting('dated', { postedAt: 100, capturedAt: 1 }),
    ];
    renderJobsPage();
    expect(filteredIds()).toEqual(['dated', 'undated']);
  });

  it('dated band is newest-first; undated band trails, sorted by capture then id', () => {
    postingsContainer.data = [
      sortPosting('old', { postedAt: 100, capturedAt: 1 }),
      sortPosting('new', { postedAt: 200, capturedAt: 1 }),
      sortPosting('u2', { capturedAt: 50 }),
      sortPosting('u1', { capturedAt: 50 }),
    ];
    renderJobsPage();
    // Dated newest-first: new, old. Undated trail; equal capture → id tiebreak.
    expect(filteredIds()).toEqual(['new', 'old', 'u1', 'u2']);
  });
});

// ---------------------------------------------------------------------------
// Header/scrape-form scroll container — jsdom can't measure layout, so assert
// the bounding + scroll classes are present on the wrapper. Without them, a
// full board selection + open advanced grid overflows the viewport and the
// Start button becomes unreachable at the 900x600 window floor.
// ---------------------------------------------------------------------------

describe('JobsPage — scrape form scroll container', () => {
  beforeEach(() => {
    postingsContainer.data = [];
  });

  it('bounds and scrolls the header wrapper so tall form content cannot clip the Start button', () => {
    renderJobsPage();

    const wrapper = screen.getByTestId(TEST_IDS.jobs.scrapeFormScroll);
    expect(wrapper.className).toContain('overflow-y-auto');
    expect(wrapper.className).toContain('max-h-[55vh]');
    expect(wrapper.className).toContain('min-h-0');
  });
});
