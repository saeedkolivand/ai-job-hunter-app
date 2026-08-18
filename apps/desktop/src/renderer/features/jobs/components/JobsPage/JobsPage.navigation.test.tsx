/**
 * JobsPage — the scrape survives a route change.
 *
 * A route change unmounts JobsPage while the Rust scrape keeps running. Three
 * things used to die with the component; each gets a test here that fails if
 * the corresponding field goes back to component state:
 *
 *   1. DATA LOSS — the search signature reset to '', so "Show more" after a
 *      route change sent `replace: true` and the backend wiped the persisted
 *      postings on the first streamed item. Asserted on the WIRE payload.
 *   2. The in-flight jobId died, leaving an uncancellable orphan (no Cancel in
 *      the command bar) that the next search also failed to supersede.
 *   3. The per-board diagnostics — the only explanation of an empty result —
 *      were dropped.
 *
 * Unlike the sibling JobsPage tests this file uses the REAL `useScraping` and
 * the REAL session store: the defect lives precisely in the seam between them,
 * so mocking either out would make the test unable to fail.
 */
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import { TEST_IDS } from '@ajh/test-ids';

// ---------------------------------------------------------------------------
// Shared containers (objects, so hoisted vi.mock factories can reference them)
// ---------------------------------------------------------------------------

const jobEvents = { handler: null as ((event: unknown) => void) | null };

/** scrapeBoards mutation — the seam the `replace` flag actually crosses. */
const scrapeSpy = vi.fn<(payload: Record<string, unknown>) => Promise<unknown>>();
const cancelSpy = vi.fn<(jobId: string) => Promise<unknown>>();
/** Job-tracker poll behind the watchdog; a running job unless a test says otherwise. */
const fetchJobSpy = vi.fn<(jobId: string) => Promise<unknown>>();

const postingsContainer: { data: Array<Record<string, unknown>> } = { data: [] };

/** Props JobsCommandBar received on its last render. */
const commandBar = {
  scraping: undefined as unknown,
  boardSummaries: undefined as unknown,
  onCancelScrape: null as null | (() => void),
};

/** Props JobsResults received on its last render, plus its "Show more" handler. */
const results = {
  boardSummaries: undefined as unknown,
  failureNote: undefined as unknown,
  filtered: [] as Array<{ id: string }>,
  onShowMore: null as null | (() => void),
};

/** ScrapeForm handlers — the drawer's Search button and field edits. */
const scrapeForm = {
  onStart: null as null | (() => void),
  onFormChange: null as null | ((updates: Record<string, unknown>) => void),
};

// ---------------------------------------------------------------------------
// Module mocks — everything EXCEPT useScraping and the session store.
// ---------------------------------------------------------------------------

vi.mock('@/services', () => ({
  usePostings: () => postingsContainer,
  useClearPostings: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useInvalidatePostings: () => vi.fn().mockResolvedValue(undefined),
  useJobPreferences: () => ({ data: undefined }),
  useGeocodeSuggest: () => vi.fn().mockResolvedValue([]),
  useJobEvents: (cb: (event: unknown) => void) => {
    jobEvents.handler = cb;
  },
  useScrapeBoards: () => ({ mutateAsync: scrapeSpy }),
  useCancelJob: () => ({ mutateAsync: cancelSpy }),
  useScrapeProgress: () => null,
  fetchJob: (jobId: string) => fetchJobSpy(jobId),
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

vi.mock('@/components/scrape/BoardSummaryChips', () => ({
  BoardSummaryChips: () => null,
  sanitizeReason: (raw: string) => `sanitized:${raw}`,
}));

vi.mock('@/features/jobs/components/JobsCommandBar', () => ({
  JobsCommandBar: (props: {
    scraping?: unknown;
    boardSummaries?: unknown;
    onCancelScrape?: () => void;
  }) => {
    commandBar.scraping = props.scraping;
    commandBar.boardSummaries = props.boardSummaries;
    commandBar.onCancelScrape = props.onCancelScrape ?? null;
    return null;
  },
}));

vi.mock('@/features/jobs/components/JobsResults', () => ({
  JobsResults: (props: {
    boardSummaries?: unknown;
    failureNote?: unknown;
    filtered?: Array<{ id: string }>;
    onShowMore?: () => void;
  }) => {
    results.boardSummaries = props.boardSummaries;
    results.failureNote = props.failureNote;
    results.filtered = props.filtered ?? [];
    results.onShowMore = props.onShowMore ?? null;
    return <div data-testid={TEST_IDS.jobs.jobsResults} />;
  },
}));

vi.mock('@/features/jobs/components/ScrapeForm', () => ({
  ScrapeForm: (props: {
    onStart?: () => void;
    onFormChange?: (updates: Record<string, unknown>) => void;
  }) => {
    scrapeForm.onStart = props.onStart ?? null;
    scrapeForm.onFormChange = props.onFormChange ?? null;
    return <div data-testid={TEST_IDS.jobs.scrapeForm} />;
  },
}));

vi.mock('@ajh/translations', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));

vi.mock('@ajh/ui', () => ({
  Button: ({ children, onClick }: { children: ReactNode; onClick?: () => void }) => (
    <button type="button" onClick={onClick}>
      {children}
    </button>
  ),
  ConfirmModal: () => null,
  // Always render the children: the drawer's open state is component-local, and
  // these tests drive the form through the captured handlers, not the DOM.
  Drawer: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  Dropdown: () => null,
  Input: () => null,
  SegmentedControl: () => null,
  Tag: Object.assign(({ children }: { children: ReactNode }) => <span>{children}</span>, {
    CheckableTag: ({ children }: { children: ReactNode }) => <span>{children}</span>,
  }),
  useNotification: () => ({ error: vi.fn(), success: vi.fn(), info: vi.fn(), warning: vi.fn() }),
}));

// Import AFTER mocks.
import { JOBS_DEFAULTS, useSessionStore } from '@/store/session-store';

import { JobsPage } from './index';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function renderPage() {
  jobEvents.handler = null;
  return render(<JobsPage />);
}

/** The payload of the nth scrapeBoards invocation, as the backend received it. */
function sentPayload(callIndex: number): Record<string, unknown> {
  const payload = scrapeSpy.mock.calls[callIndex]?.[0];
  expect(payload).toBeDefined();
  return payload as Record<string, unknown>;
}

async function search(query: string) {
  await act(async () => {
    scrapeForm.onFormChange?.({ query });
  });
  await act(async () => {
    scrapeForm.onStart?.();
    await Promise.resolve();
  });
}

/** A minimal valid streamed Posting (the handler shape-checks these fields). */
function posting(id: string) {
  return {
    id,
    source: 'linkedin',
    externalId: id,
    url: `https://example.com/${id}`,
    title: `Role ${id}`,
    company: 'Acme',
    description: '',
    capturedAt: 0,
  };
}

function fireJobEvent(event: unknown) {
  act(() => {
    jobEvents.handler?.(event);
  });
}

beforeEach(() => {
  useSessionStore.setState({ jobs: { ...JOBS_DEFAULTS, viewMode: 'list' } });
  scrapeSpy.mockReset().mockResolvedValue({ jobId: 'job-1' });
  cancelSpy.mockReset().mockResolvedValue(undefined);
  fetchJobSpy.mockReset().mockResolvedValue({ status: 'running' });
  postingsContainer.data = [];
  commandBar.scraping = undefined;
  commandBar.boardSummaries = undefined;
  results.boardSummaries = undefined;
});

// ---------------------------------------------------------------------------
// 1 — DATA LOSS
// ---------------------------------------------------------------------------

describe('JobsPage — "Show more" after a route change (defect 1: data loss)', () => {
  it('appends instead of replacing: no `replace` flag reaches the backend', async () => {
    const first = renderPage();
    await search('engineer');

    // A brand-new search legitimately replaces — an absolute baseline, so a
    // regression that made `replace` disappear everywhere cannot pass this file.
    expect(sentPayload(0).replace).toBe(true);
    expect(sentPayload(0)).toMatchObject({ query: 'engineer', amount: 25 });

    // The user goes to Settings and comes back.
    first.unmount();
    renderPage();

    // …and presses "Show more", whose entire contract is APPEND.
    await act(async () => {
      results.onShowMore?.();
      await Promise.resolve();
    });

    expect(scrapeSpy).toHaveBeenCalledTimes(2);
    expect(sentPayload(1).replace).toBeUndefined();
    // The criteria survived too — otherwise "Show more" would silently run a
    // DIFFERENT (empty-query) search and append foreign results.
    expect(sentPayload(1)).toMatchObject({ query: 'engineer', amount: 50 });
  });

  it('two form changes in one tick both survive (#884 — the store patch re-reads state)', async () => {
    renderPage();

    // A location pick fires onChange then onSelectSuggestion in the SAME tick.
    // Both must compose; spreading a render-captured form would drop the first.
    await act(async () => {
      scrapeForm.onFormChange?.({ location: 'Berlin' });
      scrapeForm.onFormChange?.({ countryCode: 'DE' });
    });

    const stored = useSessionStore.getState().jobs.scrapeForm;
    expect(stored.location).toBe('Berlin');
    expect(stored.countryCode).toBe('DE');
  });

  it('a genuinely new search after a route change still replaces', async () => {
    const first = renderPage();
    await search('engineer');
    first.unmount();

    renderPage();
    await search('designer');

    expect(sentPayload(1)).toMatchObject({ query: 'designer', replace: true });
  });
});

// ---------------------------------------------------------------------------
// 2 — the orphaned, uncancellable scrape
// ---------------------------------------------------------------------------

describe('JobsPage — the in-flight scrape after a route change (defect 2: orphan)', () => {
  it('the command bar comes back in a scraping state and can cancel the run', async () => {
    const first = renderPage();
    await search('engineer');
    expect(commandBar.scraping).toBe(true);

    first.unmount();
    renderPage();

    // Both the progress strip and the Cancel button are gated on `scraping`.
    expect(commandBar.scraping).toBe(true);

    await act(async () => {
      commandBar.onCancelScrape?.();
      await Promise.resolve();
    });

    expect(cancelSpy).toHaveBeenCalledWith('job-1');
    expect(commandBar.scraping).toBe(false);
    expect(useSessionStore.getState().jobs.scrapeJobId).toBeNull();
  });

  it('the next search supersedes the orphan instead of racing it into the same cache', async () => {
    const first = renderPage();
    await search('engineer');
    first.unmount();

    scrapeSpy.mockResolvedValue({ jobId: 'job-2' });
    renderPage();
    await search('designer');

    expect(cancelSpy).toHaveBeenCalledWith('job-1');
    expect(useSessionStore.getState().jobs.scrapeJobId).toBe('job-2');
  });

  it("the superseded job's stream items are still rejected (two scrapes, one cache)", async () => {
    const first = renderPage();
    await search('engineer');
    first.unmount();

    scrapeSpy.mockResolvedValue({ jobId: 'job-2' });
    renderPage();
    await search('designer');

    // An item still in flight from the cancelled job-1 must not land, and must
    // not consume the replace latch job-2's first item is waiting on.
    fireJobEvent({ type: 'job.stream', jobId: 'job-1', data: posting('stale') });
    expect(results.filtered.map((p) => p.id)).toEqual([]);
    expect(useSessionStore.getState().jobs.replacePending).toBe(true);

    fireJobEvent({ type: 'job.stream', jobId: 'job-2', data: posting('fresh') });
    expect(results.filtered.map((p) => p.id)).toEqual(['fresh']);
    expect(useSessionStore.getState().jobs.replacePending).toBe(false);
  });

  it('a remount onto a job that already finished settles instead of spinning forever', async () => {
    const first = renderPage();
    await search('engineer');
    first.unmount();

    // The job ended while the user was away: the `job.completed` event was
    // emitted to nobody (the subscription is page-scoped). Only the watchdog,
    // re-armed off the stored job id, can reconcile this.
    fetchJobSpy.mockResolvedValue({ status: 'completed', result: { count: 0, boards: [] } });
    vi.useFakeTimers();
    try {
      renderPage();
      expect(commandBar.scraping).toBe(true);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(2600);
      });

      expect(commandBar.scraping).toBe(false);
      expect(useSessionStore.getState().jobs.scrapeJobId).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});

// ---------------------------------------------------------------------------
// 3 — the per-board diagnostics
// ---------------------------------------------------------------------------

describe('JobsPage — per-board diagnostics after a route change (defect 3)', () => {
  const boards = [
    { board: 'aggregator', count: 0, error: '429 rate limited' },
    { board: 'greenhouse', count: 4 },
  ];

  it('the chip strip is still there after navigating away and back', async () => {
    const first = renderPage();
    await search('engineer');

    fireJobEvent({ type: 'job.completed', jobId: 'job-1', data: { boards } });
    expect(results.boardSummaries).toEqual(boards);

    first.unmount();
    renderPage();

    expect(results.boardSummaries).toEqual(boards);
  });

  it('a scrape that finishes off-page recovers its summaries from the job tracker', async () => {
    const first = renderPage();
    await search('engineer');
    first.unmount();

    fetchJobSpy.mockResolvedValue({ status: 'completed', result: { count: 4, boards } });
    vi.useFakeTimers();
    try {
      renderPage();
      await act(async () => {
        await vi.advanceTimersByTimeAsync(2600);
      });

      expect(results.boardSummaries).toEqual(boards);
    } finally {
      vi.useRealTimers();
    }
  });

  it('an outright failure note also survives the route change', async () => {
    const first = renderPage();
    await search('engineer');

    fireJobEvent({ type: 'job.failed', jobId: 'job-1', data: 'connection refused' });
    expect(results.failureNote).toBe('sanitized:connection refused');

    first.unmount();
    renderPage();

    expect(results.failureNote).toBe('sanitized:connection refused');
  });
});
